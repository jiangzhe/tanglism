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

use crate::shape::{Segment, Stroke, SubTrend, SubTrendType, ValuePoint};
use chrono::NaiveDateTime;
use crate::{Result, Error};

#[derive(Debug, Clone, PartialEq)]
pub struct TrendConfig {
    pub level: u32,
}

// pub fn merge_subtrends<G, K, E>(
//     sgs: &[Segment],
//     sks: &[Stroke],
//     sg_fn: G,
//     sk_fn: K,
// ) -> StdResult<Vec<SubTrend>, E>
// where
//     G: Fn(&Segment) -> StdResult<SubTrend, E>,
//     K: Fn(&Stroke) -> StdResult<SubTrend, E>,
// {
//     let mut subtrends = Vec::new();
//     let mut sgi = 0;
//     let mut ski = 0;
//     while sgi < sgs.len() {
//         let sg = &sgs[sgi];
//         // 将线段前的笔加入次级别走势
//         while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.start_pt.extremum_ts {
//             let sk = &sks[ski];
//             subtrends.push(sk_fn(sk)?);
//             ski += 1;
//         }
//         // 将线段加入次级别走势
//         subtrends.push(sg_fn(sg)?);
//         sgi += 1;
//         // 跳过所有被线段覆盖的笔
//         while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.end_pt.extremum_ts {
//             ski += 1;
//         }
//     }
//     // 将线段后的所有笔加入次级别走势
//     while ski < sks.len() {
//         let sk = &sks[ski];
//         subtrends.push(sk_fn(sk)?);
//         ski += 1;
//     }
//     Ok(subtrends)
// }

/// 将线段与笔对齐为某个周期下的次级别走势
/// 线段直接视为次级别走势
/// 笔需要进行如下判断
/// 连续1笔：如果1笔存在缺口，视为次级别走势，并标记缺口
///          如果不存在缺口，因为该笔前后必为段，则检查前后两段是否可合并为同向的段
///          如不可以，该笔独立成段，并标记分段。
/// 连续至少2笔：只可能存在两种可能，与前一段合并为同向段，与后一段合并为同向段。
pub fn unify_subtrends(sgs: &[Segment], sks: &[Stroke], tick: &str) -> Result<Vec<SubTrend>> {
    let mut subtrends = Vec::new();
    let mut strokes = Vec::new();
    let mut sgi = 0;
    let mut ski = 0;
    while sgi < sgs.len() {
        let sg = &sgs[sgi];
        // 将线段前的笔加入次级别走势
        strokes.clear();
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.start_pt.extremum_ts {
            let sk = &sks[ski];
            strokes.push(sk.clone());
            if accumulate_strokes(&mut subtrends, &strokes, tick)? {
                strokes.clear();
            }
            ski += 1;
        }
        if strokes.is_empty() {
            // 将线段加入次级别走势
            subtrends.push(segment_as_subtrend(sg, tick)?);
        } else if strokes.len() == 1 {
            let sk = strokes.pop().unwrap();
            subtrends.push(stroke_as_subtrend(&sk, tick, SubTrendType::Divider)?);
            subtrends.push(segment_as_subtrend(sg, tick)?);
        } else {
            // 大于1笔，判断其与后段是否同向
            let sk = strokes.first().unwrap();
            let upward = sk.end_pt.extremum_price > sk.start_pt.extremum_price
                && sg.end_pt.extremum_price > sg.start_pt.extremum_price
                && sg.end_pt.extremum_price > sk.start_pt.extremum_price;
            let downward = sk.end_pt.extremum_price < sk.start_pt.extremum_price
                && sg.end_pt.extremum_price < sg.start_pt.extremum_price
                && sg.end_pt.extremum_price < sk.start_pt.extremum_price;
            if upward || downward {
                // 合并插入
                subtrends.push(SubTrend{
                    start: ValuePoint{ts: align_tick(tick, sk.start_pt.extremum_ts)?, value: sk.start_pt.extremum_price.clone()},
                    end: ValuePoint{ts: align_tick(tick, sg.end_pt.extremum_ts)?, value: sg.end_pt.extremum_price.clone()},
                    level: 1,
                    typ: SubTrendType::Combination,
                });
            } else {
                // 忽略笔
                subtrends.push(segment_as_subtrend(sg, tick)?);
            }
        }
        sgi += 1;
        // 跳过所有被线段覆盖的笔
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.end_pt.extremum_ts {
            ski += 1;
        }
    }
    // todo
    Ok(subtrends)
}

