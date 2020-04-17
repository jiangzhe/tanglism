#![allow(dead_code)]

//! 该模块将被废弃

use crate::schema::stock_daily_prices;
use crate::{DbPool, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel::prelude::*;
use jqdata::{GetPricePeriod, JqdataClient};
use serde_derive::*;
use tanglism_utils::{end_of_day_str, start_of_day_str};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Path {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Param {
    pub start_dt: NaiveDate,
    pub end_dt: Option<NaiveDate>,
}

pub type Response = super::Response<StockPrice>;

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct StockPrice {
    pub dt: NaiveDate,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub volume: BigDecimal,
    pub amount: BigDecimal,
}

type StockPriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
    stock_daily_prices::volume,
    stock_daily_prices::amount,
);
const STOCK_PRICE_COLUMNS: StockPriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
    stock_daily_prices::volume,
    stock_daily_prices::amount,
);

pub fn query_db_prices(
    pool: &DbPool,
    input_code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
) -> Result<Vec<StockPrice>> {
    use crate::schema::stock_daily_prices::dsl::*;
    let conn = pool.get()?;
    let data = stock_daily_prices
        .filter(code.eq(input_code).and(dt.ge(start_dt)).and(dt.le(end_dt)))
        .order(dt.asc())
        .select(STOCK_PRICE_COLUMNS)
        .load::<StockPrice>(&conn)?;
    Ok(data)
}

pub async fn query_api_prices(
    jq: &JqdataClient,
    code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
) -> Result<Vec<jqdata::Price>> {
    let resp = jq
        .execute(GetPricePeriod {
            code: code.to_owned(),
            unit: "1d".to_owned(),
            date: start_of_day_str(start_dt),
            end_date: end_of_day_str(end_dt),
            fq_ref_date: None,
        })
        .await?;
    Ok(resp)
}
