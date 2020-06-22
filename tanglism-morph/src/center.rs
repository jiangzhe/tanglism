use crate::shape::{Center, CenterElement, SemiCenter, SubTrend};
use bigdecimal::BigDecimal;

/// 临时元素
///
/// 用于标记尚未完成的中枢元素序列
#[derive(Debug, Clone)]
enum TemporaryElement {
    Center(TemporaryCenter),
    SubTrend(TemporarySubTrend),
    // 类中枢可以考虑共享开头和结束的一个次级别走势 todo
    SemiCenter(TemporarySemiCenter),
}

#[derive(Debug, Clone)]
struct TemporaryCenter {
    //起始三段的下标
    start_idx: usize,
    end_idx: usize,
    // 延伸的次级别走势数
    extended_subtrends: usize,
    // 是否不可移动
    fixed: bool,
}

impl TemporaryCenter {
    fn last_end_idx(&self) -> usize {
        self.end_idx + self.extended_subtrends
    }
}

#[derive(Debug, Clone)]
struct TemporarySubTrend {
    idx: usize,
    // 是否紧邻一个类中枢
    beside_semi: bool,
}

#[derive(Debug, Clone)]
struct TemporarySemiCenter {
    // 起始三段的下标
    start_idx: usize,
    end_idx: usize,
    // 保持类中枢的次级别走势数
    extended_subtrends: usize,
    shared_start: bool,
}

impl TemporarySemiCenter {
    #[allow(dead_code)]
    fn last_start_idx(&self) -> usize {
        self.start_idx + self.extended_subtrends
    }

    fn last_end_idx(&self) -> usize {
        self.end_idx + self.extended_subtrends
    }
}

pub fn unify_centers(subtrends: &[SubTrend]) -> Vec<CenterElement> {
    let standard = Standard::new();
    standard.aggregate(subtrends)
}

/// 中枢策略
///
/// 将次级别走势转化为中枢元素序列。
/// 可使用不同测策略分析
pub trait CenterStrategy {
    fn aggregate(self, subtrends: &[SubTrend]) -> Vec<CenterElement>;
}

struct Standard {
    tmp: Vec<TemporaryElement>,
}

impl CenterStrategy for Standard {
    fn aggregate(mut self, subtrends: &[SubTrend]) -> Vec<CenterElement> {
        for idx in 0..subtrends.len() {
            self.accumulate(subtrends, idx);
        }
        self.centers(subtrends)
    }
}

impl Standard {
    fn new() -> Self {
        Standard { tmp: Vec::new() }
    }

    #[inline]
    fn accumulate(&mut self, subtrends: &[SubTrend], idx: usize) {
        if let Some(elem) = self.last1() {
            match elem {
                TemporaryElement::Center(tc) => {
                    let tc = tc.clone();
                    self.accumulate_after_center(subtrends, idx, tc);
                }
                TemporaryElement::SubTrend(_) => {
                    self.accumulate_after_subtrend(subtrends, idx);
                }
                TemporaryElement::SemiCenter(tsc) => {
                    let tsc = tsc.clone();
                    self.accumulate_after_semicenter(subtrends, idx, tsc);
                }
            }
            return;
        }

        // 第一个元素
        self.push_subtrend(idx, false);
    }

    fn centers(self, subtrends: &[SubTrend]) -> Vec<CenterElement> {
        self.tmp
            .into_iter()
            .map(|te| match te {
                TemporaryElement::Center(tc) => {
                    let mut c = center(&subtrends[tc.start_idx..=tc.end_idx]).unwrap();
                    if tc.extended_subtrends > 0 {
                        c.end = subtrends[tc.end_idx + tc.extended_subtrends].end.clone();
                        c.n += tc.extended_subtrends;
                    }
                    CenterElement::Center(c)
                }
                TemporaryElement::SubTrend(tst) => {
                    CenterElement::SubTrend(subtrends[tst.idx].clone())
                }
                TemporaryElement::SemiCenter(tsc) => {
                    let sc = semicenter(
                        &subtrends[tsc.start_idx..=tsc.last_end_idx()],
                        tsc.shared_start,
                    )
                    .unwrap();
                    CenterElement::SemiCenter(sc)
                }
            })
            .collect()
    }

