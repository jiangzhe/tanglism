use crate::schema::stock_daily_prices;
use crate::{DbPool, Error, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde_derive::*;
use crate::models::StockPriceTick;
use jqdata::JqdataClient;

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
pub struct Price {
    pub dt: NaiveDate,
    pub open: BigDecimal,
    pub close: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
}

type PriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
);
const PRICE_COLUMNS: PriceColumns = (
    stock_daily_prices::dt,
    stock_daily_prices::open,
    stock_daily_prices::close,
    stock_daily_prices::high,
    stock_daily_prices::low,
);

pub type Response = super::Response<Price>;

pub fn prices(
    pool: &DbPool,
    input_code: &str,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
) -> Result<Response> {
    use crate::schema::stock_daily_prices::dsl::*;
    use diesel::prelude::*;

    if let Some(conn) = pool.try_get() {
        let data = stock_daily_prices
            .filter(code.eq(input_code).and(dt.ge(start_dt)).and(dt.le(end_dt)))
            .order(dt.asc())
            .select(PRICE_COLUMNS)
            .load::<Price>(&conn)?;
        return Ok(Response {
            code: input_code.to_string(),
            tick: "daily".into(),
            start_dt,
            end_dt,
            data: data,
        });
    }
    Err(Error::FailedAcquireDbConn())
}

pub struct PriceQuery {
    pub code: String,
    pub start_dt: NaiveDate,
    pub end_dt: NaiveDate,
}


use diesel::prelude::*;
impl PriceQuery {
    
    // synchronous db query of prices
    pub fn query_db_prices(&self, pool: &DbPool) -> Result<Vec<Price>> {
        use crate::schema::stock_daily_prices::dsl::*;
        if let Some(conn) = pool.try_get() {
            let data = stock_daily_prices
                .filter(code.eq(&self.code).and(dt.ge(self.start_dt)).and(dt.le(self.end_dt)))
                .order(dt.asc())
                .select(PRICE_COLUMNS)
                .load::<Price>(&conn)?;
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

    pub async fn query_api_prices(&self, api: &JqdataClient) -> Result<Vec<Price>> {
        api.execute()
        // todo
    }
}

mod a {
    pub struct GetPricePeriod {
        pub code: String,
        pub unit: String,
        pub date: String,
        pub end_date: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fq_ref_date: Option<String>,
    }
}
