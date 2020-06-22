use super::stock_prices::ticks;
use crate::{Error, ErrorKind, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;
use std::str::FromStr;
use tanglism_morph::{
    ks_to_pts, pts_to_sks, sks_to_sgs, trend_as_subtrend, unify_centers, unify_subtrends,
    unify_trends, StrokeConfig, StrokeJudge, TrendConfig, K,
};
use tanglism_morph::{CenterElement, Parting, Segment, Stroke, SubTrend, Trend};

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
    pts_to_sks(pts, tick, stroke_cfg).map_err(Into::into)
}

pub fn get_tanglism_segments(sks: &[Stroke]) -> Result<Vec<Segment>> {
    sks_to_sgs(&sks).map_err(Into::into)
}

// segments and strokes must be 1m ticked
pub fn get_tanglism_subtrends(
    segments: &[Segment],
    strokes: &[Stroke],
    tick: &str,
    level: i32,
) -> Result<Vec<SubTrend>> {
    if level < 1 {
        return Err(Error::custom(
            ErrorKind::BadRequest,
            "minimal level is 1".to_owned(),
        ));
    }
    if level == 1 {
        let subtrends = unify_subtrends(segments, strokes, tick)?;
        return Ok(subtrends);
    }
    log::debug!("unify subtrends with level {}", level);
    let mut subtrends = unify_subtrends(segments, strokes, "1m")?;
    for lv in 2..=level {
        let centers = unify_centers(&subtrends);
        let trends = unify_trends(&centers);
        subtrends.clear();
        for tr in &trends {
            subtrends.push(trend_as_subtrend(
                tr,
                if lv == level { tick } else { "1m" },
            )?);
        }
    }
    Ok(subtrends)
}

pub fn get_tanglism_centers(subtrends: &[SubTrend]) -> Result<Vec<CenterElement>> {
    Ok(unify_centers(&subtrends))
}

pub fn get_tanglism_trends(centers: &[CenterElement]) -> Result<Vec<Trend>> {
    Ok(unify_trends(&centers))
}

pub fn parse_stroke_cfg(s: &str) -> Result<StrokeConfig> {
    if s.is_empty() {
        return Ok(StrokeConfig::default());
    }
    let cfg_strs: Vec<&str> = s.split(',').collect();
    let mut indep_k = true;
    let mut judge = StrokeJudge::None;
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
        }
    }
    Ok(StrokeConfig { indep_k, judge })
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
    Ok(TrendConfig { level })
}
