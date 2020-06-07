use super::stock_prices::ticks;
use crate::{Error, ErrorKind, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;
use std::str::FromStr;
use tanglism_morph::{
    ks_to_pts, sks_to_sgs, trend, center, StrokeBacktrack, StrokeConfig, StrokeJudge, StrokeShaper, K, TrendConfig,
};
use tanglism_morph::{CenterElement, Parting, Segment, Stroke, SubTrend};
use tanglism_utils::{LOCAL_DATES, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN};

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

pub fn get_tanglism_partings(prices: &[ticks::StockPrice]) -> Result<Vec<Parting>> {
    let ks: Vec<K> = prices
        .iter()
        .map(|p| K {
            ts: p.ts,
            low: p.low.clone(),
            high: p.high.clone(),
        })
        .collect();
    ks_to_pts(&ks).map_err(|e| e.into())
}

pub fn get_tanglism_strokes(
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
            return Err(
                Error::custom(
                ErrorKind::BadRequest,
                format!("invalid tick: {}", &tick),
            ))
        }
    };
    Ok(data)
}

pub fn get_tanglism_segments(sks: &[Stroke]) -> Result<Vec<Segment>> {
    sks_to_sgs(&sks).map_err(Into::into)
}

pub fn get_tanglism_subtrends(
    segments: &[Segment],
    strokes: &[Stroke],
    tick: &str,
) -> Result<Vec<SubTrend>> {
    let data = trend::unify_subtrends(
        segments,
        strokes,
        tick,
    )?;
    Ok(data)
}

pub fn get_tanglism_centers(subtrends: &[SubTrend]) -> Result<Vec<CenterElement>> {
    Ok(center::unify_centers(&subtrends))
}

pub fn parse_stroke_cfg(s: &str) -> Result<StrokeConfig> {
    if s.is_empty() {
        return Ok(StrokeConfig::default());
    }
    let cfg_strs: Vec<&str> = s.split(',').collect();
    let mut indep_k = true;
    let mut judge = StrokeJudge::None;
    let mut backtrack = StrokeBacktrack::None;
    for c in &cfg_strs {
        if c.starts_with("indep_k") {
            let is: Vec<&str> = c.split(':').collect();
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
            let bs: Vec<&str> = c.split(':').collect();
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

pub fn parse_trend_cfg(s: &str) -> Result<TrendConfig> {
    let mut level = 1;
    for c in s.split(',') {
        if c.starts_with("level") {
            let ls: Vec<&str> = c.split(':').collect();
            if ls.len() == 2 {
                if let Ok(lv) = ls[1].parse() {
                    level = lv;
                }
            }
        }
    }
    Ok(TrendConfig{level})
}
