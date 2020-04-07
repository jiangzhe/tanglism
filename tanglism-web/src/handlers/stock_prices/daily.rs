use crate::schema::stock_daily_prices;
use crate::{DbPool, Error, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde_derive::*;
use crate::models::StockPriceTick;
use jqdata::{JqdataClient, GetPricePeriod};
use tanglism_utils::{start_of_day_str, end_of_day_str, parse_date_from_str};
use actix_web::web;

#[derive(Debug, Deserialize)]
pub struct Path {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Param {
    pub start_dt: NaiveDate,
    pub end_dt: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct StockPrice {
    pub dt: NaiveDate,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
}

type StockPriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
);
const STOCK_PRICE_COLUMNS: StockPriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
);

pub type Response = super::Response<StockPrice>;

pub struct StockPriceQuery {
    pub code: String,
    pub start_dt: NaiveDate,
    pub end_dt: NaiveDate,
}

use diesel::prelude::*;
impl StockPriceQuery {
    
    // synchronous db query of prices
    pub fn query_db_prices(&self, pool: &DbPool) -> Result<Vec<StockPrice>> {
        use crate::schema::stock_daily_prices::dsl::*;
        if let Some(conn) = pool.try_get() {
            let data = stock_daily_prices
                .filter(code.eq(&self.code).and(dt.ge(self.start_dt)).and(dt.le(self.end_dt)))
                .order(dt.asc())
                .select(STOCK_PRICE_COLUMNS)
                .load::<StockPrice>(&conn)?;
            return Ok(data);
        }
        Err(Error::FailedAcquireDbConn())
    }

    // synchronous db query of price period
    pub fn query_db_period(&self, pool: &DbPool) -> Result<Option<StockPriceTick>> {
        use crate::schema::stock_price_ticks::dsl::*;
        use diesel::prelude::*;
        if let Some(conn) = pool.try_get() {
            match stock_price_ticks
                .find((&self.code, "1d"))
                .first(&conn)
            {
                Ok(rs) => return Ok(Some(rs)),
                Err(diesel::result::Error::NotFound) => return Ok(None),
                Err(err) => return Err(err.into()),
            }
        }
        Err(Error::FailedAcquireDbConn())
    }

    pub async fn query_api_prices(&self, api: &JqdataClient) -> Result<Vec<StockPrice>> {
        let resp = api.execute(GetPricePeriod{
            code: self.code.to_owned(),
            unit: "1d".to_owned(),
            date: start_of_day_str(self.start_dt),
            end_date: end_of_day_str(self.end_dt),
            fq_ref_date: None,
        }).await?;
        
        let mut data = Vec::new();
        for r in resp.into_iter() {
            let dt = parse_date_from_str(&r.date)?;
            data.push(StockPrice{
                dt,
                open: r.open,
                close: r.close,
                high: r.high,
                low: r.low
            })
        }
        Ok(data)
    }
}

