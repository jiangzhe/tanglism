use crate::Result;
use crate::shape::{Stroke, CStroke, Segment, Parting};
use chrono::NaiveDateTime;
use bigdecimal::BigDecimal;

/// 将笔序列解析为线段序列
pub fn sks_to_sgs(sks: &[Stroke]) -> Result<Vec<Segment>> {
    SegmentShaper::new(sks).run()
}

pub struct SegmentShaper<'s> {
    sks: &'s [Stroke],
}

impl<'s> SegmentShaper<'s> {
    fn new(sks: &'s [Stroke]) -> Self {
        SegmentShaper{
            sks,
        }
    }

    fn run(self) -> Result<Vec<Segment>> {
        if self.sks.is_empty() {
            return Ok(Vec::new());
        }

        let mut body = Vec::new();
        let input = &self.sks;
        let len = input.len();
        let mut ps = PendingSegment::new(input[0].clone());
        let mut index = 1;
        while index < len {
            let action = ps.add(input[index].clone());
            if let Some(reset_ts) = action.reset_ts {
                if let Some(sg) = action.sg {
                    body.push(sg);
                }
                // reset index to new start
                if let Some(new_start) = self.rfind_sk_index(index as i32, reset_ts) {
                    index = new_start as usize;
                }
            }
            index += 1;
        }

        let mut rest_sks = Vec::new();
        if let Some(last_sg) = body.last() {
            if let Some(pending_sg) = ps.action_none().sg {
                if last_sg.end_pt.extremum_ts < pending_sg.end_pt.extremum_ts {
                    // 将剩余部分加入最终结果
                    body.push(pending_sg);
                }
            }
        } else {
            // 当结果未空但剩余部分可完成，添加到最终结果
            if ps.completable {
                if let Some(pending_sg) = ps.action_none().sg {
                    body.push(pending_sg);
                }
            }
        }
        // 收集无法成线段的笔
        if let Some(last_sg) = body.last() {
            if let Some(rest_start) = self.rfind_sk_index((len-1) as i32, last_sg.end_pt.extremum_ts) {
                for i in rest_start as usize..len {
                    rest_sks.push(input[i].clone());
                }
            }
        } else {
            rest_sks.extend(input.iter().cloned());
        }

        Ok(body)
    }

    fn rfind_sk_index(&self, mut index: i32, start_ts: NaiveDateTime) -> Option<i32> {
        while index >= 0 {
            let curr_ts = self.sks[index as usize].start_pt.extremum_ts;
            if curr_ts == start_ts {
                return Some(index);
            } else if curr_ts < start_ts {
                return None;
            }
            index -= 1;
        }
        None
    } 
}


#[derive(Debug)]
struct SegmentAction {
    // 当前线段
    sg: Option<Segment>,
    // 重置时刻
    // 当重置时刻不为空且当前线段不为空时，可将当前线段视为完成线段
    // 重置时刻是新线段起点时刻
    reset_ts: Option<NaiveDateTime>,
}

#[derive(Debug)]
struct PendingSegment {
    // 起始点
    start_pt: Parting,
    // 终止点
    end_pt: Parting,
    // 是否存在跳空
    gap_sg: Option<Segment>,
    // 跳空序列
    gap_cs: Vec<CStroke>,
    // 是否可完成，只要有连续三笔符合定义，则可视为可完成
    completable: bool,
    // 走向
    upward: bool,
    // 当前笔的奇偶性
    odd: bool,
    // 主序列，存储组成线段的所有笔
    ms: Vec<Stroke>,
    // 特征序列，存储线段的特征序列
    // 当线段向上时，由所有向下笔构成
    // 当线段向下时，由所有向上笔构成
    // 当且仅当特征序列构成顶分型时，结束向上线段
    // 当且仅当特征序列构成底分型时，结束向下线段
    // 尤其需要注意的时，特征序列的顶底分型与K线不完全一样
    // 其中，转折点前后的特征序列不可以应用包含关系，
    // 因为转折点前后的特征序列的性质并不相同（分属于不同的笔）
    // 详细解释见71课
    cs: Vec<CStroke>,
}

