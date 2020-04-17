use super::stock_prices::{get_stock_tick_prices, ticks};
use crate::helpers::respond_json;
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::web::Json;
use actix_web::{get, web};
use chrono::NaiveDateTime;
use jqdata::JqdataClient;
use serde::Serialize;
use serde_derive::*;
use tanglism_morph::{ks_to_pts, sks_to_sgs, StrokeConfig, StrokeShaper, K};
use tanglism_morph::{Parting, Segment, Stroke};
use tanglism_utils::{
    parse_ts_from_str, LOCAL_DATES, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    code: String,
    tick: String,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
    data: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Param {
    pub start_dt: String,
    pub end_dt: Option<String>,
    pub indep_k: Option<bool>,
}

#[get("/tanglism/partings/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_partings(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Parting>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices
            .into_iter()
            .map(|p| K {
                ts: p.ts,
                low: p.low,
                high: p.high,
            })
            .collect();
        ks_to_pts(&ks).map_err(|e| e.into())
    })
    .await
}

#[get("/tanglism/strokes/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_strokes(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Stroke>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices
            .into_iter()
            .map(|p| K {
                ts: p.ts,
                low: p.low,
                high: p.high,
            })
            .collect();
        let pts = ks_to_pts(&ks)?;
        // 增加对独立K线的判断
        let cfg = param
            .indep_k
            .map(|indep_k| StrokeConfig { indep_k })
            .unwrap_or_default();
        match path.tick.as_ref() {
            "1m" => StrokeShaper::new(&pts, &*LOCAL_TS_1_MIN, cfg)
                .run()
                .map_err(|e| e.into()),
            "5m" => StrokeShaper::new(&pts, &*LOCAL_TS_5_MIN, cfg)
                .run()
                .map_err(|e| e.into()),
            "30m" => StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, cfg)
                .run()
                .map_err(|e| e.into()),
            "1d" => StrokeShaper::new(&pts, &**LOCAL_DATES, cfg)
                .run()
                .map_err(|e| e.into()),
            _ => Err(Error::custom(
                ErrorKind::BadRequest,
                format!("invalid tick: {}", &path.tick),
            )),
        }
    })
    .await
}

#[get("/tanglism/segments/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_segments(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Segment>>>> {
    get_tanglism_entities(&pool, &jq, &path, &param, |prices| {
        let ks: Vec<K> = prices
            .into_iter()
            .map(|p| K {
                ts: p.ts,
                low: p.low,
                high: p.high,
            })
            .collect();
        let pts = ks_to_pts(&ks)?;
        // 增加对独立K线的判断
        let cfg = param
            .indep_k
            .map(|indep_k| StrokeConfig { indep_k })
            .unwrap_or_default();
        let sks = match path.tick.as_ref() {
            "1m" => StrokeShaper::new(&pts, &*LOCAL_TS_1_MIN, cfg).run()?,
            "5m" => StrokeShaper::new(&pts, &*LOCAL_TS_5_MIN, cfg).run()?,
            "30m" => StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, cfg).run()?,
            "1d" => StrokeShaper::new(&pts, &**LOCAL_DATES, cfg).run()?,
            _ => {
                return Err(Error::custom(
                    ErrorKind::BadRequest,
                    format!("invalid tick: {}", &path.tick),
                ))
            }
        };
        sks_to_sgs(&sks).map_err(Into::into)
    })
    .await
}

async fn get_tanglism_entities<T, F>(
    pool: &DbPool,
    jq: &JqdataClient,
    path: &ticks::Path,
    param: &Param,
    price_fn: F,
) -> Result<Json<Response<T>>>
where
    T: Serialize,
    F: FnOnce(Vec<ticks::StockPrice>) -> Result<T>,
{
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
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}
