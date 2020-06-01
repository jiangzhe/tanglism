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
use crate::{Result, Error};
use std::result::Result as StdResult;

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
    // 中枢级别
    pub level: i32,
    // 展开幅度
    pub unfolded_range: BigDecimal,
    // 方向，由第一个走势确定
    // 一般的，在趋势中的中枢方向总与趋势相反
    // 即上升时，中枢总是由下上下三段次级别走势构成
    // 盘整时，相邻连个中枢方向不一定一致
    pub upward: bool,
    // 组成该中枢的次级别走势个数
    pub n: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SubTrendType {
    Normal,
    // 由缺口形成的次级别
    Gap,
    // 前后两段被笔破坏
    Divider,
    // 由多条线段组合而成
    Combination,
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
    pub typ: SubTrendType,
}

impl SubTrend {
    fn sorted(&self) -> (&BigDecimal, &BigDecimal) {
        if self.start_price < self.end_price {
            (&self.start_price, &self.end_price)
        } else {
            (&self.end_price, &self.start_price)
        }
    }

    fn combined(&self, end_ts: NaiveDateTime, end_price: &BigDecimal) -> Option<Self> {
        let upward = self.end_price > self.start_price && end_price > &self.start_price;
        let downward = self.end_price < self.start_price && end_price < &self.start_price;
        if upward || downward {
            return Some(SubTrend{
                start_ts: self.start_ts,
                start_price: self.start_price.clone(),
                end_ts: end_ts,
                end_price: end_price.clone(),
                level: 1,
                typ: SubTrendType::Combination,
            });
        }
        None
    }
}

pub fn merge_subtrends<G, K, E>(
    sgs: &[Segment],
    sks: &[Stroke],
    sg_fn: G,
    sk_fn: K,
) -> StdResult<Vec<SubTrend>, E>
where
    G: Fn(&Segment) -> StdResult<SubTrend, E>,
    K: Fn(&Stroke) -> StdResult<SubTrend, E>,
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

/// 将线段与笔对齐为某个周期下的次级别走势
/// 线段直接视为次级别走势
/// 笔需要进行如下判断
/// 连续1笔：如果1笔存在缺口，视为次级别走势，并标记缺口
///          如果不存在缺口，因为该笔前后必为段，则检查前后两段是否可合并为同向的段
///          如不可以，该笔独立成段，并标记分段。
/// 连续至少2笔：只可能存在两种可能，与前一段合并为同向段，与后一段合并为同向段。
pub fn align_subtrends(sgs: &[Segment], sks: &[Stroke], tick: &str) -> Result<Vec<SubTrend>> {
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
                    start_ts: align_tick(tick, sk.start_pt.extremum_ts)?,
                    start_price: sk.start_pt.extremum_price.clone(),
                    end_ts: align_tick(tick, sg.end_pt.extremum_ts)?,
                    end_price: sg.end_pt.extremum_price.clone(),
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
        start_ts: align_tick(tick, sg.start_pt.extremum_ts)?,
        start_price: sg.start_pt.extremum_price.clone(),
        end_ts: align_tick(tick, sg.end_pt.extremum_ts)?,
        end_price: sg.end_pt.extremum_price.clone(),
        level: 1,
        typ: SubTrendType::Normal,
    })
}

fn stroke_as_subtrend(sk: &Stroke, tick: &str, typ: SubTrendType) -> Result<SubTrend> {
    Ok(SubTrend{
        start_ts: align_tick(tick, sk.start_pt.extremum_ts)?,
        start_price: sk.start_pt.extremum_price.clone(),
        end_ts: align_tick(tick, sk.end_pt.extremum_ts)?,
        end_price: sk.end_pt.extremum_price.clone(),
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
            let upward = prev_st.end_price > prev_st.start_price && sk.end_pt.extremum_price > prev_st.start_price;
            let downward = prev_st.end_price < prev_st.start_price && sk.end_pt.extremum_price < prev_st.start_price;
            if upward || downward {
                let st = subtrends.last_mut().unwrap();
                st.end_ts = sk.end_pt.extremum_ts;
                st.end_price = sk.end_pt.extremum_price.clone();
                st.typ = SubTrendType::Combination;
                return Ok(true);
            }
        }
    }
    // 不处理2笔以上情况
    Ok(false)
}