impl PendingSegment {
    // 通过一笔构造线段
    fn new(sk: Stroke) -> Self {
        let upward = sk.start_pt.extremum_price < sk.end_pt.extremum_price;
        PendingSegment{
            start_pt: sk.start_pt.clone(),
            end_pt: sk.end_pt.clone(),
            gap_sg: None,
            gap_cs: Vec::new(),
            completable: false,
            upward,
            odd: true,
            ms: vec![sk],
            cs: Vec::new(),
        }
    }

    // 重置，gap_sg与gap_cs需要保留
    fn reset_start(&mut self, sk: Stroke) {
        self.start_pt = sk.start_pt.clone();
        self.end_pt = sk.end_pt.clone();
        self.completable = false;
        self.upward = sk.start_pt.extremum_price < sk.end_pt.extremum_price;
        self.odd = true;
        self.ms.clear();
        self.ms.push(sk);
        self.cs.clear();
    }

    fn reset_gap(&mut self) {
        self.gap_sg = None;
        self.gap_cs.clear();
    }

    fn add(&mut self, sk: Stroke) -> SegmentAction {
        // 首先将笔加入主序列
        self.ms.push(sk.clone());
        self.odd = !self.odd;
        let action = if self.odd {
            self.add_odd(sk)
        } else {
            self.add_even(sk)
        };
        action
    }

    // 处理与线段同向的笔
    fn add_odd(&mut self, sk: Stroke) -> SegmentAction {
        // 是否存在跳空
        if self.gap_sg.is_some() {
            // 跳空后的特征序列使用奇数笔，与线段同向
            let csk0 = Self::cs_sk(sk.clone());
            // 跳空后，走出相反分型
            if self.cs_pt(&self.gap_cs, &csk0, false) {
                let new_start_ts = self.gap_sg.as_ref().unwrap().end_pt.extremum_ts;
                let new_start = self.find_sk(new_start_ts).cloned().expect("next start stroke not found");
                return self.action_reset(new_start);
            }
            // 存在跳空且继续突破
            if self.exceeds_end(&sk.end_pt.extremum_price) {
                self.end_pt = sk.end_pt;
                self.completable = true;
                // 丢弃gap
                self.reset_gap();
                return self.action_none();
            }
            // 跳空特征序列为空，直接插入
            if self.gap_cs.is_empty() {
                self.gap_cs.push(csk0);
                return self.action_none();
            }
            let last_csk = self.gap_cs.last().unwrap();
            // 判断包含关系，合并插入
            // 跳空后的合并关系与线段走向相反
            // if let Some(csk) = Self::cs_incl(&last_csk, &csk0, !self.upward) {
            if let Some(csk) = Self::cs_incl_right(&last_csk, &csk0) {
                *self.gap_cs.last_mut().unwrap() = csk;
            } else {
                self.gap_cs.push(csk0);
            }
            return self.action_none();
        }

        // 突破最高/低点
        if self.exceeds_end(&sk.end_pt.extremum_price) {
            self.end_pt = sk.end_pt;
            self.completable = true;
        }
        // 未突破
        self.action_none()
    }