fn segment_as_subtrend(sg: &Segment, tick: &str) -> Result<SubTrend> {
    Ok(SubTrend{
        start: ValuePoint{ts: align_tick(tick, sg.start_pt.extremum_ts)?, value: sg.start_pt.extremum_price.clone()},
        end: ValuePoint{ts: align_tick(tick, sg.end_pt.extremum_ts)?, value: sg.end_pt.extremum_price.clone()},
        level: 1,
        typ: SubTrendType::Normal,
    })
}

fn stroke_as_subtrend(sk: &Stroke, tick: &str, typ: SubTrendType) -> Result<SubTrend> {
    Ok(SubTrend{
        start: ValuePoint{ts: align_tick(tick, sk.start_pt.extremum_ts)?, value: sk.start_pt.extremum_price.clone()},
        end: ValuePoint{ts: align_tick(tick, sk.end_pt.extremum_ts)?, value: sk.end_pt.extremum_price.clone()},
        level: 1,
        typ,
    })
}

// 尝试将增量的笔合并进已存在的次级别走势
fn accumulate_strokes(subtrends: &mut Vec<SubTrend>, strokes: &[Stroke], tick: &str) -> Result<bool> {
    if strokes.is_empty() {
        return Ok(false);
    }
    if strokes.len() == 1 {
        // 仅连续1笔
        let sk = strokes.last().unwrap();
        if sk.start_pt.right_gap.is_some() || sk.end_pt.left_gap.is_some() {
            subtrends.push(stroke_as_subtrend(sk, tick, SubTrendType::Gap)?);
            return Ok(true);
        }
    }
    if strokes.len() == 2 {
        let sk = strokes.last().unwrap();
        if let Some(prev_st) = subtrends.last() {
            let upward = prev_st.end.value > prev_st.start.value && sk.end_pt.extremum_price > prev_st.start.value;
            let downward = prev_st.end.value < prev_st.start.value && sk.end_pt.extremum_price < prev_st.start.value;
            if upward || downward {
                let st = subtrends.last_mut().unwrap();
                st.end.ts = align_tick(tick, sk.end_pt.extremum_ts)?;
                st.end.value = sk.end_pt.extremum_price.clone();
                st.typ = SubTrendType::Combination;
                return Ok(true);
            }
        }
    }
    // 不处理2笔以上情况
    Ok(false)
}

#[inline]
fn align_tick(tick: &str, ts: NaiveDateTime) -> Result<NaiveDateTime> {
    use tanglism_utils::{TradingTimestamps, LOCAL_DATES, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN, LOCAL_TS_1_MIN};
    let aligned = match tick {
        "1d" => LOCAL_DATES.aligned_tick(ts),
        "30m" => LOCAL_TS_30_MIN.aligned_tick(ts),
        "5m" => LOCAL_TS_5_MIN.aligned_tick(ts),
        "1m" => LOCAL_TS_1_MIN.aligned_tick(ts),
        _ => {
            return Err(Error(format!("invalid tick: {}", tick)));
        }
    };
    aligned.ok_or_else(|| Error(format!("invalid timestamp: {}", ts)))
}

