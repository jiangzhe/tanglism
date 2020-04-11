pub mod daily;
pub mod ticks;

use crate::helpers::respond_json;
use crate::models::{StockDailyPrice, StockPriceTick, StockTickPrice};
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::get;
use actix_web::web::{self, Json};
use chrono::{NaiveDate, NaiveDateTime};
use jqdata::JqdataClient;
use log::debug;
use serde_derive::*;
use std::sync::Arc;
use tanglism_utils::{parse_date_from_str, parse_ts_from_str, TradingDates, LOCAL_DATES};


#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    code: String,
    tick: String,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
    data: Vec<T>,
}

#[get("/stock-prices/{code}/daily")]
pub async fn api_get_stock_daily_prices(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<daily::Path>,
    param: web::Query<daily::Param>,

) -> Result<Json<daily::Response>> {
    let end_dt = param
        .end_dt
        .unwrap_or_else(|| chrono::Local::today().naive_local());
    let data = get_stock_daily_prices(&pool, &jq, &path.code, param.start_dt, end_dt).await?;
    respond_json(Response {
        code: path.code.to_owned(),
        tick: "1d".to_owned(),
        start_dt: param.start_dt,
        end_dt: end_dt,
        data,
    })
}

pub async fn get_stock_daily_prices(
    pool: &DbPool,
    jq: &JqdataClient,
    code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
) -> Result<Vec<daily::StockPrice>> {
    let code = Arc::new(code.to_owned());
    // 起始时间大于结束时间或当天
    if start_dt > end_dt {
        return Err(Error::Custom(
            ErrorKind::BadRequest,
            format!("start_dt {} > end_dt {}", start_dt, end_dt),
        ));
    } else if start_dt >= chrono::Local::today().naive_local() {
        return Err(Error::custom(
            ErrorKind::BadRequest,
            format!("start_dt {} >= current day not supported", start_dt),
        ));
    }

    let period = {
        let code = Arc::clone(&code);
        let pool = pool.clone();
        web::block(move || query_db_period(&pool, "1d", &code)).await?
    };
    if let Some(period) = period {
        // 数据库中存在时间段，说明已进行过查询，则仅进行增量查询并插入

        // 当且仅当查询起始时间早于或者等于数据库中开始日期的前一个交易日，则进行API查询
        if let Some(prev_day) = LOCAL_DATES.prev_day(period.start_dt) {
            if start_dt <= prev_day {
                debug!(
                    "{} daily prices between {} and {} will be fetched via remote API",
                    &code, start_dt, prev_day
                );
                let resp = daily::query_api_prices(&jq, &code, start_dt, prev_day).await?;
                if !resp.is_empty() {
                    let mut prices = Vec::with_capacity(resp.len());
                    for p in resp.into_iter() {
                        let dp = jq_price_to_daily_price(&code, p)?;
                        prices.push(dp);
                    }
                    let pool = pool.clone();
                    web::block(move || {
                        insert_daily_prices(&pool, &prices, UpdatePricePeriod::Lowerbound)
                    })
                    .await?;
                }
            }
        }

        // 当且仅当查询结束时间晚于或等于数据库中结束日期的下一个交易日，则进行API查询
        if let Some(next_day) = LOCAL_DATES.next_day(period.end_dt) {
            if end_dt >= next_day {
                debug!(
                    "{} daily prices between {} and {} will be fetched via remote API",
                    &code, next_day, end_dt
                );
                let resp = daily::query_api_prices(&jq, &code, next_day, end_dt).await?;
                if !resp.is_empty() {
                    let mut prices = Vec::with_capacity(resp.len());
                    for p in resp.into_iter() {
                        let dp = jq_price_to_daily_price(&code, p)?;
                        prices.push(dp);
                    }
                    let pool = pool.clone();
                    web::block(move || {
                        insert_daily_prices(&pool, &prices, UpdatePricePeriod::Upperbound)
                    })
                    .await?;
                }
            }
        }
    } else {
        // 数据库中无区间，进行第一次全量查询并插入
        debug!(
            "{} daily prices between {} and {} will be initially loaded",
            &code, start_dt, end_dt
        );
        let resp = daily::query_api_prices(&jq, &code, start_dt, end_dt).await?;
        if !resp.is_empty() {
            let mut prices = Vec::with_capacity(resp.len());
            for p in resp.into_iter() {
                let dp = jq_price_to_daily_price(&code, p)?;
                prices.push(dp);
            }
            let pool = pool.clone();
            web::block(move || insert_daily_prices(&pool, &prices, UpdatePricePeriod::Entire))
                .await?;
        }
    }

    let data = {
        let code = Arc::clone(&code);
        let pool = pool.clone();
        web::block(move || daily::query_db_prices(&pool, &code, start_dt, end_dt)).await?
    };
    Ok(data)
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

pub async fn get_stock_tick_prices(
    pool: &DbPool,
    jq: &JqdataClient,
    tick: &str,
    code: &str,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
) -> Result<Vec<ticks::StockPrice>> {
    // 仅支持1m, 5m, 30m
    let tick = match tick {
        "1m" | "5m" | "30m" => Arc::new(tick.to_owned()),
        _ => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                format!("Invalid tick: {}", tick),
            ))
        }
    };
    
    let code = Arc::new(code.to_owned());
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
                debug!(
                    "{} {} prices between {} and {} will be fetched via remote API",
                    &code, &tick, start_ts.date(), prev_day
                );
                let resp = ticks::query_api_prices(&jq, &tick, &code, start_ts.date(), prev_day).await?;
                if !resp.is_empty() {
                    let mut prices = Vec::with_capacity(resp.len());
                    for p in resp.into_iter() {
                        let dp = jq_price_to_tick_price(&tick, &code, p)?;
                        prices.push(dp);
                    }
                    let pool = pool.clone();
                    web::block(move || {
                        insert_tick_prices(&pool, &prices, UpdatePricePeriod::Lowerbound)
                    })
                    .await?;
                }
            }
        }

        // 当且仅当数据库中结束日期的下一个交易日早于或等于给定的结束日期，则进行API查询
        if let Some(next_day) = LOCAL_DATES.next_day(period.end_dt) {
            if next_day <= end_ts.date() {
                debug!(
                    "{} {} prices between {} and {} will be fetched via remote API",
                    &code, &tick, next_day, end_ts.date()
                );
                let resp = ticks::query_api_prices(&jq, &tick, &code, next_day, end_ts.date()).await?;
                if !resp.is_empty() {
                    let mut prices = Vec::with_capacity(resp.len());
                    for p in resp.into_iter() {
                        let dp = jq_price_to_tick_price(&tick, &code, p)?;
                        prices.push(dp);
                    }
                    let pool = pool.clone();
                    web::block(move || {
                        insert_tick_prices(&pool, &prices, UpdatePricePeriod::Upperbound)
                    })
                    .await?;
                }
            }
        }
    } else {
        // 数据库中无区间，进行第一次全量查询并插入
        debug!(
            "{} {} prices between {} and {} will be initially loaded",
            &code, &tick, start_ts.date(), end_ts.date()
        );
        let resp = ticks::query_api_prices(&jq, &tick, &code, start_ts.date(), end_ts.date()).await?;
        if !resp.is_empty() {
            let mut prices = Vec::with_capacity(resp.len());
            for p in resp.into_iter() {
                let dp = jq_price_to_tick_price(&tick, &code, p)?;
                prices.push(dp);
            }
            let pool = pool.clone();
            web::block(move || insert_tick_prices(&pool, &prices, UpdatePricePeriod::Entire))
                .await?;
        }
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

#[inline]
fn jq_price_to_daily_price(code: &str, p: jqdata::Price) -> Result<StockDailyPrice> {
    let dp = StockDailyPrice {
        code: code.to_owned(),
        dt: parse_date_from_str(&p.date)?,
        open: p.open,
        close: p.close,
        high: p.high,
        low: p.low,
        volume: p.volume,
        amount: p.money,
    };
    Ok(dp)
}

#[inline]
fn jq_price_to_tick_price(tick: &str, code: &str, p: jqdata::Price) -> Result<StockTickPrice> {
    let (ts, _) = parse_ts_from_str(&p.date)?;
    let dp = StockTickPrice {
        tick: tick.to_owned(),
        code: code.to_owned(),
        ts,
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
        Ok(rs) => return Ok(Some(rs)),
        Err(diesel::result::Error::NotFound) => return Ok(None),
        Err(err) => return Err(err.into()),
    }
}

#[derive(Debug)]
enum UpdatePricePeriod {
    Entire,
    Upperbound,
    Lowerbound,
}

fn insert_daily_prices(
    pool: &DbPool,
    prices: &[StockDailyPrice],
    upd: UpdatePricePeriod,
) -> Result<()> {
    if prices.is_empty() {
        return Ok(());
    }
    let input_code: &str = &prices.first().as_ref().unwrap().code;
    let input_tick: &str = "1d";

    use diesel::prelude::*;

    let conn = pool.get()?;
    conn.transaction::<_, Error, _>(|| {
        // 插入价格数据
        {
            use crate::schema::stock_daily_prices::dsl::*;
            diesel::insert_into(stock_daily_prices)
                .values(prices)
                .execute(&conn)?;
            debug!("{} rows of stock daily prices inserted", prices.len());
        }
        // 更新价格区间
        {
            use crate::schema::stock_price_ticks::dsl::*;
            match upd {
                UpdatePricePeriod::Upperbound => {
                    let input_end_dt = prices.last().as_ref().unwrap().dt;
                    diesel::update(
                        stock_price_ticks.filter(code.eq(input_code).and(tick.eq(input_tick))),
                    )
                    .set(end_dt.eq(input_end_dt))
                    .execute(&conn)?;
                }
                UpdatePricePeriod::Lowerbound => {
                    let input_start_dt = prices.first().as_ref().unwrap().dt;
                    diesel::update(
                        stock_price_ticks.filter(code.eq(input_code).and(tick.eq(input_tick))),
                    )
                    .set(start_dt.eq(input_start_dt))
                    .execute(&conn)?;
                }
                UpdatePricePeriod::Entire => {
                    let input_start_dt = prices.first().as_ref().unwrap().dt;
                    let input_end_dt = prices.last().as_ref().unwrap().dt;
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
            debug!("{} rows of stock tick[{}] prices inserted", prices.len(), input_tick);
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
