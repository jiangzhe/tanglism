mod ema;

use super::stock_prices::get_stock_tick_prices;
use crate::helpers::respond_json;
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::web::Json;
use actix_web::{get, web};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use ema::{approximate_macd, approximate_ema};
use jqdata::JqdataClient;
use serde_derive::*;
use tanglism_utils::{parse_ts_from_str, TradingDates, LOCAL_DATES};

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    code: String,
    tick: String,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
    data: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Path {
    code: String,
    tick: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Param {
    pub start_dt: String,
    pub end_dt: Option<String>,
    pub metrics_cfg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub ts: NaiveDateTime,
    pub value: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdMetric {
    pub fast_ema_period: u32,
    pub slow_ema_period: u32,
    pub dea_period: u32,
    pub dif: Vec<Metric>,
    pub dea: Vec<Metric>,
    pub macd: Vec<Metric>,
}

#[get("/metrics/ema/{code}/ticks/{tick}")]
pub async fn api_get_metrics_ema(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Metric>>>> {
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let ema_period = param
        .metrics_cfg
        .as_ref()
        .and_then(|mc| parse_ema_cfg(mc))
        .unwrap_or(12);
    let search_start_dt = ema_approximate_start(start_ts.date(), &path.tick, ema_period)?;
    let prices = get_stock_tick_prices(
        &pool,
        &jq,
        &path.tick,
        &path.code,
        search_start_dt.and_hms(0, 0, 0),
        end_ts,
    )
    .await?;
    let ema = approximate_ema(&prices, ema_period, |p| p.close.clone(), |p| p.ts);
    assert_eq!(prices.len(), ema.len());
    let data: Vec<Metric> = ema.into_iter().filter(|e| e.ts >= start_ts).collect();
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

fn parse_ema_cfg(s: &str) -> Option<u32> {
    if let Some(cfg) = s.split(',').find(|c| c.starts_with("ema:")) {
        if let Some(num) = cfg.split(':').nth(1) {
            if let Ok(n) = num.parse() {
                return Some(n);
            }
        }
    }
    None
}

#[get("/metrics/macd/{code}/ticks/{tick}")]
pub async fn api_get_metrics_macd(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<MacdMetric>>> {
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let difdea_cfg = match param.metrics_cfg {
        Some(ref s) => parse_difdea_cfg(s),
        None => (None, None, None),
    };
    let fast_ema_period = difdea_cfg.0.unwrap_or(12);
    let slow_ema_period = difdea_cfg.1.unwrap_or(26);
    let dea_period = difdea_cfg.2.unwrap_or(9);
    if slow_ema_period < fast_ema_period || slow_ema_period < dea_period {
        return Err(Error::custom(
            ErrorKind::BadRequest,
            format!(
                "invalid setting: slow ema {} is no less than fast ema {} or dea {}",
                slow_ema_period, fast_ema_period, dea_period
            ),
        ));
    }
    let search_start_dt = ema_approximate_start(start_ts.date(), &path.tick, slow_ema_period)?;
    let prices = get_stock_tick_prices(
        &pool,
        &jq,
        &path.tick,
        &path.code,
        search_start_dt.and_hms(0, 0, 0),
        end_ts,
    )
    .await?;
    let (dif_raw, dea_raw, macd_raw) = approximate_macd(
        &prices,
        fast_ema_period,
        slow_ema_period,
        dea_period,
        |p| p.close.clone(),
        |p| p.ts,
    );
    let dif = dif_raw.into_iter().filter(|d| d.ts >= start_ts).collect();
    let dea = dea_raw.into_iter().filter(|d| d.ts >= start_ts).collect();
    let macd = macd_raw.into_iter().filter(|d| d.ts >= start_ts).collect();
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data: MacdMetric {
            fast_ema_period,
            slow_ema_period,
            dea_period,
            dif,
            dea,
            macd,
        },
    })
}

fn parse_difdea_cfg(s: &str) -> (Option<u32>, Option<u32>, Option<u32>) {
    let mut fast_ema = None;
    let mut slow_ema = None;
    let mut dea = None;
    for c in s.split(',') {
        if c.starts_with("fast_ema:") {
            if let Ok(n) = c[9..].parse() {
                fast_ema = Some(n);
            }
        } else if c.starts_with("slow_ema:") {
            if let Ok(n) = c[9..].parse() {
                slow_ema = Some(n);
            }
        } else if c.starts_with("dea:") {
            if let Ok(n) = c[4..].parse() {
                dea = Some(n);
            }
        }
    }
    (fast_ema, slow_ema, dea)
}

fn ema_approximate_start(start_dt: NaiveDate, tick: &str, period: u32) -> Result<NaiveDate> {
    // 计算额外所需的价格序列的起始区间
    // 3.5 * 周期，之前的价格影响很小
    let total_period = (3.50_f64 * period as f64) as i64;
    let day_factor = match tick {
        "1m" => 240,
        "5m" => 48,
        "30m" => 8,
        "1d" => 1,
        _ => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                format!("invalid tick: {}", tick),
            ))
        }
    };
    let offset_days = (total_period / day_factor + 1) as i64;

    let mut dt = start_dt;
    for _i in 0..offset_days {
        if let Some(prev_dt) = LOCAL_DATES.prev_day(dt) {
            dt = prev_dt;
        } else {
            // 超过边界
            return Err(Error::custom(
                ErrorKind::InternalServerError,
                "exceeds time limit".to_owned(),
            ));
        }
    }
    Ok(dt)
}
