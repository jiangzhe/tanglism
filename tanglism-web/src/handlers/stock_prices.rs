pub mod daily;
pub mod ticks;

use crate::helpers::respond_json;
use crate::models::{StockPriceTick, StockTickPrice};
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::get;
use actix_web::web::{self, Json};
use chrono::{NaiveDate, NaiveDateTime};
use jqdata::JqdataClient;
use lazy_static::*;
use log::{debug, warn};
use serde_derive::*;
use std::collections::HashMap;
use std::sync::Arc;
use tanglism_utils::{parse_ts_from_str, TradingDates, LOCAL_DATES};
use tokio::sync::Mutex;

// 批量插入操作的数量限制，受限于SQL的变量绑定<=65535
const MAX_DB_INSERT_BATCH_SIZE: i64 = 5000;

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    code: String,
    tick: String,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
    data: Vec<T>,
}

#[get("/stock-prices/{code}/ticks/{tick}")]
pub async fn api_get_stock_tick_prices(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<ticks::Param>,
) -> Result<Json<ticks::Response>> {
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let data = get_stock_tick_prices(&pool, &jq, &path.tick, &path.code, start_ts, end_ts).await?;
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_dt: start_ts.date(),
        end_dt: end_ts.date(),
        data,
    })
}

// 对于价格的查询和插入，使用互斥锁
lazy_static! {
    static ref PRICE_ACCESS: Arc<Mutex<PriceTickAccess>> =
        Arc::new(Mutex::new(PriceTickAccess::new()));
}

struct PriceTickAccess(HashMap<String, Arc<Mutex<()>>>);

impl PriceTickAccess {
    fn new() -> Self {
        PriceTickAccess(HashMap::new())
    }

    fn get(&mut self, tick: &str, code: &str) -> Arc<Mutex<()>> {
        let key = format!("{}/{}", tick, code);
        let value = self
            .0
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())));
        Arc::clone(value)
    }
}

pub async fn get_stock_tick_prices(
    pool: &DbPool,
    jq: &JqdataClient,
    tick: &str,
    code: &str,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
) -> Result<Vec<ticks::StockPrice>> {
    // 仅支持1m, 5m, 30m, 1d
    let tick = match tick {
        "1m" | "5m" | "30m" | "1d" => Arc::new(tick.to_owned()),
        _ => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                format!("Invalid tick: {}", tick),
            ))
        }
    };
    // 起始时间大于结束时间或当天
    if start_ts > end_ts {
        return Err(Error::custom(
            ErrorKind::BadRequest,
            format!("start_ts {} > end_ts {}", start_ts, end_ts),
        ));
    } else if start_ts >= chrono::Local::today().naive_local().and_hms(0, 0, 0) {
        return Err(Error::custom(
            ErrorKind::BadRequest,
            format!("start_ts {} >= current day not supported", start_ts),
        ));
    }

    let code = Arc::new(code.to_owned());

    // 禁止多线程同时读写price表
    let pa = {
        let mut pas = PRICE_ACCESS.lock().await;
        pas.get(&tick, &code)
    };
    let _pa_access = pa.lock().await;

    // 检查已抓取的数据区间
    let period = {
        let code = Arc::clone(&code);
        let tick = Arc::clone(&tick);
        let pool = pool.clone();
        web::block(move || query_db_period(&pool, &tick, &code)).await?
    };
    if let Some(period) = period {
        // 数据库中存在时间段，说明已进行过查询，则仅进行增量查询并插入

        // 当且仅当数据库中开始日期的前一个交易日晚于或等于给定的起始日期，则进行API查询
        if let Some(prev_day) = LOCAL_DATES.prev_day(period.start_dt) {
            if prev_day.and_hms(15, 30, 1) > start_ts {
                fill_prices(
                    &jq,
                    &pool,
                    &tick,
                    &code,
                    start_ts.date(),
                    prev_day,
                    UpdatePricePeriod::Lowerbound,
                )
                .await?;
            }
        }

        // 当且仅当数据库中结束日期的下一个交易日早于或等于给定的结束日期，则进行API查询
        if let Some(next_day) = LOCAL_DATES.next_day(period.end_dt) {
            if next_day <= end_ts.date() {
                fill_prices(
                    &jq,
                    &pool,
                    &tick,
                    &code,
                    next_day,
                    end_ts.date(),
                    UpdatePricePeriod::Upperbound,
                )
                .await?;
            }
        }
    } else {
        // 数据库中无区间，进行第一次全量查询并插入
        fill_prices(
            &jq,
            &pool,
            &tick,
            &code,
            start_ts.date(),
            end_ts.date(),
            UpdatePricePeriod::Entire,
        )
        .await?;
    }
    let data = {
        let tick = Arc::clone(&tick);
        let code = Arc::clone(&code);
        let pool = pool.clone();
        let start_dt = start_ts.date();
        let end_dt = end_ts.date();
        web::block(move || ticks::query_db_prices(&pool, &tick, &code, start_dt, end_dt)).await?
    };
    Ok(data)
}