    // 前一个元素是中枢
    // 判断当前次级别走势与中枢的关系。
    // 优先判断中枢是否需要移动，判断逻辑为：
    // 当前三段可构成中枢，前中枢仅3段且允许被移动，当前次级别走势较前中枢起始段更靠近中枢区间。
    // 1. 次级别走势起点在中枢区间内，结束在中枢区间内：合并进中枢区间。
    // 2. 次级别走势起点在中枢区间内，结束在中枢区间外：合并进中枢区间。
    // 3. 次级别走势起点在中枢区间外，结束在中枢区间内：合并进中枢区间（离开中枢后又返回中枢）。
    // 4. 次级别走势起点在中枢区间外，结束在中枢区间外，且不跨越中枢区间：作为单独的次级别走势（结束点往往是买卖点）。
    // 5. 次级别走势起点在中枢区间外，结束在中枢区间外，且跨越中枢区间：合并进中枢区间。
    // 当中枢仅3段时，需判断中枢是否迁移
    // todo: 中枢延伸至9段或以上的处理
    fn accumulate_after_center(
        &mut self,
        subtrends: &[SubTrend],
        idx: usize,
        tmp_center: TemporaryCenter,
    ) {
        let subtrend = &subtrends[idx];
        let center_data = &subtrends[tmp_center.start_idx..=tmp_center.end_idx];
        // 仅以前三段获取中枢区间
        let prev_center = center(center_data).expect("center created from subtrends");
        if tmp_center.extended_subtrends == 0
            && center(&subtrends[tmp_center.start_idx + 1..=idx]).is_some()
        {
            // 当前段和中枢起始段相比，是否更靠近中枢区间
            let subtrend0 = &subtrends[tmp_center.start_idx];
            if !tmp_center.fixed {
                let mid = (&prev_center.shared_high.value + &prev_center.shared_low.value) / 2;
                let diff0 =
                    abs_diff(&subtrend0.start.value, &mid) + abs_diff(&subtrend0.end.value, &mid);
                let diff =
                    abs_diff(&subtrend.start.value, &mid) + abs_diff(&subtrend.end.value, &mid);
                if diff < diff0 {
                    // 中枢迁移
                    self.remove_lastn(1);
                    self.push_subtrend(tmp_center.start_idx, false);
                    // 移动一次后不允许再次移动
                    let tc = TemporaryCenter {
                        start_idx: tmp_center.end_idx - 1,
                        end_idx: idx,
                        extended_subtrends: 0,
                        fixed: true,
                    };
                    self.push_center(tc);
                    return;
                }
            }
        }

        if prev_center.contains_price(&subtrend.start.value) {
            if prev_center.contains_price(&subtrend.end.value) {
                // case 1
                self.modify_last_center(|tc| {
                    tc.extended_subtrends = idx - tc.end_idx;
                });
            } else {
                // case 2
                self.modify_last_center(|tc| {
                    tc.extended_subtrends = idx - tc.end_idx;
                });
            }
        } else if prev_center.contains_price(&subtrend.end.value) {
            // case 3
            self.modify_last_center(|tc| {
                tc.extended_subtrends = idx - tc.end_idx;
            })
        } else if !prev_center.split_prices(&subtrend.start.value, &subtrend.end.value) {
            // case 4
            self.push_subtrend(idx, true);
        } else {
            // case 5
            self.modify_last_center(|tc| {
                tc.extended_subtrends = idx - tc.end_idx;
            });
        }
    }

