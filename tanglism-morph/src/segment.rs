use crate::Result;
use crate::shape::{Stroke, CStroke, StrokeSeq, Segment, SegmentSeq, Parting};
use chrono::NaiveDateTime;

/// 将笔序列解析为线段序列
pub fn sks_to_sgs(sks: &StrokeSeq) -> Result<SegmentSeq> {
    SegmentShaper::new(sks).run()
}

pub struct SegmentShaper<'s> {
    sks: &'s StrokeSeq,
}

impl<'s> SegmentShaper<'s> {
    fn new(sks: &'s StrokeSeq) -> Self {
        SegmentShaper{
            sks,
        }
    }

    fn run(mut self) -> Result<SegmentSeq> {
        if self.sks.body.is_empty() {
            return Ok(SegmentSeq{
                body: Vec::new(),
                tail: Some(self.sks.clone()),
            });
        }

        let mut body = Vec::new();
        let input = &self.sks.body;
        let len = input.len();
        let mut ps = PendingSegment::new(input[0]);
        let mut index = 1;
        while index < len {
            let action = ps.add(input[index]);
            if action.complete {
                body.push(action.sg.expect("segment not found"));
                // reset index to new start
                if let Some(new_start) = self.rfind_sk_index(index as i32, ps.end_pt.start_ts) {
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
                    if let Some(rest_start) = self.rfind_sk_index((len-1) as i32, pending_sg.end_pt.extremum_ts) {
                        for i in rest_start as usize..len {
                            rest_sks.push(input[i]);
                        }
                    }
                }
            }
        }
        
        let tail = if rest_sks.is_empty() && self.sks.tail.is_none() {
            None
        } else {
            Some(StrokeSeq{
                body: rest_sks,
                tail: self.sks.tail.clone(),
            })
        };

        Ok(SegmentSeq{
            body,
            tail,
        })
    }