async fn fill_prices(
    jq: &JqdataClient,
    pool: &DbPool,
    tick: &str,
    code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
    upd: UpdatePricePeriod,
) -> Result<()> {
    let estimated_batch_size = estimate_batch_size(start_dt, end_dt, &tick);
    if estimated_batch_size >= MAX_DB_INSERT_BATCH_SIZE {
        warn!(
            "Estimated db insertion batch size exceeds limitation for data from {} to {}: {} rows",
            start_dt, end_dt, estimated_batch_size
        );
        return Err(Error::custom(
            ErrorKind::BadRequest,
            "Date range exceeds query limit".to_owned(),
        ));
    }
    debug!(
        "{} {} prices between {} and {} will be fetched via remote API",
        &code, &tick, start_dt, end_dt
    );
    let resp = ticks::query_api_prices(jq, tick, code, start_dt, end_dt).await?;
    if !resp.is_empty() {
        let mut prices = Vec::with_capacity(resp.len());
        for p in resp.into_iter() {
            let dp = jq_price_to_tick_price(&tick, &code, p)?;
            prices.push(dp);
        }
        let pool = pool.clone();
        web::block(move || insert_tick_prices(&pool, &prices, upd)).await?;
    }
    Ok(())
}

#[inline]
fn jq_price_to_tick_price(tick: &str, code: &str, p: jqdata::Price) -> Result<StockTickPrice> {
    let (ts, is_day) = parse_ts_from_str(&p.date)?;
    let dp = StockTickPrice {
        tick: tick.to_owned(),
        code: code.to_owned(),
        //如果是日期，则转换为每日收盘时间
        ts: if is_day {
            ts.date().and_hms(15, 0, 0)
        } else {
            ts
        },
        open: p.open,
        close: p.close,
        high: p.high,
        low: p.low,
        volume: p.volume,
        amount: p.money,
    };
    Ok(dp)
}

pub fn query_db_period(
    pool: &DbPool,
    input_tick: &str,
    input_code: &str,
) -> Result<Option<StockPriceTick>> {
    use crate::schema::stock_price_ticks::dsl::*;
    use diesel::prelude::*;
    let conn = pool.get()?;
    match stock_price_ticks
        .find((input_tick, input_code))
        .first(&conn)
    {
        Ok(rs) => Ok(Some(rs)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[derive(Debug)]
enum UpdatePricePeriod {
    Entire,
    Upperbound,
    Lowerbound,
}

fn estimate_batch_size(start_dt: NaiveDate, end_dt: NaiveDate, tick: &str) -> i64 {
    let size_per_day = match tick {
        "1d" => 1,
        "30m" => 8,
        "5m" => 48,
        "1m" => 240,
        _ => return std::i64::MAX,
    };

    let naive_size = ((end_dt - start_dt).num_days() + 1) * size_per_day;
    if naive_size < MAX_DB_INSERT_BATCH_SIZE {
        return naive_size;
    }
    let mut start = start_dt;
    let mut size = 0;
    while LOCAL_DATES.contains_day(start) && start <= end_dt {
        size += size_per_day;

        if let Some(next_day) = LOCAL_DATES.next_day(start) {
            start = next_day;
        } else {
            return std::i64::MAX;
        }
    }
    size
}

fn insert_tick_prices(
    pool: &DbPool,
    prices: &[StockTickPrice],
    upd: UpdatePricePeriod,
) -> Result<()> {
    if prices.is_empty() {
        return Ok(());
    }
    let input_code: &str = &prices.first().as_ref().unwrap().code;
    let input_tick: &str = &prices.first().as_ref().unwrap().tick;

    use diesel::prelude::*;

    let conn = pool.get()?;
    conn.transaction::<_, Error, _>(|| {
        // 插入价格数据
        {
            use crate::schema::stock_tick_prices::dsl::*;
            diesel::insert_into(stock_tick_prices)
                .values(prices)
                .execute(&conn)?;
            debug!(
                "{} rows of stock tick[{}] prices inserted",
                prices.len(),
                input_tick
            );
        }
        // 更新价格区间
        {
            use crate::schema::stock_price_ticks::dsl::*;
            match upd {
                UpdatePricePeriod::Upperbound => {
                    let input_end_dt = prices.last().as_ref().unwrap().ts.date();
                    diesel::update(
                        stock_price_ticks.filter(code.eq(input_code).and(tick.eq(input_tick))),
                    )
                    .set(end_dt.eq(input_end_dt))
                    .execute(&conn)?;
                }
                UpdatePricePeriod::Lowerbound => {
                    let input_start_dt = prices.first().as_ref().unwrap().ts.date();
                    diesel::update(
                        stock_price_ticks.filter(code.eq(input_code).and(tick.eq(input_tick))),
                    )
                    .set(start_dt.eq(input_start_dt))
                    .execute(&conn)?;
                }
                UpdatePricePeriod::Entire => {
                    let input_start_dt = prices.first().as_ref().unwrap().ts.date();
                    let input_end_dt = prices.last().as_ref().unwrap().ts.date();
                    diesel::insert_into(stock_price_ticks)
                        .values(StockPriceTick {
                            code: input_code.to_owned(),
                            tick: input_tick.to_owned(),
                            start_dt: input_start_dt,
                            end_dt: input_end_dt,
                        })
                        .execute(&conn)?;
                }
            }
            debug!("stock price tick updated with state {:?}", upd);
        }
        Ok(())
    })
}