    // 前一个元素是次级别走势
    // 需要判断前两元素，按先后顺序设为elem1, elem2
    // 1. elem1不存在，则作为次级别走势插入。
    // 2. elem1为次级别走势，判断3个次级别走势是否形成类中枢/中枢，如形成，则合并为一个，否则，插入。
    // 3. elem1为中枢，将中枢的最后一个次级别走势(x)拆解出来，判断x, elem2，curr_elem形态。
    //    a) 形成类中枢/中枢，合并插入，并设置shared_start
    //    b) 不形成中枢，直接插入
    // 4. elem1为类中枢，将类中枢的最后一个次级别走势(x)拆解出来，判断x, elem2, curr_elem形态。
    //    a) 形成类中枢，合并进原类中枢。
    //    b) 形成中枢，以次级别走势直接插入。
    //    c) 不形成中枢（不可能发生）。
    fn accumulate_after_subtrend(&mut self, subtrends: &[SubTrend], idx: usize) {
        let subtrend = &subtrends[idx];
        match self.last2() {
            None => {
                // case 1
                self.push_subtrend(idx, false);
            }
            Some((TemporaryElement::SubTrend(st1), TemporaryElement::SubTrend(st2))) => {
                // case 2
                let subtrend1 = &subtrends[st1.idx];
                let subtrend2 = &subtrends[st2.idx];
                if let Some(c) = center3(subtrend1, subtrend2, subtrend) {
                    if st1.beside_semi {
                        // 起始段紧邻类中枢，该中枢固定不可移动
                        let c = TemporaryCenter {
                            start_idx: st1.idx,
                            end_idx: idx,
                            extended_subtrends: 0,
                            fixed: true,
                        };
                        self.remove_lastn(2);
                        self.push_center(c);
                    } else if c.semi() {
                        let sc = TemporarySemiCenter {
                            start_idx: st1.idx,
                            end_idx: idx,
                            extended_subtrends: 0,
                            shared_start: false,
                        };
                        self.remove_lastn(2);
                        self.push_semicenter(sc);
                    } else {
                        let c = TemporaryCenter {
                            start_idx: st1.idx,
                            end_idx: idx,
                            extended_subtrends: 0,
                            fixed: false,
                        };
                        self.remove_lastn(2);
                        self.push_center(c);
                    }
                } else {
                    self.push_subtrend(idx, false);
                }
            }
            Some((TemporaryElement::Center(c1), TemporaryElement::SubTrend(st2))) => {
                // case 3
                let st1_idx = c1.last_end_idx();
                let subtrend1 = &subtrends[st1_idx];
                let subtrend2 = &subtrends[st2.idx];
                if let Some(c) = center3(subtrend1, subtrend2, subtrend) {
                    if c.semi() {
                        let c1_extended_subtrends = c1.extended_subtrends;
                        let sc = TemporarySemiCenter {
                            start_idx: st1_idx,
                            end_idx: idx,
                            extended_subtrends: 0,
                            shared_start: c1_extended_subtrends == 0,
                        };
                        self.remove_lastn(1);
                        // 将中枢延伸最后一段去除
                        if c1_extended_subtrends > 0 {
                            self.modify_last_center(|c| {
                                c.extended_subtrends -= 1;
                            });
                        }
                        self.push_semicenter(sc);
                    } else {
                        // 若前一中枢存在延伸，可以借取最后一段形成中枢
                        if c1.extended_subtrends > 0 {
                            let c = TemporaryCenter {
                                start_idx: st1_idx,
                                end_idx: idx,
                                extended_subtrends: 0,
                                fixed: false,
                            };
                            self.remove_lastn(1);
                            self.modify_last_center(|c| {
                                c.extended_subtrends -= 1;
                            });
                            self.push_center(c);
                        } else {
                            self.push_subtrend(idx, false);
                        }
                    }
                } else {
                    self.push_subtrend(idx, false);
                }
            }
            Some((TemporaryElement::SemiCenter(sc1), TemporaryElement::SubTrend(st2))) => {
                // case 4
                let st1_idx = sc1.last_end_idx();
                let subtrend1 = &subtrends[st1_idx];
                let subtrend2 = &subtrends[st2.idx];
                if let Some(c) = center3(subtrend1, subtrend2, subtrend) {
                    if c.semi() {
                        self.remove_lastn(1);
                        self.modify_last_semicenter(|sc| {
                            sc.extended_subtrends = idx - sc.end_idx;
                        });
                    } else {
                        self.push_subtrend(idx, false);
                    }
                } else {
                    // should not happen
                    self.push_subtrend(idx, false);
                }
            }
            _ => unreachable!(),
        }
    }

    // 前一个元素是类中枢
    // 判断当前次级别走势是否与类中枢的后两段形成中枢
    // 1. 形成中枢：
    //    a) 如果类中枢延伸不少于两段，则剥离两段合成中枢，即类中枢迁移为中枢，该中枢不可移动
    //    b) 类中枢仅3段，则拆分为1段+合成中枢，该中枢不可移动
    // 2. 未形成中枢，以次级别走势插入
    fn accumulate_after_semicenter(
        &mut self,
        subtrends: &[SubTrend],
        idx: usize,
        tmp_sc: TemporarySemiCenter,
    ) {
        let subtrend = &subtrends[idx];
        let st2_idx = tmp_sc.last_end_idx();
        let st1_idx = st2_idx - 1;
        let subtrend1 = &subtrends[st1_idx];
        let subtrend2 = &subtrends[st2_idx];
        if center3(subtrend1, subtrend2, subtrend).is_some() {
            if tmp_sc.extended_subtrends >= 2 {
                // case 1-a
                self.modify_last_semicenter(|sc| {
                    sc.extended_subtrends -= 2;
                });
                self.push_center(TemporaryCenter {
                    start_idx: st1_idx,
                    end_idx: idx,
                    extended_subtrends: 0,
                    fixed: true,
                });
            } else {
                // case 1-b
                self.remove_lastn(1);
                if !tmp_sc.shared_start {
                    self.push_subtrend(tmp_sc.start_idx, false);
                }
                self.push_center(TemporaryCenter {
                    start_idx: st1_idx,
                    end_idx: idx,
                    extended_subtrends: 0,
                    fixed: true,
                });
            }
        } else {
            self.push_subtrend(idx, true);
        }
    }