fn combine_strokes(strokes: &[Stroke]) -> Option<SubTrend> {
    if strokes.len() <= 2 {
        return None;
    }
    if let (Some(first), Some(last)) = (strokes.first(), strokes.last()) {
        let upward = first.end_pt.extremum_price > first.start_pt.extremum_price 
            && last.end_pt.extremum_price > last.start_pt.extremum_price
            && last.end_pt.extremum_price > first.start_pt.extremum_price;
        let downward = first.end_pt.extremum_price < first.start_pt.extremum_price
            && last.end_pt.extremum_price < last.start_pt.extremum_price
            && last.end_pt.extremum_price < first.start_pt.extremum_price;
        if upward || downward {
            return Some(SubTrend{
                start_ts: first.start_pt.extremum_ts,
                start_price: first.start_pt.extremum_price.clone(),
                end_ts: last.end_pt.extremum_ts,
                end_price: last.end_pt.extremum_price.clone(),
                level: 1,
                typ: SubTrendType::Combination,
            })
        }
    }
    None
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

/// 仅使用次级别走势判断本级别中枢
///
/// 以下判断逻辑以段指称次级别走势：
/// 当连续三段重合时，取第一段的前一段，
/// 若前一段的起点高于候选中枢的区间高点，则中枢必须以向上段作为起始。
/// 反之，若前一段的起点低于候选中枢的区间高点，则中枢必须以向下段作为起始。
pub fn merge_centers(subtrends: &[SubTrend], base_level: i32) -> Vec<Center> {
    if subtrends.len() <= 3 {
        return Vec::new();
    }
    let mut ca = CenterArray::new();
    // 前三段必不成中枢
    for (i, s) in subtrends.iter().enumerate().skip(3) {
        if let Some(lc) = ca.last() {
            if lc.n >= 9 || i - ca.end_idx >= 5 {
                // 前中枢大于等于9段时，不再延伸，而进行新中枢的生成判断
                // 相差5段以上时，不再与前中枢进行比较，而是与第一段的前一段进行比较
                // 当且仅当前一段的区间包含中枢区间时，才形成中枢
                if let Some(cc) = maybe_center(subtrends, i, base_level) {
                    if let Some(prev_s) = subtrends.get(i - 3) {
                        let (prev_min, prev_max) = prev_s.sorted();
                        if prev_min <= &cc.shared_low && prev_max >= &cc.shared_high {
                            ca.add_last(i, cc);
                        }
                    }
                }
            } else if i - ca.end_idx >= 3 {
                // 与前中枢间隔三段或以上，可直接判断新中枢形成
                if let Some(cc) = maybe_center(subtrends, i, base_level) {
                    if (cc.shared_low >= lc.shared_high && !cc.upward) || (cc.shared_high <= lc.shared_low && cc.upward) {
                        ca.add_last(i, cc);
                    }
                }
            } else {
                // 检查前中枢是否延伸
                // 使用中枢区间，而不是中枢的最高最低点
                // 1. 当前走势的终点落在中枢区间内
                // 2. 当前走势跨越整个中枢区间，即最高点高于中枢区间，最低点低于中枢区间
                // 3. 且不能跨越走势
                if s.end_price >= lc.shared_low && s.end_price <= lc.shared_high {
                    // 当前走势的终点落在中枢区间内，则中枢延伸至走势终点
                    let end_idx = ca.end_idx;
                    ca.modify_last(i, |lc| {
                        lc.end_ts = s.end_ts;
                        lc.end_price = s.end_price.clone();
                        lc.n += i - end_idx;
                    });
                } else {
                    let (s_min, s_max) = s.sorted();
                    // 当前走势跨越整个中枢区间，即最高点高于中枢区间，最低点低于中枢区间
                    // 则中枢延伸至走势起点
                    if s_min <= &lc.shared_low && s_max >= &lc.shared_high {
                        let end_idx = ca.end_idx;
                        ca.modify_last(i, |lc| {
                            lc.end_ts = s.start_ts;
                            lc.end_price = s.start_price.clone();
                            lc.n += i - end_idx;
                        });
                    }
                }
            }
        } else if let Some(cc) = maybe_center(subtrends, i, base_level) {
            // 判断新中枢
            if let Some(prev_s) = subtrends.get(i - 3) {
                let (prev_min, prev_max) = prev_s.sorted();
                if prev_min <= &cc.shared_low && prev_max >= &cc.shared_high {
                    ca.add_last(i, cc);
                }
            }
        }
    }
    ca.cs
}

struct CenterArray {
    cs: Vec<Center>,
    end_idx: usize,
}

impl CenterArray {
    fn new() -> Self {
        CenterArray {
            cs: Vec::new(),
            end_idx: 0,
        }
    }

    // 保证中枢和索引同时更新
    fn add_last(&mut self, end_idx: usize, c: Center) {
        self.end_idx = end_idx;
        self.cs.push(c);
    }

    fn last(&self) -> Option<&Center> {
        self.cs.last()
    }

    fn modify_last<F>(&mut self, end_idx: usize, f: F)
    where
        F: Fn(&mut Center),
    {
        self.end_idx = end_idx;
        if let Some(last) = self.cs.last_mut() {
            f(last);
        }
    }
}

fn maybe_center(subtrends: &[SubTrend], idx: usize, base_level: i32) -> Option<Center> {
    if idx < 3 {
        return None;
    }
    center3(
        &subtrends[idx - 2],
        &subtrends[idx - 1],
        &subtrends[idx],
        base_level,
    )
}

fn center3(s1: &SubTrend, s2: &SubTrend, s3: &SubTrend, base_level: i32) -> Option<Center> {
    if s1.level == base_level && s2.level == base_level && s3.level == base_level {
        return center2(s1, s3);
    }
    None
}

#[inline]
fn center2(s1: &SubTrend, s3: &SubTrend) -> Option<Center> {
    assert!(s1.level == s3.level);

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
        level: s1.level + 1,
        unfolded_range: abs_diff(&s1.start_price, &s1.end_price)
            + abs_diff(&s1.end_price, &s3.start_price)
            + abs_diff(&s3.start_price, &s3.end_price),
        upward: s1.end_price > s1.start_price,
        n: 3,
    })
}

