use crate::{DbPool, Result, Error, ErrorKind};
use serde_derive::*;
use actix_web::{get, web};
use actix_web::web::Json;
use tanglism_morph::{Parting, Stroke, Segment};
use chrono::NaiveDateTime;
use jqdata::JqdataClient;
use tanglism_morph::{K, ks_to_pts, pts_to_sks, sks_to_sgs};
use super::stock_prices::{ticks, get_stock_tick_prices};
use crate::helpers::respond_json;
use tanglism_utils::{parse_ts_from_str, LOCAL_TS_1_MIN, LOCAL_TS_5_MIN, LOCAL_TS_30_MIN};
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    code: String,
    tick: String,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
    data: T,
}

#[get("/tanglism/partings/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_partings(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<ticks::Param>,
) -> Result<Json<Response<Vec<Parting>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices.into_iter().map(|p| K{
            ts: p.ts,
            low: p.low,
            high: p.high,
        }).collect();
        ks_to_pts(&ks).map_err(|e| e.into())
    }).await
}

#[get("/tanglism/strokes/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_strokes(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<ticks::Param>,
) -> Result<Json<Response<Vec<Stroke>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices.into_iter().map(|p| K{
            ts: p.ts,
            low: p.low,
            high: p.high,
        }).collect();
        let pts = ks_to_pts(&ks)?;
        match path.tick.as_ref() {
            "1m" => pts_to_sks(&pts, &*LOCAL_TS_1_MIN).map_err(|e| e.into()),
            "5m" => pts_to_sks(&pts, &*LOCAL_TS_5_MIN).map_err(|e| e.into()),
            "30m" => pts_to_sks(&pts, &*LOCAL_TS_30_MIN).map_err(|e| e.into()),
            _ => Err(Error::custom(ErrorKind::BadRequest, format!("invalid tick: {}", &path.tick))),
        }
    }).await
}

#[get("/tanglism/segments/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_segments(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<ticks::Param>,
) -> Result<Json<Response<Vec<Segment>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices.into_iter().map(|p| K{
            ts: p.ts,
            low: p.low,
            high: p.high,
        }).collect();
        let pts = ks_to_pts(&ks)?;
        let sks = match path.tick.as_ref() {
            "1m" => pts_to_sks(&pts, &*LOCAL_TS_1_MIN)?,
            "5m" => pts_to_sks(&pts, &*LOCAL_TS_5_MIN)?,
            "30m" => pts_to_sks(&pts, &*LOCAL_TS_30_MIN)?,
            _ => return Err(Error::custom(ErrorKind::BadRequest, format!("invalid tick: {}", &path.tick))),
        };
        sks_to_sgs(&sks).map_err(Into::into)
    }).await
}


async fn get_tanglism_entities<T, F>(
    pool: &DbPool,
    jq: &JqdataClient,
    path: &ticks::Path,
    param: &ticks::Param,
    price_fn: F,
) -> Result<Json<Response<T>>> 
where
    T: Serialize,
    F: FnOnce(Vec<ticks::StockPrice>) -> Result<T> {
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let prices = get_stock_tick_prices(pool, jq, &path.tick, &path.code, start_ts, end_ts).await?;
    let data = price_fn(prices)?;
    respond_json(Response{
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts: start_ts,
        end_ts: end_ts,
        data,
    })
}

