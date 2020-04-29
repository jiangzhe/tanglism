//! 走势类型
//!
//! 缠论的基础概念
//!
//! 分为趋势和盘整
//! 趋势由至少2个没有价格区间重叠的中枢构成，趋势向上则为上涨，趋势向下则为下跌
//! 盘整由1个中枢构成
//!
//! 走势分解定理一：任何级别的任何走势，都可以分解成同级别盘整、下跌与上涨三种走势类型的连接。
//! 走势分解定理二：任何级别的任何走势类型，都至少由三段以上次级别走势类型构成。
//!
//! 目前的实现是直接使用次级别段作为次级别走势，而次级别笔作为次级别以下走势。

use crate::shape::{Segment, Stroke};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;

/// 中枢
///
/// 缠论的基础概念
/// 由至少3个存在重叠区间的次级别走势类型构成。
/// 1分钟K线图中走势类型由线段代替。
/// 1分钟K线图的笔即可视为1分钟“中枢”，极端如20课所说，
/// 连续多天开盘封涨停仍只形成1分钟中枢。
/// 5分钟的中枢由至少3个1分钟级别的线段构成。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Center {
    // 起始时刻
    pub start_ts: NaiveDateTime,
    // 起始价格
    pub start_price: BigDecimal,
    // 结束时刻
    pub end_ts: NaiveDateTime,
    // 结束价格
    pub end_price: BigDecimal,
    // 共享最低点，即所有次级别走势类型的最低点中的最高点
    pub shared_low: BigDecimal,
    // 共享最高点，即所有次级别走势类型的最高点中的最低点
    pub shared_high: BigDecimal,
    // 最低点
    pub low: BigDecimal,
    // 最高点
    pub high: BigDecimal,
    // 中枢扩展
    pub extension: Option<Extension>,
}

/// 中枢扩展
///
/// 先不考虑扩展
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Extension {
    pub end_ts: NaiveDateTime,
    pub n: i32,
}

/// 次级别走势
///
/// 当前实现使用次级别K线图中的线段和笔（次级别以下）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubTrend {
    pub start_ts: NaiveDateTime,
    pub start_price: BigDecimal,
    pub end_ts: NaiveDateTime,
    pub end_price: BigDecimal,
    pub level: i32,
}

impl SubTrend {
    fn sorted(&self) -> (&BigDecimal, &BigDecimal) {
        if &self.start_price < &self.end_price {
            (&self.start_price, &self.end_price)
        } else {
            (&self.end_price, &self.start_price)
        }
    }
}

pub fn merge_subtrends<G, K, E>(
    sgs: Vec<Segment>,
    sks: Vec<Stroke>,
    sg_fn: G,
    sk_fn: K,
) -> Result<Vec<SubTrend>, E>
where
    G: Fn(&Segment) -> Result<SubTrend, E>,
    K: Fn(&Stroke) -> Result<SubTrend, E>,
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

pub fn centers(subtrends: &[SubTrend], base_level: i32) -> Vec<Center> {
    if subtrends.len() < 3 {
        return Vec::new();
    }

    let first_st = subtrends[0].clone();
    let second_st = subtrends[1].clone();
    let third_st = subtrends[2].clone();

    todo!()
}

fn center(s1: &SubTrend, s3: &SubTrend) -> Option<Center> {
    let (s1_min, s1_max) = s1.sorted();
    let (s3_min, s3_max) = s3.sorted();

    if s1_max < s3_min || s1_min > s3_max {
        return None;
    }
    let (low, shared_low) = if s1_min < s3_min {
        (s1_min.clone(), s3_min.clone())
    } else {
        (s3_min.clone(), s1_min.clone())
    };
    let (high, shared_high) = if s1_max > s3_max {
        (s1_max.clone(), s3_max.clone())
    } else {
        (s3_max.clone(), s1_max.clone())
    };

    Some(Center {
        start_ts: s1.start_ts,
        start_price: s1.start_price.clone(),
        end_ts: s3.end_ts,
        end_price: s3.end_price.clone(),
        shared_low,
        shared_high,
        low,
        high,
        extension: None,
    })
}
