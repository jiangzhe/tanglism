use serde_derive::*;
use chrono::NaiveDateTime;
use bigdecimal::BigDecimal;
use crate::shape::{CenterElement, Center, SemiCenter, SubTrend};

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
}

impl TemporaryCenter {
    fn last_end_idx(&self) -> usize {
        self.end_idx + self.extended_subtrends
    }
}

#[derive(Debug, Clone)]
struct TemporarySubTrend {
    idx: usize,
    // 是否紧邻一个中枢
    beside_center: bool,
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
        Standard{
            tmp: Vec::new(),
        }
    }

    #[inline]
    fn accumulate(&mut self, subtrends: &[SubTrend], idx: usize) {
        if let Some(elem) = self.last1() {
            match elem {
                TemporaryElement::Center(tc) => {
                    self.accumulate_after_center(subtrends, idx, tc.clone());         
                }
                TemporaryElement::SubTrend(_) => {
                    self.accumulate_after_subtrend(subtrends, idx);
                }
                TemporaryElement::SemiCenter(tsc) => {
                    self.accumulate_after_semicenter(subtrends, idx, tsc.clone());
                }
            }
            return;
        }

        // 第一个元素
        self.push_subtrend(idx, false);
    }

    fn centers(self, subtrends: &[SubTrend]) -> Vec<CenterElement> {
        self.tmp.into_iter().map(|te| {
            match te {
                TemporaryElement::Center(tc) => {
                    let mut c = center(&subtrends[tc.start_idx..=tc.end_idx]).unwrap();
                    if tc.extended_subtrends > 0 {
                        c.end = subtrends[tc.end_idx+tc.extended_subtrends].end.clone();
                        c.n += tc.extended_subtrends;
                    }
                    CenterElement::Center(c)
                }
                TemporaryElement::SubTrend(tst) => CenterElement::SubTrend(subtrends[tst.idx].clone()),
                TemporaryElement::SemiCenter(tsc) => {
                    let sc = semicenter(&subtrends[tsc.start_idx..=tsc.last_end_idx()], tsc.shared_start).unwrap();
                    CenterElement::SemiCenter(sc)
                },
            }
        }).collect()
    }

    // 前一个元素是中枢
    // 判断当前次级别走势与中枢的关系。
    // 1. 次级别走势起点在中枢区间内，结束在中枢区间内：合并进中枢区间。
    // 2. 次级别走势起点在中枢区间内，结束在中枢区间外：合并进中枢区间。
    // 3. 次级别走势起点在中枢区间外，结束在中枢区间内：合并进中枢区间（离开中枢后又返回中枢）。
    // 4. 次级别走势起点在中枢区间外，结束在中枢区间外，且不跨越中枢区间：作为单独的次级别走势（结束点往往是买卖点）。
    // 5. 次级别走势起点在中枢区间外，结束在中枢区间外，且跨越中枢区间：合并进中枢区间。
    // todo: 中枢延伸至9段或以上的处理
    fn accumulate_after_center(&mut self, subtrends: &[SubTrend], idx: usize, tmp_center: TemporaryCenter) {
        let subtrend = &subtrends[idx];
        let center_data = &subtrends[tmp_center.start_idx..=tmp_center.end_idx];
        // 获取中枢区间
        let center = center(center_data).expect("center created from subtrends");

        if center.contains_price(&subtrend.start.value) {
            if center.contains_price(&subtrend.end.value) {
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
        } else {
            if center.contains_price(&subtrend.end.value) {
                // case 3
                self.modify_last_center(|tc| {
                    tc.extended_subtrends = idx - tc.end_idx;
                })
            } else if !center.split_prices(&subtrend.start.value, &subtrend.end.value) {
                // case 4
                self.push_subtrend(idx, true);
            } else {
                // case 5
                self.modify_last_center(|tc| {
                    tc.extended_subtrends = idx - tc.end_idx;
                });
            }
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
                    if c.semi() {
                        let sc = TemporarySemiCenter{start_idx: st1.idx, end_idx: idx, extended_subtrends: 0, shared_start: false};
                        self.remove_lastn(2);
                        self.push_semicenter(sc);
                    } else {
                        let c = TemporaryCenter{start_idx: st1.idx, end_idx: idx, extended_subtrends: 0};
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
                        let sc = TemporarySemiCenter{start_idx: st1_idx, end_idx: idx, extended_subtrends: 0, shared_start: c1.extended_subtrends == 0};
                        self.remove_lastn(1);
                        self.push_semicenter(sc);
                    } else {
                        // 中枢无法和其他中枢共享次级别走势
                        self.push_subtrend(idx, false);
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
    //    a) 如果类中枢延伸不少于两段，则剥离两段合成中枢
    //    b) 类中枢仅3段，则拆分为1段+合成中枢
    // 2. 未形成中枢，以次级别走势插入
    fn accumulate_after_semicenter(&mut self, subtrends: &[SubTrend], idx: usize, tmp_sc: TemporarySemiCenter) {
        let subtrend = &subtrends[idx];
        let st2_idx = tmp_sc.last_end_idx();
        let st1_idx = st2_idx - 1;
        let subtrend1 = &subtrends[st1_idx];
        let subtrend2 = &subtrends[st2_idx];
        if let Some(c) = center3(subtrend1, subtrend2, subtrend) {
            if !c.semi() {
                if tmp_sc.extended_subtrends >= 2 {
                    // case 1-a
                    self.modify_last_semicenter(|sc| {
                        sc.extended_subtrends -= 2;
                    });
                    self.push_center(TemporaryCenter{start_idx: st1_idx, end_idx: idx, extended_subtrends: 0});
                } else {
                    // case 1-b
                    self.remove_lastn(1);
                    if !tmp_sc.shared_start {
                        // 这里beside_center设置可能存在问题
                        self.push_subtrend(tmp_sc.start_idx, false);
                    }
                    self.push_center(TemporaryCenter{start_idx: st1_idx, end_idx: idx, extended_subtrends: 0});
                }
            } else {
                unreachable!();
            }
        } else {
            self.push_subtrend(idx, false);
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
        Some((&self.tmp[len-2], &self.tmp[len-1]))
    }

    fn lastn(&self, n: usize) -> Option<&[TemporaryElement]> {
        let len = self.tmp.len();
        if len < n {
            return None;
        }
        Some(&self.tmp[len-n..])
    }

    fn remove_lastn(&mut self, n: usize) {
        for _ in 0..n {
            self.tmp.pop();
        }
    }

    fn modify_last_center<F>(&mut self, f: F) where F: FnOnce(&mut TemporaryCenter) {
        if let Some(TemporaryElement::Center(tc)) = self.last1_mut() {
            f(tc)
        }
    }

    fn modify_last_semicenter<F>(&mut self, f: F) where F: FnOnce(&mut TemporarySemiCenter) {
        if let Some(TemporaryElement::SemiCenter(tsc)) = self.last1_mut() {
            f(tsc)
        }
    }

    fn push_subtrend(&mut self, idx: usize, beside_center: bool) {
        self.tmp.push(TemporaryElement::SubTrend(TemporarySubTrend{idx, beside_center}));
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
    Some(SemiCenter{
        start,
        end,
        level,
        upward,
        n,
        shared_start,
    })
}




// #[cfg(test)]
// mod tests {
//     use super::*;
//     use bigdecimal::BigDecimal;
//     use chrono::NaiveDateTime;

//     #[test]
//     fn test_center2_single() {
//         let s1 = SubTrend {
//             start_ts: new_ts("2020-02-10 15:00"),
//             start_price: BigDecimal::from(10),
//             end_ts: new_ts("2020-02-11 15:00"),
//             end_price: BigDecimal::from(11),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         let s3 = SubTrend {
//             start_ts: new_ts("2020-02-12 15:00"),
//             start_price: BigDecimal::from(10.5),
//             end_ts: new_ts("2020-02-13 15:00"),
//             end_price: BigDecimal::from(11.5),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         let c = center2(&s1, &s3).unwrap();
//         assert_eq!(new_ts("2020-02-10 15:00"), c.start_ts);
//         assert_eq!(BigDecimal::from(10), c.start_price);
//         assert_eq!(new_ts("2020-02-13 15:00"), c.end_ts);
//         assert_eq!(BigDecimal::from(11.5), c.end_price);
//         assert_eq!(BigDecimal::from(10.5), c.shared_low);
//         assert_eq!(BigDecimal::from(11), c.shared_high);
//         assert_eq!(BigDecimal::from(10), c.low.price);
//         assert_eq!(BigDecimal::from(11.5), c.high.price);
//         assert_eq!(2, c.level);
//     }

//     #[test]
//     fn test_center2_narrow() {
//         let s1 = SubTrend {
//             start_ts: new_ts("2020-02-10 15:00"),
//             start_price: BigDecimal::from(15),
//             end_ts: new_ts("2020-02-11 15:00"),
//             end_price: BigDecimal::from(15.5),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         let s3 = SubTrend {
//             start_ts: new_ts("2020-02-12 15:00"),
//             start_price: BigDecimal::from(14.5),
//             end_ts: new_ts("2020-02-13 15:00"),
//             end_price: BigDecimal::from(15.2),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         let c = center2(&s1, &s3).unwrap();
//         assert_eq!(new_ts("2020-02-10 15:00"), c.start_ts);
//         assert_eq!(BigDecimal::from(15), c.start_price);
//         assert_eq!(new_ts("2020-02-13 15:00"), c.end_ts);
//         assert_eq!(BigDecimal::from(15.2), c.end_price);
//         assert_eq!(BigDecimal::from(15), c.shared_low);
//         assert_eq!(BigDecimal::from(15.2), c.shared_high);
//         assert_eq!(BigDecimal::from(14.5), c.low.price);
//         assert_eq!(BigDecimal::from(15.5), c.high.price);
//         assert_eq!(2, c.level);
//     }

//     #[test]
//     fn test_center2_none() {
//         let s1 = SubTrend {
//             start_ts: new_ts("2020-02-10 15:00"),
//             start_price: BigDecimal::from(10),
//             end_ts: new_ts("2020-02-11 15:00"),
//             end_price: BigDecimal::from(10.2),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         let s3 = SubTrend {
//             start_ts: new_ts("2020-02-12 15:00"),
//             start_price: BigDecimal::from(9.5),
//             end_ts: new_ts("2020-02-13 15:00"),
//             end_price: BigDecimal::from(9.8),
//             level: 1,
//             typ: SubTrendType::Normal,
//         };
//         assert!(center2(&s1, &s3).is_none());
//     }

//     #[test]
//     fn test_centers_none() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(0, cs.len());
//     }

//     #[test]
//     fn test_centers_none_with_no_overlap() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-07 15:00"),
//                 start_price: BigDecimal::from(10.2),
//                 end_ts: new_ts("2020-02-10 15:00"),
//                 end_price: BigDecimal::from(10),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(0, cs.len());
//     }

//     #[test]
//     fn test_centers_single() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-07 15:00"),
//                 start_price: BigDecimal::from(13),
//                 end_ts: new_ts("2020-02-10 15:00"),
//                 end_price: BigDecimal::from(10),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(1, cs.len());
//         assert_eq!(BigDecimal::from(10.5), cs[0].shared_low);
//         assert_eq!(BigDecimal::from(11), cs[0].shared_high);
//     }

//     #[test]
//     fn test_centers_double() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-07 15:00"),
//                 start_price: BigDecimal::from(13),
//                 end_ts: new_ts("2020-02-10 15:00"),
//                 end_price: BigDecimal::from(10),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-13 15:00"),
//                 start_price: BigDecimal::from(11.5),
//                 end_ts: new_ts("2020-02-18 15:00"),
//                 end_price: BigDecimal::from(8),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-18 15:00"),
//                 start_price: BigDecimal::from(8),
//                 end_ts: new_ts("2020-02-19 15:00"),
//                 end_price: BigDecimal::from(8.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-19 15:00"),
//                 start_price: BigDecimal::from(8.5),
//                 end_ts: new_ts("2020-02-20 15:00"),
//                 end_price: BigDecimal::from(8.2),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-20 15:00"),
//                 start_price: BigDecimal::from(8.2),
//                 end_ts: new_ts("2020-02-21 15:00"),
//                 end_price: BigDecimal::from(9.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(2, cs.len());
//         assert_eq!(new_ts("2020-02-18 15:00"), cs[1].start_ts);
//         assert_eq!(BigDecimal::from(8), cs[1].start_price);
//         assert_eq!(new_ts("2020-02-21 15:00"), cs[1].end_ts);
//         assert_eq!(BigDecimal::from(9.5), cs[1].end_price);
//         assert_eq!(BigDecimal::from(8.2), cs[1].shared_low);
//         assert_eq!(BigDecimal::from(8.5), cs[1].shared_high);
//         assert_eq!(BigDecimal::from(8.0), cs[1].low.price);
//         assert_eq!(BigDecimal::from(9.5), cs[1].high.price);
//     }

//     #[test]
//     fn test_centers_extension() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-07 15:00"),
//                 start_price: BigDecimal::from(13),
//                 end_ts: new_ts("2020-02-10 15:00"),
//                 end_price: BigDecimal::from(10),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-13 15:00"),
//                 start_price: BigDecimal::from(11.5),
//                 end_ts: new_ts("2020-02-18 15:00"),
//                 end_price: BigDecimal::from(10.8),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(1, cs.len());
//         assert_eq!(new_ts("2020-02-10 15:00"), cs[0].start_ts);
//         assert_eq!(new_ts("2020-02-18 15:00"), cs[0].end_ts);
//     }

//     #[test]
//     fn test_centers_extension_through() {
//         let sts = vec![
//             SubTrend {
//                 start_ts: new_ts("2020-02-07 15:00"),
//                 start_price: BigDecimal::from(13),
//                 end_ts: new_ts("2020-02-10 15:00"),
//                 end_price: BigDecimal::from(10),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-10 15:00"),
//                 start_price: BigDecimal::from(10),
//                 end_ts: new_ts("2020-02-11 15:00"),
//                 end_price: BigDecimal::from(11),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-11 15:00"),
//                 start_price: BigDecimal::from(11),
//                 end_ts: new_ts("2020-02-12 15:00"),
//                 end_price: BigDecimal::from(10.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-12 15:00"),
//                 start_price: BigDecimal::from(10.5),
//                 end_ts: new_ts("2020-02-13 15:00"),
//                 end_price: BigDecimal::from(11.5),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-13 15:00"),
//                 start_price: BigDecimal::from(11.5),
//                 end_ts: new_ts("2020-02-18 15:00"),
//                 end_price: BigDecimal::from(9),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//             SubTrend {
//                 start_ts: new_ts("2020-02-18 15:00"),
//                 start_price: BigDecimal::from(9),
//                 end_ts: new_ts("2020-02-19 15:00"),
//                 end_price: BigDecimal::from(12),
//                 level: 1,
//                 typ: SubTrendType::Normal,
//             },
//         ];
//         let cs = merge_centers(&sts, 1);
//         assert_eq!(1, cs.len());
//         assert_eq!(new_ts("2020-02-10 15:00"), cs[0].start_ts);
//         assert_eq!(new_ts("2020-02-18 15:00"), cs[0].end_ts);
//     }

//     fn new_ts(s: &str) -> NaiveDateTime {
//         NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
//     }
// }