#[inline]
fn abs_diff(d1: &BigDecimal, d2: &BigDecimal) -> BigDecimal {
    if d1 > d2 {
        d1 - d2
    } else {
        d2 - d1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;

    #[test]
    fn test_center2_single() {
        let s1 = SubTrend {
            start_ts: new_ts("2020-02-10 15:00"),
            start_price: BigDecimal::from(10),
            end_ts: new_ts("2020-02-11 15:00"),
            end_price: BigDecimal::from(11),
            level: 1,
            typ: SubTrendType::Normal,
        };
        let s3 = SubTrend {
            start_ts: new_ts("2020-02-12 15:00"),
            start_price: BigDecimal::from(10.5),
            end_ts: new_ts("2020-02-13 15:00"),
            end_price: BigDecimal::from(11.5),
            level: 1,
            typ: SubTrendType::Normal,
        };
        let c = center2(&s1, &s3).unwrap();
        assert_eq!(new_ts("2020-02-10 15:00"), c.start_ts);
        assert_eq!(BigDecimal::from(10), c.start_price);
        assert_eq!(new_ts("2020-02-13 15:00"), c.end_ts);
        assert_eq!(BigDecimal::from(11.5), c.end_price);
        assert_eq!(BigDecimal::from(10.5), c.shared_low);
        assert_eq!(BigDecimal::from(11), c.shared_high);
        assert_eq!(BigDecimal::from(10), c.low);
        assert_eq!(BigDecimal::from(11.5), c.high);
        assert_eq!(2, c.level);
    }

    #[test]
    fn test_center2_narrow() {
        let s1 = SubTrend {
            start_ts: new_ts("2020-02-10 15:00"),
            start_price: BigDecimal::from(15),
            end_ts: new_ts("2020-02-11 15:00"),
            end_price: BigDecimal::from(15.5),
            level: 1,
            typ: SubTrendType::Normal,
        };
        let s3 = SubTrend {
            start_ts: new_ts("2020-02-12 15:00"),
            start_price: BigDecimal::from(14.5),
            end_ts: new_ts("2020-02-13 15:00"),
            end_price: BigDecimal::from(15.2),
            level: 1,
            typ: SubTrendType::Normal,
        };
        let c = center2(&s1, &s3).unwrap();
        assert_eq!(new_ts("2020-02-10 15:00"), c.start_ts);
        assert_eq!(BigDecimal::from(15), c.start_price);
        assert_eq!(new_ts("2020-02-13 15:00"), c.end_ts);
        assert_eq!(BigDecimal::from(15.2), c.end_price);
        assert_eq!(BigDecimal::from(15), c.shared_low);
        assert_eq!(BigDecimal::from(15.2), c.shared_high);
        assert_eq!(BigDecimal::from(14.5), c.low);
        assert_eq!(BigDecimal::from(15.5), c.high);
        assert_eq!(2, c.level);
    }

    #[test]
    fn test_center2_none() {
        let s1 = SubTrend {
            start_ts: new_ts("2020-02-10 15:00"),
            start_price: BigDecimal::from(10),
            end_ts: new_ts("2020-02-11 15:00"),
            end_price: BigDecimal::from(10.2),
            level: 1,
            typ: SubTrendType::Normal,
        };
        let s3 = SubTrend {
            start_ts: new_ts("2020-02-12 15:00"),
            start_price: BigDecimal::from(9.5),
            end_ts: new_ts("2020-02-13 15:00"),
            end_price: BigDecimal::from(9.8),
            level: 1,
            typ: SubTrendType::Normal,
        };
        assert!(center2(&s1, &s3).is_none());
    }

    #[test]
    fn test_centers_none() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(0, cs.len());
    }

    #[test]
    fn test_centers_none_with_no_overlap() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-07 15:00"),
                start_price: BigDecimal::from(10.2),
                end_ts: new_ts("2020-02-10 15:00"),
                end_price: BigDecimal::from(10),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(0, cs.len());
    }

    #[test]
    fn test_centers_single() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-07 15:00"),
                start_price: BigDecimal::from(13),
                end_ts: new_ts("2020-02-10 15:00"),
                end_price: BigDecimal::from(10),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(1, cs.len());
        assert_eq!(BigDecimal::from(10.5), cs[0].shared_low);
        assert_eq!(BigDecimal::from(11), cs[0].shared_high);
    }

    #[test]
    fn test_centers_double() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-07 15:00"),
                start_price: BigDecimal::from(13),
                end_ts: new_ts("2020-02-10 15:00"),
                end_price: BigDecimal::from(10),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-13 15:00"),
                start_price: BigDecimal::from(11.5),
                end_ts: new_ts("2020-02-18 15:00"),
                end_price: BigDecimal::from(8),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-18 15:00"),
                start_price: BigDecimal::from(8),
                end_ts: new_ts("2020-02-19 15:00"),
                end_price: BigDecimal::from(8.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-19 15:00"),
                start_price: BigDecimal::from(8.5),
                end_ts: new_ts("2020-02-20 15:00"),
                end_price: BigDecimal::from(8.2),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-20 15:00"),
                start_price: BigDecimal::from(8.2),
                end_ts: new_ts("2020-02-21 15:00"),
                end_price: BigDecimal::from(9.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(2, cs.len());
        assert_eq!(new_ts("2020-02-18 15:00"), cs[1].start_ts);
        assert_eq!(BigDecimal::from(8), cs[1].start_price);
        assert_eq!(new_ts("2020-02-21 15:00"), cs[1].end_ts);
        assert_eq!(BigDecimal::from(9.5), cs[1].end_price);
        assert_eq!(BigDecimal::from(8.2), cs[1].shared_low);
        assert_eq!(BigDecimal::from(8.5), cs[1].shared_high);
        assert_eq!(BigDecimal::from(8.0), cs[1].low);
        assert_eq!(BigDecimal::from(9.5), cs[1].high);
    }

    #[test]
    fn test_centers_extension() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-07 15:00"),
                start_price: BigDecimal::from(13),
                end_ts: new_ts("2020-02-10 15:00"),
                end_price: BigDecimal::from(10),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-13 15:00"),
                start_price: BigDecimal::from(11.5),
                end_ts: new_ts("2020-02-18 15:00"),
                end_price: BigDecimal::from(10.8),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(1, cs.len());
        assert_eq!(new_ts("2020-02-10 15:00"), cs[0].start_ts);
        assert_eq!(new_ts("2020-02-18 15:00"), cs[0].end_ts);
    }

    #[test]
    fn test_centers_extension_through() {
        let sts = vec![
            SubTrend {
                start_ts: new_ts("2020-02-07 15:00"),
                start_price: BigDecimal::from(13),
                end_ts: new_ts("2020-02-10 15:00"),
                end_price: BigDecimal::from(10),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-10 15:00"),
                start_price: BigDecimal::from(10),
                end_ts: new_ts("2020-02-11 15:00"),
                end_price: BigDecimal::from(11),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-11 15:00"),
                start_price: BigDecimal::from(11),
                end_ts: new_ts("2020-02-12 15:00"),
                end_price: BigDecimal::from(10.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-12 15:00"),
                start_price: BigDecimal::from(10.5),
                end_ts: new_ts("2020-02-13 15:00"),
                end_price: BigDecimal::from(11.5),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-13 15:00"),
                start_price: BigDecimal::from(11.5),
                end_ts: new_ts("2020-02-18 15:00"),
                end_price: BigDecimal::from(9),
                level: 1,
                typ: SubTrendType::Normal,
            },
            SubTrend {
                start_ts: new_ts("2020-02-18 15:00"),
                start_price: BigDecimal::from(9),
                end_ts: new_ts("2020-02-19 15:00"),
                end_price: BigDecimal::from(12),
                level: 1,
                typ: SubTrendType::Normal,
            },
        ];
        let cs = merge_centers(&sts, 1);
        assert_eq!(1, cs.len());
        assert_eq!(new_ts("2020-02-10 15:00"), cs[0].start_ts);
        assert_eq!(new_ts("2020-02-18 15:00"), cs[0].end_ts);
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
