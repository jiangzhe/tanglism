use super::stock_prices::{get_stock_tick_prices, ticks};
use crate::helpers::respond_json;
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::web::Json;
use actix_web::{get, web};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use jqdata::JqdataClient;
use serde::Serialize;
use serde_derive::*;
use std::str::FromStr;
use tanglism_morph::{
    centers_with_auxiliary_segments, ks_to_pts, sks_to_sgs, trend, StrokeBacktrack, StrokeConfig,
    StrokeJudge, StrokeShaper, K,
};
use tanglism_morph::{Center, Parting, Segment, Stroke, SubTrend};
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
    let tick = &path.tick;
    let code = &path.code;
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let prices = get_stock_tick_prices(&pool, &jq, &tick, &code, start_ts, end_ts).await?;
    let data = get_tanglism_partings(&prices)?;
    respond_json(Response {
        code: code.to_owned(),
        tick: tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

#[get("/tanglism/strokes/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_strokes(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Stroke>>>> {
    let tick = &path.tick;
    let code = &path.code;
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let stroke_cfg = match param.stroke_cfg {
        None => StrokeConfig::default(),
        Some(ref s) => parse_stroke_cfg(s)?,
    };
    let prices = get_stock_tick_prices(&pool, &jq, &tick, &code, start_ts, end_ts).await?;
    let partings = get_tanglism_partings(&prices)?;
    let data = get_tanglism_strokes(&partings, tick, stroke_cfg)?;
    respond_json(Response {
        code: code.to_owned(),
        tick: tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

#[get("/tanglism/segments/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_segments(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Segment>>>> {
    let tick = &path.tick;
    let code = &path.code;
    let (start_ts, _) = parse_ts_from_str(&param.start_dt)?;
    let end_ts = match param.end_dt {
        Some(ref s) => {
            let (et, _) = parse_ts_from_str(s)?;
            et
        }
        None => chrono::Local::today().naive_local().and_hms(23, 59, 59),
    };
    let stroke_cfg = match param.stroke_cfg {
        None => StrokeConfig::default(),
        Some(ref s) => parse_stroke_cfg(s)?,
    };
    let prices = get_stock_tick_prices(&pool, &jq, &tick, &code, start_ts, end_ts).await?;
    let partings = get_tanglism_partings(&prices)?;
    let strokes = get_tanglism_strokes(&partings, tick, stroke_cfg)?;
    let data = get_tanglism_segments(&strokes)?;
    respond_json(Response {
        code: code.to_owned(),
        tick: tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

// async fn get_tanglism_subtrends(
//     pool: &DbPool,
//     jq: &JqdataClient,
//     tick: &str,
//     code: &str,
//     start_ts: NaiveDateTime,
//     end_ts: NaiveDateTime,
//     stroke_cfg: StrokeConfig,
// ) -> Result<Vec<SubTrend>> {
//     // 取次级别tick
//     let subtick = match tick {
//         "1d" => "30m",
//         "30m" => "5m",
//         "5m" => "1m",
//         "1m" => {
//             return Err(Error::custom(
//                 ErrorKind::BadRequest,
//                 "tick 1m cannot have subtrends".to_owned(),
//             ))
//         }
//         _ => {
//             return Err(Error::custom(
//                 ErrorKind::BadRequest,
//                 format!("invalid tick: {}", tick),
//             ))
//         }
//     };
//     let prices = get_stock_tick_prices(&pool, &jq, subtick, code, start_ts, end_ts).await?;
//     let partings = get_tanglism_partings(&prices)?;
//     // 使用次级别tick
//     let strokes = get_tanglism_strokes(&partings, subtick, stroke_cfg)?;
//     let segments = get_tanglism_segments(&strokes)?;
//     // 将笔和线段整合为次级别走势
//     let data = trend::merge_subtrends::<_, _, crate::Error>(
//         segments,
//         strokes,
//         |sg| {
//             Ok(SubTrend {
//                 start_ts: align_tick(tick, sg.start_pt.extremum_ts)?,
//                 start_price: sg.start_pt.extremum_price.clone(),
//                 end_ts: align_tick(tick, sg.end_pt.extremum_ts)?,
//                 end_price: sg.end_pt.extremum_price.clone(),
//                 level: 2,
//             })
//         },
//         |sk| {
//             Ok(SubTrend {
//                 start_ts: align_tick(tick, sk.start_pt.extremum_ts)?,
//                 start_price: sk.start_pt.extremum_price.clone(),
//                 end_ts: align_tick(tick, sk.end_pt.extremum_ts)?,
//                 end_price: sk.end_pt.extremum_price.clone(),
//                 level: 1,
//             })
//         },
//     )?;
//     Ok(data)
// }

#[get("/tanglism/subtrends/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_subtrends(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<SubTrend>>>> {
    let tick = path.tick.as_ref();
    let code = &path.code;
    // 取次级别tick
    let subtick = match tick {
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
                format!("invalid tick: {}", tick),
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
    // 增加成笔逻辑判断
    let stroke_cfg = match param.stroke_cfg {
        None => StrokeConfig::default(),
        Some(ref s) => parse_stroke_cfg(s)?,
    };
    let prices = get_stock_tick_prices(&pool, &jq, &subtick, &code, start_ts, end_ts).await?;
    let partings = get_tanglism_partings(&prices)?;
    let strokes = get_tanglism_strokes(&partings, subtick, stroke_cfg)?;
    let segments = get_tanglism_segments(&strokes)?;
    let data = get_tanglism_subtrends(&segments, &strokes, &tick)?;
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

#[get("/tanglism/centers/{code}/ticks/{tick}")]
pub async fn api_get_tanglism_centers(
    pool: web::Data<DbPool>,
    jq: web::Data<JqdataClient>,
    path: web::Path<ticks::Path>,
    param: web::Query<Param>,
) -> Result<Json<Response<Vec<Center>>>> {
    let tick = path.tick.as_ref();
    let code = &path.code;
    // 取次级别tick
    let subtick = match tick {
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
                format!("invalid tick: {}", tick),
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
    let stroke_cfg = match param.stroke_cfg {
        None => StrokeConfig::default(),
        Some(ref s) => parse_stroke_cfg(s)?,
    };
    // 当前级别线段
    let segments = {
        let prices = get_stock_tick_prices(&pool, &jq, &tick, &code, start_ts, end_ts).await?;
        let partings = get_tanglism_partings(&prices)?;
        let strokes = get_tanglism_strokes(&partings, tick, stroke_cfg.clone())?;
        get_tanglism_segments(&strokes)?
    };
    // 次级别走势
    let subtrends = {
        let prices = get_stock_tick_prices(&pool, &jq, &subtick, &code, start_ts, end_ts).await?;
        let partings = get_tanglism_partings(&prices)?;
        let strokes = get_tanglism_strokes(&partings, subtick, stroke_cfg)?;
        let segments = get_tanglism_segments(&strokes)?;
        get_tanglism_subtrends(&segments, &strokes, &tick)?
    };
    let data = centers_with_auxiliary_segments(&subtrends, 2, &segments);
    respond_json(Response {
        code: path.code.to_owned(),
        tick: path.tick.to_owned(),
        start_ts,
        end_ts,
        data,
    })
}

fn get_tanglism_partings(prices: &[ticks::StockPrice]) -> Result<Vec<Parting>> {
    let ks: Vec<K> = prices
        .into_iter()
        .map(|p| K {
            ts: p.ts,
            low: p.low.clone(),
            high: p.high.clone(),
        })
        .collect();
    ks_to_pts(&ks).map_err(|e| e.into())
}

fn get_tanglism_strokes(
    pts: &[Parting],
    tick: &str,
    stroke_cfg: StrokeConfig,
) -> Result<Vec<Stroke>> {
    let data = match tick {
        "1m" => StrokeShaper::new(&pts, &*LOCAL_TS_1_MIN, stroke_cfg).run()?,
        "5m" => StrokeShaper::new(&pts, &*LOCAL_TS_5_MIN, stroke_cfg).run()?,
        "30m" => StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, stroke_cfg).run()?,
        "1d" => StrokeShaper::new(&pts, &**LOCAL_DATES, stroke_cfg).run()?,
        _ => {
            return Err(Error::custom(
                ErrorKind::BadRequest,
                format!("invalid tick: {}", &tick),
            ))
        }
    };
    Ok(data)
}

fn get_tanglism_segments(sks: &[Stroke]) -> Result<Vec<Segment>> {
    sks_to_sgs(&sks).map_err(Into::into)
}

fn get_tanglism_subtrends(
    segments: &[Segment],
    strokes: &[Stroke],
    tick: &str,
) -> Result<Vec<SubTrend>> {
    let data = trend::merge_subtrends::<_, _, crate::Error>(
        segments,
        strokes,
        |sg| {
            Ok(SubTrend {
                start_ts: align_tick(tick, sg.start_pt.extremum_ts)?,
                start_price: sg.start_pt.extremum_price.clone(),
                end_ts: align_tick(tick, sg.end_pt.extremum_ts)?,
                end_price: sg.end_pt.extremum_price.clone(),
                level: 2,
            })
        },
        |sk| {
            Ok(SubTrend {
                start_ts: align_tick(tick, sk.start_pt.extremum_ts)?,
                start_price: sk.start_pt.extremum_price.clone(),
                end_ts: align_tick(tick, sk.end_pt.extremum_ts)?,
                end_price: sk.end_pt.extremum_price.clone(),
                level: 1,
            })
        },
    )?;
    Ok(data)
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
    if s.is_empty() {
        return Ok(StrokeConfig::default());
    }
    let cfg_strs: Vec<&str> = s.split(',').collect();
    let mut indep_k = true;
    let mut judge = StrokeJudge::None;
    let mut backtrack = StrokeBacktrack::None;
    for c in &cfg_strs {
        if c.starts_with("indep_k") {
            let is: Vec<&str> = c.split(":").collect();
            if is.len() == 2 && is[1] == "false" {
                indep_k = false;
            }
        } else if c.starts_with("gap_opening") {
            let gs: Vec<&str> = c.split(':').collect();
            if gs.len() < 2 || gs[1] == "morning" {
                judge = StrokeJudge::GapOpening(false);
            } else {
                judge = StrokeJudge::GapOpening(true);
            }
        } else if c.starts_with("gap_ratio") {
            let gs: Vec<&str> = c.split(':').collect();
            if gs.len() < 2 {
                judge = StrokeJudge::GapRatio(BigDecimal::from_str("0.01").unwrap());
            } else {
                let ratio = BigDecimal::from_str(gs[1]).map_err(|_| {
                    Error::custom(
                        ErrorKind::BadRequest,
                        format!("invalid gap ratio: {}", gs[1]),
                    )
                })?;
                judge = StrokeJudge::GapRatio(ratio);
            }
        } else if c.starts_with("backtrack") {
            let bs: Vec<&str> = c.split(":").collect();
            if bs.len() < 2 {
                backtrack = StrokeBacktrack::Diff(BigDecimal::from_str("0.01").unwrap());
            } else {
                let diff = BigDecimal::from_str(bs[1]).map_err(|_| {
                    Error::custom(
                        ErrorKind::BadRequest,
                        format!("invalid backtrack diff ratio: {}", bs[1]),
                    )
                })?;
                backtrack = StrokeBacktrack::Diff(diff);
            }
        }
    }
    Ok(StrokeConfig {
        indep_k,
        judge,
        backtrack,
    })
}
