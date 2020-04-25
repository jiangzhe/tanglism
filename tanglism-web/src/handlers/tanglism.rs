use super::stock_prices::{get_stock_tick_prices, ticks};
use crate::helpers::respond_json;
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::web::Json;
use actix_web::{get, web};
use chrono::NaiveDateTime;
use jqdata::JqdataClient;
use serde::Serialize;
use serde_derive::*;
use tanglism_morph::{ks_to_pts, sks_to_sgs, StrokeJudge, StrokeBacktrack, StrokeConfig, StrokeShaper, K};
use tanglism_morph::{Parting, Segment, Stroke, SubTrend};
use tanglism_utils::{
    parse_ts_from_str, LOCAL_DATES, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN,
};
use bigdecimal::BigDecimal;
use std::str::FromStr;

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
    // 成笔的三种逻辑，默认使用独立K线成笔
    // 1. indep_k=true/false 包含1独立K线/不包含独立K线
    // 2. gap_opening=morning/all 开盘跳空/包含午盘
    // 3. gap_ratio=0.01/.../0.10 缺口比例大于指定值
    pub stroke_cfg: Option<String>,
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
        // 增加成笔逻辑判断
        let cfg = match param.stroke_cfg {
            None => StrokeConfig::default(),
            Some(ref s) => parse_stroke_cfg(s)?,
        };
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
        // 增加成笔逻辑判断
        let cfg = match param.stroke_cfg {
            None => StrokeConfig::default(),
            Some(ref s) => parse_stroke_cfg(s)?,
        };
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

#[get("/tanglism/subtrends/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_subtrends(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<SubTrend>>>> {
    // 取次级别tick
    let subtick = match path.tick.as_ref() {
        "1d" => "30m",
        "30m" => "5m",
        "5m" => "1m",
        "1m" => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                "tick 1m cannot have subtrends".to_owned(),
            ))
        }
        _ => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                format!("invalid tick: {}", &path.tick),
            ))
        }
    };
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let prices = get_stock_tick_prices(&pool, &jq, subtick, &path.code, start_ts, end_ts).await?;
    let data = {
        // 获取K线
        let ks: Vec<K> = prices
            .into_iter()
            .map(|p| K {
                ts: p.ts,
                low: p.low,
                high: p.high,
            })
            .collect();
        // 获取分型
        let pts = ks_to_pts(&ks)?;
        // 增加成笔逻辑判断
        let cfg = match param.stroke_cfg {
            None => StrokeConfig::default(),
            Some(ref s) => parse_stroke_cfg(s)?,
        };
        let sks = match subtick {
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
        // 获取线段
        let sgs = sks_to_sgs(&sks)?;
        // 将笔和线段整合为次级别走势
        let tick = path.tick.clone();
        merge_subtrends(
            sgs,
            sks,
            |sg| {
                Ok(SubTrend::Segment {
                    start_ts: align_tick(&tick, sg.start_pt.extremum_ts)?,
                    start_price: sg.start_pt.extremum_price.clone(),
                    end_ts: align_tick(&tick, sg.end_pt.extremum_ts)?,
                    end_price: sg.end_pt.extremum_price.clone(),
                })
            },
            |sk| {
                Ok(SubTrend::Stroke {
                    start_ts: align_tick(&tick, sk.start_pt.extremum_ts)?,
                    start_price: sk.start_pt.extremum_price.clone(),
                    end_ts: align_tick(&tick, sk.end_pt.extremum_ts)?,
                    end_price: sk.end_pt.extremum_price.clone(),
                })
            },
        )?
    };
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
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

fn merge_subtrends<G, K>(
    sgs: Vec<Segment>,
    sks: Vec<Stroke>,
    sg_fn: G,
    sk_fn: K,
) -> Result<Vec<SubTrend>>
where
    G: Fn(&Segment) -> Result<SubTrend>,
    K: Fn(&Stroke) -> Result<SubTrend>,
{
    let mut subtrends = Vec::new();
    let mut sgi = 0;
    let mut ski = 0;
    while sgi < sgs.len() {
        let sg = &sgs[sgi];
        // 将线段前的笔加入次级别走势
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.start_pt.extremum_ts {
            let sk = &sks[ski];
            subtrends.push(sk_fn(sk)?);
            ski += 1;
        }
        // 将线段加入次级别走势
        subtrends.push(sg_fn(sg)?);
        sgi += 1;
        // 跳过所有被线段覆盖的笔
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.end_pt.extremum_ts {
            ski += 1;
        }
    }
    // 将线段后的所有笔加入次级别走势
    while ski < sks.len() {
        let sk = &sks[ski];
        subtrends.push(sk_fn(sk)?);
        ski += 1;
    }
    Ok(subtrends)
}

#[inline]
fn align_tick(tick: &str, ts: NaiveDateTime) -> Result<NaiveDateTime> {
    use tanglism_utils::TradingTimestamps;
    let aligned = match tick {
        "1d" => LOCAL_DATES.aligned_tick(ts),
        "30m" => LOCAL_TS_30_MIN.aligned_tick(ts),
        "5m" => LOCAL_TS_5_MIN.aligned_tick(ts),
        "1m" => LOCAL_TS_1_MIN.aligned_tick(ts),
        _ => {
            return Err(Error::custom(
                ErrorKind::InternalServerError,
                format!("invalid tick: {}", tick),
            ))
        }
    };
    aligned.ok_or_else(|| {
        Error::custom(
            ErrorKind::InternalServerError,
            format!("invalid timestamp: {}", ts),
        )
    })
}


fn parse_stroke_cfg(s: &str) -> Result<StrokeConfig> {
    let cfg_strs: Vec<&str> = s.split(',').collect();
    let mut judge = StrokeJudge::IndepK;
    let mut backtrack = StrokeBacktrack::None;
    for c in &cfg_strs {
        if *c == "indep_k" {
            judge = StrokeJudge::IndepK;
        } else if *c == "non_indep_k" {
            judge = StrokeJudge::NonIndepK;
        } else if c.starts_with("gap_opening") {
            let gs: Vec<&str> = c.split('=').collect();
            if gs.len() < 2 || gs[1] == "morning" {
                judge = StrokeJudge::GapOpening(false);
            } else {
                judge = StrokeJudge::GapOpening(true);
            }
        } else if c.starts_with("gap_ratio") {
            let gs: Vec<&str> = c.split('=').collect();
            if gs.len() < 2 {
                judge = StrokeJudge::GapRatio(BigDecimal::from_str("0.01").unwrap());
            } else {
                let ratio = BigDecimal::from_str(gs[1])
                    .map_err(|_| Error::custom(ErrorKind::BadRequest, format!("invalid gap ratio: {}", gs[1])))?;
                judge = StrokeJudge::GapRatio(ratio);
            }
        } else if c.starts_with("backtrack") {
            let bs: Vec<&str> = c.split("=").collect();
            if bs.len() < 2 {
                backtrack = StrokeBacktrack::Diff(BigDecimal::from_str("0.01").unwrap());
            } else {
                let diff = BigDecimal::from_str(bs[1])
                    .map_err(|_| Error::custom(ErrorKind::BadRequest, format!("invalid backtrack diff ratio: {}", bs[1])))?;
                backtrack = StrokeBacktrack::Diff(diff);
            }
        }
    }
    Ok(StrokeConfig{
        judge,
        backtrack,
    })
}