    // 处理与线段异向的笔
    fn add_even(&mut self, sk: Stroke) -> SegmentAction {
        // 与起始价格交叉
        if self.cross_over_start(&sk.end_pt.extremum_price) {
            // 无论当前是否存在线段(self.completable == true)
            // 选择当前线段（无线段则为第一笔）后的第一笔作为起始笔
            let new_start = self.find_sk(self.end_pt.extremum_ts).cloned().expect("next start stroke not found");
            return self.action_reset(new_start);
        }

        // 不与起始价格交叉
        // 特征序列不为空，合并进特征序列
        if !self.cs.is_empty() {
            // 当前笔转化为特征序列合成笔
            let csk0 = Self::cs_sk(sk);

            // 检查与特征序列最后一笔的关系
            // 包含关系需要放到分型检查之后，添加序列之前
            if let Some(_) = Self::cs_incl_right(self.cs.last().as_ref().unwrap(), &csk0) {
                // 对于右包含情况，忽略该特征序列，直接返回
                return self.action_none();
            }

            // 检查分型关系
            // 跳空时，忽略底分型检查，因为跳空有特殊的跳空特征序列进行检查
            // 上升线段出现顶分型，下降线段出现底分型
            if self.gap_sg.is_none() && self.cs_pt(&self.cs, &csk0, true) {
                let new_start = self.find_sk(self.end_pt.extremum_ts).cloned().expect("next start stroke not found");
                return self.action_reset(new_start);
            }

            // 分型关系不满足的情况下，检查最后一个和倒数第二个的包含关系，并进行条件合并
            // 该包含合并使用右合并逻辑
            // if self.cs.len() >= 2 {
            //     if let Some(last_csk) = Self::cs_incl_right(&self.cs[self.cs.len()-2], &self.cs[self.cs.len()-1]) {
            //         self.cs.pop().unwrap();
            //         *self.cs.last_mut().unwrap() = last_csk;
            //         // 如果出现条件合并，则需要再次判断分型
            //         if self.gap_sg.is_none() && self.cs_pt(&self.cs, &csk0, true) {
            //             let new_start = *self.find_sk(self.end_pt.extremum_ts).expect("next start stroke not found");
            //             return self.action_reset(new_start);
            //         }
            //     }
            // }

            // 检查跳空关系
            if self.cs_gap(&self.cs[self.cs.len()-1], &csk0) {
                self.gap_sg.replace(Segment{
                    start_pt: self.start_pt.clone(),
                    end_pt: self.end_pt.clone(),
                });
            }
            
            // 插入特征序列
            self.cs.push(csk0);

            return self.action_none();
        }

        // 特征序列为空
        self.cs.push(Self::cs_sk(sk));
        self.action_none()
    }

    // 线段起始价格
    #[inline]
    fn start_price(&self) -> &BigDecimal {
        &self.start_pt.extremum_price
    }

    // 线段终止价格
    #[inline]
    fn end_price(&self) -> &BigDecimal {
        &self.end_pt.extremum_price
    }

    // 给定价格与起始价格交叉
    #[inline]
    fn cross_over_start(&self, price: &BigDecimal) -> bool {
        if self.upward {
            price < self.start_price()
        } else {
            price > self.start_price()
        }
    }

    // 给定价格超越终止价格
    #[inline]
    fn exceeds_end(&self, price: &BigDecimal) -> bool {
        self.exceeds(self.end_price(), price)
    }

    // 相邻特征序列是否为跳空关系
    #[inline]
    fn cs_gap(&self, csk1: &CStroke, csk2: &CStroke) -> bool {
        if self.upward {
            csk1.high_pt.extremum_price < csk2.low_pt.extremum_price
        } else {
            csk1.low_pt.extremum_price > csk2.high_pt.extremum_price
        }
    }

    // 后者价格是否超越前者（上升线段大于，下降线段小于）
    #[inline]
    fn exceeds(&self, p1: &BigDecimal, p2: &BigDecimal) -> bool {
        if self.upward {
            p1 < p2
        } else {
            p2 < p1
        }
    }

    // 两笔是否包含，并返回合并后的笔
    // 合并规则根据走向确定，向上走向合并向上，向下走向合并向下
    // 该函数在某些情况下不适用，可以用cs_incl_right()替换
    #[allow(dead_code)]
    #[inline]
    fn cs_incl(csk1: &CStroke, csk2: &CStroke, upward: bool) -> Option<CStroke> {        
        // csk1包含csk2
        if csk1.high_pt.extremum_price >= csk2.high_pt.extremum_price && csk1.low_pt.extremum_price <= csk2.low_pt.extremum_price {
            let csk = if upward {
                CStroke{
                    high_pt: csk1.high_pt.clone(),
                    low_pt: csk2.low_pt.clone(),
                }
            } else {
                CStroke {
                    high_pt: csk2.high_pt.clone(),
                    low_pt: csk1.low_pt.clone(),
                }
            };
            return Some(csk);
        }

        // csk2包含csk1
        if csk1.high_pt.extremum_price <= csk2.high_pt.extremum_price && csk1.low_pt.extremum_price >= csk2.low_pt.extremum_price {
            let csk = if upward {
                CStroke{
                    high_pt: csk2.high_pt.clone(),
                    low_pt: csk1.low_pt.clone(),
                }
            } else {
                CStroke{
                    high_pt: csk1.high_pt.clone(),
                    low_pt: csk2.low_pt.clone(),
                }
            };
            return Some(csk);
        }
        None
    }

