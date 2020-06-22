use crate::models::StockTickPrice;
use crate::schema::stock_tick_prices;
use crate::{DbPool, Error, ErrorKind, Result};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use jqdata::{GetPricePeriod, JqdataClient};
use serde_derive::*;
use tanglism_utils::{end_of_day_str, start_of_day_str};

#[derive(Debug, Serialize, Deserialize, Queryable, Clone)]
pub struct StockPrice {
    pub ts: NaiveDateTime,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub volume: BigDecimal,
    pub amount: BigDecimal,
}

type StockPriceColumns = (
    stock_tick_prices::ts,
    stock_tick_prices::open,
    stock_tick_prices::close,
    stock_tick_prices::high,
    stock_tick_prices::low,
    stock_tick_prices::volume,
    stock_tick_prices::amount,
);
const STOCK_PRICE_COLUMNS: StockPriceColumns = (
    stock_tick_prices::ts,
    stock_tick_prices::open,
    stock_tick_prices::close,
    stock_tick_prices::high,
    stock_tick_prices::low,
    stock_tick_prices::volume,
    stock_tick_prices::amount,
);

pub async fn query_db_prices(
    pool: DbPool,
    input_tick: String,
    input_code: String,
    input_start_dt: NaiveDate,
    input_end_dt: NaiveDate,
) -> Result<Vec<StockPrice>> {
    let data = tokio::task::spawn_blocking(move || {
        use crate::schema::stock_tick_prices::dsl::*;
        let conn = pool.get().map_err(Error::from)?;
        let input_start_ts = input_start_dt.and_hms(0, 0, 0);
        let input_end_ts = input_end_dt.and_hms(23, 59, 59);
        stock_tick_prices
            .filter(
                tick.eq(input_tick)
                    .and(code.eq(input_code))
                    .and(ts.ge(input_start_ts))
                    .and(ts.le(input_end_ts)),
            )
            .order(ts.asc())
            .select(STOCK_PRICE_COLUMNS)
            .load::<StockPrice>(&conn)
            .map_err(Error::from)
    })
    .await??;
    Ok(data)
}

pub async fn query_db_multiple_prices(
    pool: DbPool,
    input_tick: String,
    input_codes: Vec<String>,
    input_start_dt: NaiveDate,
    input_end_dt: NaiveDate,
) -> Result<Vec<StockTickPrice>> {
    if input_codes.is_empty() {
        return Err(Error::custom(
            ErrorKind::InternalServerError,
            "input codes are empty".to_owned(),
        ));
    }
    let data = tokio::task::spawn_blocking(move || {
        use crate::schema::stock_tick_prices::dsl::*;
        let conn = pool.get().map_err(Error::from)?;
        let input_start_ts = input_start_dt.and_hms(0, 0, 0);
        let input_end_ts = input_end_dt.and_hms(23, 59, 59);
        stock_tick_prices
            .filter(
                tick.eq(input_tick)
                    .and(code.eq_any(input_codes))
                    .and(ts.ge(input_start_ts))
                    .and(ts.le(input_end_ts)),
            )
            .order((code.asc(), ts.asc()))
            .load::<StockTickPrice>(&conn)
            .map_err(Error::from)
    })
    .await??;
    Ok(data)
}

pub async fn query_api_prices(
    jq: &JqdataClient,
    tick: &str,
    code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
) -> Result<Vec<jqdata::Price>> {
    let resp = jq
        .execute(GetPricePeriod {
            code: code.to_owned(),
            unit: tick.to_owned(),
            date: start_of_day_str(start_dt),
            end_date: end_of_day_str(end_dt),
            fq_ref_date: None,
        })
        .await?;
    Ok(resp)
}
