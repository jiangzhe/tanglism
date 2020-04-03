use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use serde_derive::*;

#[derive(Debug, Deserialize)]
pub struct Path {
    pub code: String,
    pub tick: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Param {
    pub start_dt: NaiveDate,
    pub end_dt: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct Price {
    pub ts: NaiveDateTime,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
}

pub type Response = super::Response<Price>;