    fn last1(&self) -> Option<&TemporaryElement> {
        self.tmp.last()
    }

    fn last1_mut(&mut self) -> Option<&mut TemporaryElement> {
        self.tmp.last_mut()
    }

    fn last2(&self) -> Option<(&TemporaryElement, &TemporaryElement)> {
        let len = self.tmp.len();
        if len < 2 {
            return None;
        }
        Some((&self.tmp[len - 2], &self.tmp[len - 1]))
    }

    #[allow(dead_code)]
    fn lastn(&self, n: usize) -> Option<&[TemporaryElement]> {
        let len = self.tmp.len();
        if len < n {
            return None;
        }
        Some(&self.tmp[len - n..])
    }

    fn remove_lastn(&mut self, n: usize) {
        for _ in 0..n {
            self.tmp.pop();
        }
    }

    fn modify_last_center<F>(&mut self, f: F)
    where
        F: FnOnce(&mut TemporaryCenter),
    {
        if let Some(TemporaryElement::Center(tc)) = self.last1_mut() {
            f(tc)
        }
    }

    fn modify_last_semicenter<F>(&mut self, f: F)
    where
        F: FnOnce(&mut TemporarySemiCenter),
    {
        if let Some(TemporaryElement::SemiCenter(tsc)) = self.last1_mut() {
            f(tsc)
        }
    }

    fn push_subtrend(&mut self, idx: usize, beside_semi: bool) {
        self.tmp.push(TemporaryElement::SubTrend(TemporarySubTrend {
            idx,
            beside_semi,
        }));
    }

    fn push_semicenter(&mut self, tsc: TemporarySemiCenter) {
        self.tmp.push(TemporaryElement::SemiCenter(tsc));
    }

    fn push_center(&mut self, tc: TemporaryCenter) {
        self.tmp.push(TemporaryElement::Center(tc));
    }
}

/// 由连续三段次级别走势构成中枢
fn center(subtrends: &[SubTrend]) -> Option<Center> {
    if subtrends.len() < 3 {
        return None;
    }
    // 中枢级别与次级别走势级别相同
    center3(&subtrends[0], &subtrends[1], &subtrends[2])
}

fn center3(s1: &SubTrend, s2: &SubTrend, s3: &SubTrend) -> Option<Center> {
    let level = {
        let mut lv = s1.level;
        if s2.level > lv {
            lv = s2.level;
        }
        if s3.level > lv {
            lv = s3.level;
        }
        lv
    };
    let (s1_min, s1_max) = s1.sorted_points();
    let (s3_min, s3_max) = s3.sorted_points();
    // 三段无重合
    if s1_max.value < s3_min.value || s1_min.value > s3_max.value {
        return None;
    }
    let (low, shared_low) = if s1_min.value < s3_min.value {
        (s1_min, s3_min)
    } else {
        (s3_min, s1_min)
    };
    let (high, shared_high) = if s1_max.value > s3_max.value {
        (s1_max, s3_max)
    } else {
        (s3_max, s1_max)
    };

    Some(Center {
        start: s1.start.clone(),
        end: s3.end.clone(),
        shared_low,
        shared_high,
        low,
        high,
        level,
        upward: s1.end.value > s1.start.value,
        n: 3,
    })
}

// 调用该方法应保证输入的次级别走势序列符合类中枢定义
fn semicenter(subtrends: &[SubTrend], shared_start: bool) -> Option<SemiCenter> {
    if subtrends.len() < 3 {
        return None;
    }
    let level = subtrends.iter().map(|st| st.level).max().unwrap();
    let start = subtrends[0].start.clone();
    let end = subtrends.last().unwrap().end.clone();
    let upward = end.value > start.value;
    let n = subtrends.len();
    Some(SemiCenter {
        start,
        end,
        level,
        upward,
        n,
        shared_start,
    })
}

