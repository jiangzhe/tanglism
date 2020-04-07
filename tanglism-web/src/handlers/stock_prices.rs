mod daily;
mod tick;

use crate::helpers::respond_json;
use crate::{DbPool, Error, Result};
use actix_web::get;
use actix_web::web::{self, Json};
use chrono::NaiveDate;
use serde_derive::*;

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
    path: web::Path<daily::Path>,
    web::Query(req): web::Query<daily::Param>,
) -> Result<Json<daily::Response>> {
    let code = path.into_inner().code;
    let end_dt = req
        .end_dt
        .unwrap_or_else(|| chrono::Local::today().naive_local() + chrono::Duration::days(1));
    
    let query = daily::StockPriceQuery{
        code,
        start_dt: req.start_dt,
        end_dt,
    };
    // todo

    let resp = web::block(move || daily::prices(&pool, &code, req.start_dt, end_dt)).await?;
    respond_json(resp)
}

#[get("/stock-prices/{code}/ticks/{tick}")]
pub async fn api_get_stock_tick_prices(
    pool: web::Data<DbPool>,
    path: web::Path<tick::Path>,
    web::Query(req): web::Query<tick::Param>,
) -> Result<Json<tick::Response>> {
    let path = path.into_inner();
    match path.tick.as_ref() {
        "1m" => return Err(Error::OperationNotSupported()),
        "5m" => return Err(Error::OperationNotSupported()),
        "30m" => return Err(Error::OperationNotSupported()),
        _ => return Err(Error::OperationNotSupported()),
    }

    unimplemented!()
}