    fn rfind_sk_index(&self, mut index: i32, start_ts: NaiveDateTime) -> Option<i32> {
        while index >= 0 {
            if self.sks.body[index as usize].start_pt.extremum_ts == start_ts {
                return Some(index);
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
    // 是否完成
    complete: bool,
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
        PendingSegment{
            start_pt: sk.start_pt,
            end_pt: sk.end_pt,
            gap_sg: None,
            gap_cs: Vec::new(),
            completable: false,
            upward: sk.start_pt.extremum_price < sk.end_pt.extremum_price,
            odd: true,
            ms: Vec::new(),
            cs: Vec::new(),
        }
    }

    // 重置，gap_sg与gap_cs需要保留
    fn reset_start(&mut self, sk: Stroke) {
        self.start_pt = sk.start_pt;
        self.end_pt = sk.end_pt;
        self.completable = false;
        self.upward = sk.start_pt.extremum_price < sk.end_pt.extremum_price;
        self.odd = true;
        self.ms.clear();
        self.cs.clear();
    }

    fn reset_gap(&mut self) {
        self.gap_sg = None;
        self.gap_cs.clear();
    }

    fn add(&mut self, sk: Stroke) -> SegmentAction {
        // 首先将笔加入主序列
        self.ms.push(sk);
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
            let csk0 = Self::cs_sk(&sk);
            // 跳空后，走出相反分型
            if self.cs_pt(&self.gap_cs, &csk0, false) {
                let new_start_ts = self.gap_sg.as_ref().unwrap().end_pt.extremum_ts;
                let new_start = *self.find_sk(new_start_ts).expect("next start stroke not found");
                return self.action_reset(new_start);
            }
            // 存在跳空且继续突破
            if self.exceeds_end(sk.end_pt.extremum_price) {
                self.end_pt = sk.end_pt;
                self.completable = true;
                return self.action_reset(sk);
            }
            // 跳空特征序列为空，直接插入
            if self.gap_cs.is_empty() {
                self.gap_cs.push(csk0);
                return self.action_none();
            }
            let last_csk = self.gap_cs.last().unwrap();
            // 判断包含关系，合并插入
            // 跳空后的合并关系与线段走向相反
            if let Some(csk) = Self::cs_incl(&last_csk, &csk0, !self.upward) {
                *self.gap_cs.last_mut().unwrap() = csk;
            } else {
                self.gap_cs.push(csk0);
            }
            return self.action_none();
        }

        // 突破最高/低点
        if self.exceeds_end(sk.end_pt.extremum_price) {
            self.end_pt = sk.end_pt;
            self.completable = true;
            // self.reset_gap();
            return self.action_reset(sk);
        }
        // 未突破
        self.action_none()
    }

    // 处理与线段异向的笔
    fn add_even(&mut self, sk: Stroke) -> SegmentAction {
        // 与起始价格交叉
        if self.cross_over_start(sk.end_pt.extremum_price) {
            // 无论当前是否存在线段(self.completable == true)
            // 选择当前线段（无线段则为第一笔）后的第一笔作为起始笔
            let new_start = *self.find_sk(self.end_pt.extremum_ts).expect("next start stroke not found");
            return self.action_reset(new_start);
        }

        // 不与起始价格交叉
        // 特征序列不为空，合并进特征序列
        if !self.cs.is_empty() {
            // 当前笔转化为特征序列合成笔
            let csk0 = Self::cs_sk(&sk);

            // 检查与特征序列最后一笔的关系
            // 包含关系需要放到分型检查之后，添加序列之前

            // 检查分型关系
            // 上升线段出现顶分型，下降线段出现底分型
            if self.cs_pt(&self.cs, &csk0, true) {
                let new_start = *self.find_sk(self.end_pt.extremum_ts).expect("next start stroke not found");
                return self.action_reset(new_start);
            }

            // 分型关系不满足的情况下，检查最后一个和倒数第二个的包含关系，并进行条件合并
            if self.cs.len() >= 2 {
                if let Some(last_csk) = Self::cs_incl(&self.cs[self.cs.len()-2], &self.cs[self.cs.len()-1], self.upward) {
                    self.cs.pop().unwrap();
                    *self.cs.last_mut().unwrap() = last_csk;
                }
            }

            // 检查跳空关系
            if self.cs_gap(&self.cs[self.cs.len()-1], &csk0) {
                self.gap_sg.replace(Segment{
                    start_pt: self.start_pt,
                    end_pt: self.end_pt,
                });
            }
            
            // 插入特征序列
            self.cs.push(csk0);

            return self.action_none();
        }

        // 特征序列为空
        self.cs.push(Self::cs_sk(&sk));
        self.action_none()
    }

    // 线段起始价格
    #[inline]
    fn start_price(&self) -> f64 {
        self.start_pt.extremum_price
    }

    // 线段终止价格
    #[inline]
    fn end_price(&self) -> f64 {
        self.end_pt.extremum_price
    }

    // 给定价格与起始价格交叉
    #[inline]
    fn cross_over_start(&self, price: f64) -> bool {
        if self.upward {
            price < self.start_price()
        } else {
            price > self.start_price()
        }
    }

    // 给定价格超越终止价格
    #[inline]
    fn exceeds_end(&self, price: f64) -> bool {
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
    fn exceeds(&self, p1: f64, p2: f64) -> bool {
        if self.upward {
            p1 < p2
        } else {
            p2 < p1
        }
    }

    // 两笔是否包含，并返回合并后的笔，
    // 合并规则根据走向确定，向上走向合并向上，向下走向合并向下
    #[inline]
    fn cs_incl(csk1: &CStroke, csk2: &CStroke, upward: bool) -> Option<CStroke> {        
        // csk1包含csk2
        if csk1.high_pt.extremum_price >= csk2.high_pt.extremum_price && csk1.low_pt.extremum_price <= csk2.low_pt.extremum_price {
            let csk = if upward {
                CStroke{
                    high_pt: csk1.high_pt,
                    low_pt: csk2.low_pt,
                }
            } else {
                CStroke {
                    high_pt: csk2.high_pt,
                    low_pt: csk1.low_pt,
                }
            };
            return Some(csk);
        }

        // csk2包含csk1
        if csk1.high_pt.extremum_price <= csk2.high_pt.extremum_price && csk1.low_pt.extremum_price >= csk2.low_pt.extremum_price {
            let csk = if upward {
                CStroke{
                    high_pt: csk2.high_pt,
                    low_pt: csk1.low_pt,
                }
            } else {
                CStroke{
                    high_pt: csk1.high_pt,
                    low_pt: csk2.low_pt,
                }
            };
            return Some(csk);
        }
        None
    }

    // 通过单笔生成合成笔
    #[inline]
    fn cs_sk(sk: &Stroke) -> CStroke {
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

    // 特征序列走出正向分型
    // 向上线段走出顶分型，向下线段走出底分型
    // fn cs_seq_pt(&self, sk3: &CStroke) -> bool {
    //     let len = self.cs.len();
    //     if len < 2 {
    //         return false;
    //     }
    //     let sk1 = &self.cs[len-2];
    //     let sk2 = &self.cs[len-1];
    //     if self.upward {
    //         // 顶分型
    //         sk1.high_pt.extremum_price < sk2.high_pt.extremum_price && sk2.high_pt.extremum_price > sk3.high_pt.extremum_price
    //     } else {
    //         // 底分型
    //         sk1.low_pt.extremum_price > sk2.low_pt.extremum_price && sk2.low_pt.extremum_price < sk3.low_pt.extremum_price
    //     }
    // }

    // 跳空序列走出反向分型
    // 向上线段跳空后走出底分型，或向下线段跳空后走出顶分型
    // fn gap_cs_rev_pt(&self, sk3: &CStroke) -> bool {
    //     let len = self.gap_cs.len();
    //     if len < 2 {
    //         return false;
    //     }
    //     let sk1 = &self.gap_cs[len-2];
    //     let sk2 = &self.gap_cs[len-1];
    //     if self.upward {
    //         // 底分型
    //         sk1.low_pt.extremum_price > sk2.low_pt.extremum_price && sk2.low_pt.extremum_price < sk3.low_pt.extremum_price
    //     } else {
    //         // 顶分型
    //         sk1.high_pt.extremum_price < sk2.high_pt.extremum_price && sk2.high_pt.extremum_price > sk3.high_pt.extremum_price
    //     }
    // }

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
            // 顶分型
            sk1.high_pt.extremum_price < sk2.high_pt.extremum_price && sk2.high_pt.extremum_price > sk3.high_pt.extremum_price
        } else {
            // 底分型
            sk1.low_pt.extremum_price > sk2.low_pt.extremum_price && sk2.low_pt.extremum_price < sk3.low_pt.extremum_price
        }
    }

    fn action_none(&self) -> SegmentAction {
        let sg = if self.completable {
            Some(Segment{
                start_pt: self.start_pt,
                end_pt: self.end_pt,
            })
        } else {
            None
        };
        SegmentAction{
            sg,
            complete: false,
        }
    }

    fn action_reset(&mut self,  new_start: Stroke) -> SegmentAction {
        let action = self.action_none();
        self.reset_start(new_start);
        self.reset_gap();
        action
    }

    // 从主序列中找出指定开始时间的笔
    fn find_sk(&self, start_ts: NaiveDateTime) -> Option<&Stroke> {
        self.ms.iter().find(|msk| msk.start_pt.extremum_ts == start_ts)
    }
}