/// 提供中枢额外特性
trait CenterExt {
    fn semi(&self) -> bool;
}

impl CenterExt for Center {
    fn semi(&self) -> bool {
        (self.start.value < self.shared_low.value && self.end.value > self.shared_high.value)
            || (self.start.value > self.shared_high.value && self.end.value < self.shared_low.value)
    }
}

fn abs_diff(v1: &BigDecimal, v2: &BigDecimal) -> BigDecimal {
    if v1 > v2 {
        v1 - v2
    } else {
        v2 - v1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{SubTrendType, ValuePoint};
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;

    #[test]
    fn test_center3_single() {
        let sts = vec![
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 11.0),
            ("2020-02-12 15:00", 10.5),
            ("2020-02-13 15:00", 11.5),
        ]
        .build(1);
        let c = center3(&sts[0], &sts[1], &sts[2]).unwrap();
        assert_eq!(1, c.level);
        assert_eq!(new_ts("2020-02-10 15:00"), c.start.ts);
        assert_eq!(BigDecimal::from(10), c.start.value);
        assert_eq!(new_ts("2020-02-13 15:00"), c.end.ts);
        assert_eq!(BigDecimal::from(11.5), c.end.value);
        assert_eq!(BigDecimal::from(10.5), c.shared_low.value);
        assert_eq!(BigDecimal::from(11), c.shared_high.value);
        assert_eq!(BigDecimal::from(10), c.low.value);
        assert_eq!(BigDecimal::from(11.5), c.high.value);
    }

    #[test]
    fn test_center3_narrow() {
        let sts = vec![
            ("2020-02-10 15:00", 15.0),
            ("2020-02-11 15:00", 15.5),
            ("2020-02-12 15:00", 14.5),
            ("2020-02-13 15:00", 15.2),
        ]
        .build(1);
        let c = center3(&sts[0], &sts[1], &sts[2]).unwrap();
        assert_eq!(1, c.level);
        assert_eq!(new_ts("2020-02-10 15:00"), c.start.ts);
        assert_eq!(BigDecimal::from(15), c.start.value);
        assert_eq!(new_ts("2020-02-13 15:00"), c.end.ts);
        assert_eq!(BigDecimal::from(15.2), c.end.value);
        assert_eq!(BigDecimal::from(15), c.shared_low.value);
        assert_eq!(BigDecimal::from(15.2), c.shared_high.value);
        assert_eq!(BigDecimal::from(14.5), c.low.value);
        assert_eq!(BigDecimal::from(15.5), c.high.value);
    }

    #[test]
    fn test_center3_none() {
        let sts = vec![
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 10.2),
            ("2020-02-12 15:00", 9.5),
            ("2020-02-13 15:00", 9.8),
        ]
        .build(1);
        assert!(center3(&sts[0], &sts[1], &sts[2]).is_none());
    }

    #[test]
    fn test_centers_no_overlap() {
        let sts = vec![
            ("2020-02-10 15:00", 11.0),
            ("2020-02-11 15:00", 11.2),
            ("2020-02-12 15:00", 10.0),
            ("2020-02-13 15:00", 10.5),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(3, cs.len());
        assert!(cs.iter().all(|c| c.center().is_none()));
    }

    #[test]
    fn test_centers_semi() {
        let sts = vec![
            ("2020-02-07 15:00", 10.0),
            ("2020-02-10 15:00", 11.0),
            ("2020-02-11 15:00", 10.5),
            ("2020-02-12 15:00", 11.5),
            ("2020-02-13 15:00", 11.2),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(2, cs.len());
        assert!(cs[0].semicenter().is_some());
        assert!(cs[1].subtrend().is_some());
    }

    #[test]
    fn test_centers_single() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 11.0),
            ("2020-02-12 15:00", 10.5),
            ("2020-02-13 15:00", 11.5),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(2, cs.len());
        assert!(cs[0].subtrend().is_some());
        let c1 = cs[1].center().expect("expect center");
        assert_eq!(BigDecimal::from(10.5), c1.shared_low.value);
        assert_eq!(BigDecimal::from(11), c1.shared_high.value);
        assert_eq!(3, c1.n);
    }

    #[test]
    fn test_centers_double() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 11.0),
            ("2020-02-12 15:00", 10.5),
            ("2020-02-13 15:00", 11.5),
            ("2020-02-18 15:00", 8.0),
            ("2020-02-19 15:00", 8.5),
            ("2020-02-20 15:00", 8.2),
            ("2020-02-21 15:00", 9.5),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        // todo
        // assert_eq!(4, cs.len());
        assert!(cs[0].subtrend().is_some());
        let c1 = cs[1].center().expect("expect center");
        assert_eq!(new_ts("2020-02-10 15:00"), c1.start.ts);
        assert_eq!(BigDecimal::from(10.0), c1.start.value);
        assert_eq!(new_ts("2020-02-13 15:00"), c1.end.ts);
        assert_eq!(BigDecimal::from(11.5), c1.end.value);
        assert!(cs[2].subtrend().is_some());
        let c3 = cs[3].center().expect("expect center");
        assert_eq!(new_ts("2020-02-18 15:00"), c3.start.ts);
        assert_eq!(BigDecimal::from(8), c3.start.value);
        assert_eq!(new_ts("2020-02-21 15:00"), c3.end.ts);
        assert_eq!(BigDecimal::from(9.5), c3.end.value);
        assert_eq!(BigDecimal::from(8.2), c3.shared_low.value);
        assert_eq!(BigDecimal::from(8.5), c3.shared_high.value);
        assert_eq!(BigDecimal::from(8.0), c3.low.value);
        assert_eq!(BigDecimal::from(9.5), c3.high.value);
    }

    #[test]
    fn test_centers_extension_simple() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 11.0),
            ("2020-02-12 15:00", 10.5),
            ("2020-02-13 15:00", 11.5),
            ("2020-02-18 15:00", 10.8),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(2, cs.len());
        assert!(cs[0].subtrend().is_some());
        let c1 = cs[1].center().expect("expect center");
        assert_eq!(new_ts("2020-02-10 15:00"), c1.start.ts);
        assert_eq!(new_ts("2020-02-18 15:00"), c1.end.ts);
    }

    #[test]
    fn test_centers_extension_through() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 10.0),
            ("2020-02-11 15:00", 11.0),
            ("2020-02-12 15:00", 10.5),
            ("2020-02-13 15:00", 11.5),
            ("2020-02-18 15:00", 9.0),
            ("2020-02-19 15:00", 12.0),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(2, cs.len());
        assert!(cs[0].subtrend().is_some());
        let c1 = cs[1].center().expect("expect center");
        assert_eq!(new_ts("2020-02-10 15:00"), c1.start.ts);
        assert_eq!(new_ts("2020-02-19 15:00"), c1.end.ts);
        assert_eq!(5, c1.n);
    }

    #[test]
    fn test_centers_semi_simple() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 11.0),
            ("2020-02-11 15:00", 11.5),
            ("2020-02-12 15:00", 10.0),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(1, cs.len());
        let c0 = cs[0].semicenter().expect("expect semicenter");
        assert_eq!(new_ts("2020-02-07 15:00"), c0.start.ts);
        assert_eq!(new_ts("2020-02-12 15:00"), c0.end.ts);
    }

    #[test]
    fn test_centers_semi_extension() {
        let sts = vec![
            ("2020-02-07 15:00", 13.0),
            ("2020-02-10 15:00", 11.0),
            ("2020-02-11 15:00", 11.5),
            ("2020-02-12 15:00", 10.0),
            ("2020-02-13 15:00", 10.5),
            ("2020-02-18 15:00", 9.0),
        ]
        .build(1);
        let cs = unify_centers(&sts);
        assert_eq!(1, cs.len());
        let c0 = cs[0].semicenter().expect("expect semicenter");
        assert_eq!(new_ts("2020-02-07 15:00"), c0.start.ts);
        assert_eq!(new_ts("2020-02-18 15:00"), c0.end.ts);
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn new_point(ts: &str, price: f64) -> ValuePoint {
        ValuePoint {
            ts: new_ts(ts),
            value: BigDecimal::from(price),
        }
    }

    trait BuildSubTrendVec {
        fn build(self, level: i32) -> Vec<SubTrend>;
    }

    impl<'a> BuildSubTrendVec for Vec<(&'a str, f64)> {
        fn build(self, level: i32) -> Vec<SubTrend> {
            self.iter()
                .zip(self.iter().skip(1))
                .map(|(start, end)| {
                    let start = new_point(start.0, start.1);
                    let end = new_point(end.0, end.1);
                    SubTrend {
                        start,
                        end,
                        level,
                        typ: SubTrendType::Normal,
                    }
                })
                .collect()
        }
    }
}
