use crate::schema::{stock_daily_prices, stock_price_ticks, stock_tick_prices};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct TradeDay {
    pub dt: NaiveDate,
}

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct Security {
    pub code: String,
    pub display_name: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub tp: String,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Insertable, Identifiable)]
#[primary_key(code, tick)]
pub struct StockPriceTick {
    pub tick: String,
    pub code: String,
    pub start_dt: NaiveDate,
    pub end_dt: NaiveDate,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Insertable, Identifiable)]
#[primary_key(code, dt)]
pub struct StockDailyPrice {
    pub code: String,
    pub dt: NaiveDate,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub volume: BigDecimal,
    pub amount: BigDecimal,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Insertable, Identifiable)]
#[primary_key(tick, code, ts)]
pub struct StockTickPrice {
    pub tick: String,
    pub code: String,
    pub ts: NaiveDateTime,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub volume: BigDecimal,
    pub amount: BigDecimal,
}