    // 右包含
    // 判断左侧笔是否包含右侧笔，如包含则返回左侧笔
    // 对线段的特征序列，我们采用不同于k线的处理方式
    // 即忽略较小波动，而不是向上或向下压缩波动
    fn cs_incl_right(csk1: &CStroke, csk2: &CStroke) -> Option<CStroke> {
        if csk1.high_pt.extremum_price >= csk2.high_pt.extremum_price && csk1.low_pt.extremum_price <= csk2.low_pt.extremum_price {
            return Some(csk1.clone())
        }
        None
    }

    // 通过单笔生成合成笔
    #[inline]
    fn cs_sk(sk: Stroke) -> CStroke {
        if sk.start_pt.extremum_price < sk.end_pt.extremum_price {
            CStroke{
                high_pt: sk.end_pt,
                low_pt: sk.start_pt,
            }
        } else {
            CStroke{
                high_pt: sk.start_pt,
                low_pt: sk.end_pt,
            }
        }
    }

    // 特征序列分型判断
    // 跳空后特征序列判断forward=false
    fn cs_pt(&self, cs: &[CStroke], sk3: &CStroke, forward: bool) -> bool {
        let len = cs.len();
        if len < 2 {
            return false;
        }
        let sk1 = &cs[len-2];
        let sk2 = &cs[len-1];
        let top_pt_check = (self.upward && forward) || (!self.upward && !forward);
        if top_pt_check {
            // 顶分型，放松第一二元素的底价要求
            sk1.high_pt.extremum_price < sk2.high_pt.extremum_price && 
                sk2.high_pt.extremum_price > sk3.high_pt.extremum_price &&
                sk2.low_pt.extremum_price > sk3.low_pt.extremum_price
        } else {
            // 底分型，放松第一二元素的顶价要求
            sk1.low_pt.extremum_price > sk2.low_pt.extremum_price && 
                sk2.low_pt.extremum_price < sk3.low_pt.extremum_price &&
                sk2.high_pt.extremum_price < sk3.high_pt.extremum_price
        }
    }

    fn action_none(&self) -> SegmentAction {
        let sg = if self.completable {
            Some(Segment{
                start_pt: self.start_pt.clone(),
                end_pt: self.end_pt.clone(),
            })
        } else {
            None
        };
        SegmentAction{
            sg,
            reset_ts: None,
        }
    }

    // 将当前线段重置为给定的笔作为起点
    // 若有可完成的候选线段，则在结果中返回，并设置complete=true
    fn action_reset(&mut self,  new_start: Stroke) -> SegmentAction {
        let sg = if self.completable {
            Some(Segment{
                start_pt: self.start_pt.clone(),
                end_pt: self.end_pt.clone(),
            })
        } else {
            None
        };
        let reset_ts = new_start.start_pt.extremum_ts;
        self.reset_start(new_start);
        self.reset_gap();
        SegmentAction {
            sg,
            reset_ts: Some(reset_ts),
        }
    }