// /// 仅使用次级别走势判断本级别中枢
// ///
// /// 以下判断逻辑以段指称次级别走势：
// /// 当连续三段重合时，取第一段的前一段，
// /// 若前一段的起点高于候选中枢的区间高点，则中枢必须以向上段作为起始。
// /// 反之，若前一段的起点低于候选中枢的区间高点，则中枢必须以向下段作为起始。
// pub fn merge_centers(subtrends: &[SubTrend], base_level: i32) -> Vec<Center> {
//     if subtrends.len() <= 3 {
//         return Vec::new();
//     }
//     let mut ca = CenterArray::new();
//     // 前三段必不成中枢
//     for (i, s) in subtrends.iter().enumerate().skip(3) {
//         if let Some(lc) = ca.last() {
//             if lc.n >= 9 || i - ca.end_idx >= 5 {
//                 // 前中枢大于等于9段时，不再延伸，而进行新中枢的生成判断
//                 // 相差5段以上时，不再与前中枢进行比较，而是与第一段的前一段进行比较
//                 // 当且仅当前一段的区间包含中枢区间时，才形成中枢
//                 if let Some(cc) = maybe_center(subtrends, i, base_level) {
//                     if let Some(prev_s) = subtrends.get(i - 3) {
//                         let (prev_min, prev_max) = prev_s.sorted();
//                         if prev_min <= &cc.shared_low.value && prev_max >= &cc.shared_high.value {
//                             ca.add_last(i, cc);
//                         }
//                     }
//                 }
//             } else if i - ca.end_idx >= 3 {
//                 // 与前中枢间隔三段或以上，可直接判断新中枢形成
//                 if let Some(cc) = maybe_center(subtrends, i, base_level) {
//                     if (cc.shared_low.value >= lc.shared_high.value && !cc.upward) || (cc.shared_high.value <= lc.shared_low.value && cc.upward) {
//                         ca.add_last(i, cc);
//                     }
//                 }
//             } else {
//                 // 检查前中枢是否延伸
//                 // 使用中枢区间，而不是中枢的最高最低点
//                 // 1. 当前走势的终点落在中枢区间内
//                 // 2. 当前走势跨越整个中枢区间，即最高点高于中枢区间，最低点低于中枢区间
//                 // 3. 且不能跨越走势
//                 if s.end.value >= lc.shared_low.value && s.end.value <= lc.shared_high.value {
//                     // 当前走势的终点落在中枢区间内，则中枢延伸至走势终点
//                     let end_idx = ca.end_idx;
//                     ca.modify_last(i, |lc| {
//                         lc.end = s.end.clone();
//                         lc.n += i - end_idx;
//                     });
//                 } else {
//                     let (s_min, s_max) = s.sorted();
//                     // 当前走势跨越整个中枢区间，即最高点高于中枢区间，最低点低于中枢区间
//                     // 则中枢延伸至走势起点
//                     if s_min <= &lc.shared_low.value && s_max >= &lc.shared_high.value {
//                         let end_idx = ca.end_idx;
//                         ca.modify_last(i, |lc| {
//                             lc.end = s.start.clone();
//                             lc.n += i - end_idx;
//                         });
//                     }
//                 }
//             }
//         } else if let Some(cc) = maybe_center(subtrends, i, base_level) {
//             // 判断新中枢
//             if let Some(prev_s) = subtrends.get(i - 3) {
//                 let (prev_min, prev_max) = prev_s.sorted();
//                 if prev_min <= &cc.shared_low.value && prev_max >= &cc.shared_high.value {
//                     ca.add_last(i, cc);
//                 }
//             }
//         }
//     }
//     ca.cs
// }

// struct CenterArray {
//     cs: Vec<Center>,
//     end_idx: usize,
// }

// impl CenterArray {
//     fn new() -> Self {
//         CenterArray {
//             cs: Vec::new(),
//             end_idx: 0,
//         }
//     }

//     // 保证中枢和索引同时更新
//     fn add_last(&mut self, end_idx: usize, c: Center) {
//         self.end_idx = end_idx;
//         self.cs.push(c);
//     }

//     fn last(&self) -> Option<&Center> {
//         self.cs.last()
//     }

//     fn modify_last<F>(&mut self, end_idx: usize, f: F)
//     where
//         F: Fn(&mut Center),
//     {
//         self.end_idx = end_idx;
//         if let Some(last) = self.cs.last_mut() {
//             f(last);
//         }
//     }
// }

// fn maybe_center(subtrends: &[SubTrend], idx: usize, base_level: i32) -> Option<Center> {
//     if idx < 3 {
//         return None;
//     }
//     center3(
//         &subtrends[idx - 2],
//         &subtrends[idx - 1],
//         &subtrends[idx],
//         base_level,
//     )
// }

// fn center3(s1: &SubTrend, s2: &SubTrend, s3: &SubTrend, base_level: i32) -> Option<Center> {
//     if s1.level == base_level && s2.level == base_level && s3.level == base_level {
//         return center2(s1, s3);
//     }
//     None
// }

// #[inline]
// fn center2(s1: &SubTrend, s3: &SubTrend) -> Option<Center> {
//     assert!(s1.level == s3.level);

//     let (s1_min, s1_max) = s1.sorted_points();
//     let (s3_min, s3_max) = s3.sorted_points();

//     if s1_max.price < s3_min.price || s1_min.price > s3_max.price {
//         return None;
//     }
//     let (low, shared_low) = if s1_min.price < s3_min.price {
//         (s1_min, s3_min)
//     } else {
//         (s3_min, s1_min)
//     };
//     let (high, shared_high) = if s1_max.price > s3_max.price {
//         (s1_max, s3_max)
//     } else {
//         (s3_max, s1_max)
//     };

//     Some(Center {
//         start_ts: s1.start_ts,
//         start_price: s1.start_price.clone(),
//         end_ts: s3.end_ts,
//         end_price: s3.end_price.clone(),
//         shared_low: shared_low.price,
//         shared_high: shared_high.price,
//         low,
//         high,
//         level: s1.level + 1,
//         upward: s1.end_price > s1.start_price,
//         n: 3,
//     })
// }

// #[inline]
// fn abs_diff(d1: &BigDecimal, d2: &BigDecimal) -> BigDecimal {
//     if d1 > d2 {
//         d1 - d2
//     } else {
//         d2 - d1
//     }
// }
