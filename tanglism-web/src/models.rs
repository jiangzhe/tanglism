use crate::schema::{stock_daily_prices, stock_price_ticks};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct TradeDay {
    dt: NaiveDate,
}

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct Security {
    code: String,
    display_name: String,
    name: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    tp: String,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Insertable, Identifiable)]
#[primary_key(code, tick)]
pub struct StockPriceTick {
    code: String,
    tick: String,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
}

#[derive(Debug, Queryable, Insertable, Identifiable)]
#[primary_key(code, dt)]
pub struct StockDailyPrice {
    code: String,
    dt: NaiveDate,
    open: BigDecimal,
    close: BigDecimal,
    high: BigDecimal,
    low: BigDecimal,
    volume: BigDecimal,
    amount: BigDecimal,
}