    // 从主序列中找出指定开始时间的笔
    fn find_sk(&self, start_ts: NaiveDateTime) -> Option<&Stroke> {
        self.ms.iter().find(|msk| msk.start_pt.extremum_ts == start_ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;

    // 未确定线段
    #[test]
    fn test_segment_undetermined() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:20", 10.50),
            new_sk("2020-02-02 10:20", 10.50, "2020-02-02 10:40", 10.30),
            new_sk("2020-02-02 10:40", 10.30, "2020-02-02 11:00", 11.00),
        ];

        let sgs = sks_to_sgs(&sks)?;

        assert!(!sgs.is_empty());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[0].end_pt.extremum_ts);
        
        Ok(())
    }

    // 线段被笔破坏
    #[test]
    fn test_segment_broken_by_stroke() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:20", 10.50),
            new_sk("2020-02-02 10:20", 10.50, "2020-02-02 10:40", 10.30),
            new_sk("2020-02-02 10:40", 10.30, "2020-02-02 11:00", 11.00),
            new_sk("2020-02-02 11:00", 11.00, "2020-02-02 11:20", 9.00),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert!(!sgs.is_empty());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 未形成线段被笔破坏，起点前移
    #[test]
    fn test_segment_incomplete_broken_by_stroke() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.80),
            new_sk("2020-02-02 10:10", 10.80, "2020-02-02 10:20", 10.50),
            new_sk("2020-02-02 10:20", 10.50, "2020-02-02 10:30", 10.70),
            new_sk("2020-02-02 10:30", 10.70, "2020-02-02 10:40", 9.50),
        ];
        let sgs = sks_to_sgs(&sks)?;

        assert!(!sgs.is_empty());
        assert_eq!(new_ts("2020-02-02 10:10"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:40"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 线段被线段破坏
    #[test]
    fn test_segment_broken_by_segment() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.80),
            new_sk("2020-02-02 10:10", 10.80, "2020-02-02 10:20", 10.50),
            new_sk("2020-02-02 10:20", 10.50, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.30),
            new_sk("2020-02-02 10:40", 10.30, "2020-02-02 10:50", 10.60),
            new_sk("2020-02-02 10:50", 10.60, "2020-02-02 11:00", 9.50),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[1].end_pt.extremum_ts);
        
        Ok(())
    }

    // 跳空缺口未形成底分型
    #[test]
    fn test_segment_gap_without_parting() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.80),
            new_sk("2020-02-02 10:10", 10.80, "2020-02-02 10:20", 10.50),
            new_sk("2020-02-02 10:20", 10.50, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 11.00),
            new_sk("2020-02-02 10:40", 11.00, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 10.40),
            new_sk("2020-02-02 11:00", 10.40, "2020-02-02 11:10", 11.50),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:10"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口未形成底分型且包含
    #[test]
    fn test_segment_gap_without_parting_but_inclusive() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.70),
            new_sk("2020-02-02 10:40", 10.70, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 10.80),
            new_sk("2020-02-02 11:00", 10.80, "2020-02-02 11:10", 11.50),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:10"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口未形成底分型且未突破
    #[test]
    fn test_segment_gap_without_parting_and_exceeding() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.70),
            new_sk("2020-02-02 10:40", 10.70, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 10.80),
            new_sk("2020-02-02 11:00", 10.80, "2020-02-02 11:10", 10.90),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口形成底分型
    #[test]
    fn test_segment_gap_with_parting() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.90),
            new_sk("2020-02-02 10:40", 10.90, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 10.20),
            new_sk("2020-02-02 11:00", 10.20, "2020-02-02 11:10", 10.90),
            new_sk("2020-02-02 11:10", 10.90, "2020-02-02 11:20", 10.80),
            new_sk("2020-02-02 11:20", 10.80, "2020-02-02 11:30", 11.40),
        ];
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(3, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[2].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:30"), sgs[2].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口形成底分型且包含
    #[test]
    fn test_segment_gap_with_parting_and_inclusive() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.60),
            new_sk("2020-02-02 10:40", 10.60, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 10.70),
            new_sk("2020-02-02 11:00", 10.70, "2020-02-02 11:10", 11.00),
            new_sk("2020-02-02 11:10", 11.00, "2020-02-02 11:20", 10.40),
            new_sk("2020-02-02 11:20", 10.40, "2020-02-02 11:30", 10.80),
            new_sk("2020-02-02 11:30", 10.80, "2020-02-02 13:10", 10.60),
            new_sk("2020-02-02 13:10", 10.60, "2020-02-02 13:20", 11.15),
        ];
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(3, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:20"), sgs[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:20"), sgs[2].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 13:20"), sgs[2].end_pt.extremum_ts);
        Ok(())
    }
    
    // 跳空缺口被笔破坏
    #[test]
    fn test_segment_gap_broken_by_stroke() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.90),
            new_sk("2020-02-02 10:40", 10.90, "2020-02-02 10:50", 11.10),
            new_sk("2020-02-02 10:50", 11.10, "2020-02-02 11:00", 9.80),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口继续突破
    #[test]
    fn test_segment_gap_with_exceeding() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 11.20),
            new_sk("2020-02-02 10:30", 11.20, "2020-02-02 10:40", 10.90),
            new_sk("2020-02-02 10:40", 10.90, "2020-02-02 10:50", 11.50),
            new_sk("2020-02-02 10:50", 11.50, "2020-02-02 11:00", 11.30),
        ];
        let sgs = sks_to_sgs(&sks)?;
        
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 特征序列包含顶分型左
    #[test]
    fn test_segment_inclusive_parting_left() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 11.00),
            new_sk("2020-02-02 10:10", 11.00, "2020-02-02 10:20", 10.20),
            new_sk("2020-02-02 10:20", 10.20, "2020-02-02 10:30", 10.80),
            new_sk("2020-02-02 10:30", 10.80, "2020-02-02 10:40", 10.50),
            new_sk("2020-02-02 10:40", 10.50, "2020-02-02 10:50", 11.30),
            new_sk("2020-02-02 10:50", 11.30, "2020-02-02 11:00", 10.40),
            new_sk("2020-02-02 11:00", 10.40, "2020-02-02 11:10", 10.70),
            new_sk("2020-02-02 11:10", 10.70, "2020-02-02 11:20", 10.10),
        ];
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:20"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    // 特征序列包含顶分型右
    #[test]
    fn test_segment_inclusive_parting_right() -> Result<()> {
        let sks = vec![
            new_sk("2020-02-02 10:00", 10.00, "2020-02-02 10:10", 10.50),
            new_sk("2020-02-02 10:10", 10.50, "2020-02-02 10:20", 10.30),
            new_sk("2020-02-02 10:20", 10.30, "2020-02-02 10:30", 10.80),
            new_sk("2020-02-02 10:30", 10.80, "2020-02-02 10:40", 10.40),
            new_sk("2020-02-02 10:40", 10.40, "2020-02-02 10:50", 11.30),
            new_sk("2020-02-02 10:50", 11.30, "2020-02-02 11:00", 10.30),
            new_sk("2020-02-02 11:00", 10.30, "2020-02-02 11:10", 11.00),
            new_sk("2020-02-02 11:10", 11.00, "2020-02-02 11:20", 10.70),
            new_sk("2020-02-02 11:20", 10.70, "2020-02-02 11:30", 11.00),
            new_sk("2020-02-02 11:30", 11.00, "2020-02-02 13:10", 10.10),
        ];
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 13:10"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    fn new_sk(start_ts: &str, start_price: f64, end_ts: &str, end_price: f64) -> Stroke {
        let upward = start_price < end_price;
        let start_pt = new_pt_fix_width(start_ts, 1, start_price, 3, !upward);
        let end_pt = new_pt_fix_width(end_ts, 1, end_price, 3, upward);
        Stroke{
            start_pt,
            end_pt,
        }
    }

    fn new_pt_fix_width(ts: &str, minutes: i64, extremum_price: f64, n: i32, top: bool) -> Parting {
        let extremum_ts = new_ts(ts);
        let start_ts = extremum_ts - chrono::Duration::minutes(minutes);
        let end_ts = extremum_ts + chrono::Duration::minutes(minutes);
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(extremum_price),
            n,
            top,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
