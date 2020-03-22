use crate::{Error, Parting, Result, Segment, Stroke, CStroke, CK, K};
use serde_derive::*;
use tanglism_utils::TradingTimestamps;
use chrono::NaiveDateTime;

/// 单标的确定周期的数据来源
pub trait Source {
    // 给定代码和时刻，获取该时刻前的不多于limit条数的K线数据
    fn data_before(&self, ts: &str, limit: u32) -> Vec<K>;

    // 给定代码和时刻，获取该时刻后的不多余limit条数的K线数据
    fn data_after(&self, ts: &str, limit: u32) -> Vec<K>;
}

/// 分型序列
///
/// 包含潜在分型的序列，以及未能形成分型的尾部K线
/// 可通过输入最新K线，更新或延长已有序列
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PartingSeq {
    pub body: Vec<Parting>,
    pub tail: Vec<CK>,
}

/// 笔序列
///
/// 包含笔序列，以及未形成笔的尾部分型
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StrokeSeq {
    pub body: Vec<Stroke>,
    // 笔尾
    // 包含未能成笔的顶底分型及合成K线
    pub tail: Option<PartingSeq>,
}

/// 线段序列
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentSeq {
    pub body: Vec<Segment>,
    pub tail: Option<StrokeSeq>,
}

/// 将K线图解析为分型序列
pub fn ks_to_pts(ks: &[K]) -> Result<PartingSeq> {
    PartingShaper::new(ks).run()
}

struct PartingShaper<'k> {
    ks: &'k [K],
    body: Vec<Parting>,
    first_k: Option<CK>,
    second_k: Option<CK>,
    third_k: Option<CK>,
    upward: bool,
}

impl<'k> PartingShaper<'k> {
    fn new(ks: &'k [K]) -> Self {
        PartingShaper {
            ks,
            body: Vec::new(),
            first_k: None,
            second_k: None,
            third_k: None,
            upward: true,
        }
    }

    fn consume(&mut self, k: K) {
        // k1不存在
        if self.first_k.is_none() {
            self.first_k = Some(Self::k_to_ck(k));
            return;
        }
        // k1存在
        let k1 = self.first_k.unwrap();

        // k2不存在
        if self.second_k.is_none() {
            // 检查k1与k的包含关系
            match Self::inclusive_neighbor_k(k1, k, self.upward) {
                None => {
                    // 更新k2
                    self.second_k = Some(Self::k_to_ck(k));
                    self.upward = k.high > k1.high;
                    return;
                }
                ck => {
                    // 合并k1与k
                    self.first_k = ck;
                    return;
                }
            }
        }

        // k2存在
        let k2 = self.second_k.unwrap();

        // k3不存在
        if self.third_k.is_none() {
            // 检查k2与k的包含关系
            let ck = Self::inclusive_neighbor_k(k2, k, self.upward);
            if ck.is_some() {
                // 更新k2
                self.second_k = ck;
                return;
            }
            // 检查k1, k2与k是否形成顶/底分型
            if (self.upward && k.low < k2.low) || (!self.upward && k.high > k2.high) {
                // 形成顶/底分型，更新k2和k3，并将走势颠倒
                self.third_k = Some(Self::k_to_ck(k));
                self.upward = !self.upward;
                return;
            }

            // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变
            self.first_k = self.second_k.take();
            self.second_k = Some(Self::k_to_ck(k));
            return;
        }

        let k3 = self.third_k.unwrap();

        // 检查k3与k的包含关系
        let ck = Self::inclusive_neighbor_k(k3, k, self.upward);
        if ck.is_some() {
            // 更新k3
            self.third_k = ck;
            return;
        }

        //不包含，需构建分型并记录
        let parting = Parting {
            start_ts: k1.start_ts,
            end_ts: k3.end_ts,
            extremum_ts: k2.extremum_ts,
            extremum_price: if self.upward { k2.low } else { k2.high },
            n: k1.n + k2.n + k3.n,
            top: !self.upward,
        };
        self.body.push(parting);

        // 当k2, k3, k形成顶底分型时，左移1位
        if (self.upward && k.low < k3.low) || (!self.upward && k.high > k3.high) {
            self.first_k = self.second_k.take();
            self.second_k = self.third_k.take();
            self.third_k = Some(Self::k_to_ck(k));
            self.upward = !self.upward;
            return;
        }

        // 不形成分型时，将k3, k向左移两位
        self.upward = k.high > k3.high;
        self.first_k = Some(k3);
        self.second_k = Some(Self::k_to_ck(k));
        self.third_k = None;
    }

    fn run(mut self) -> Result<PartingSeq> {
        for k in self.ks.iter() {
            self.consume(*k);
        }

        // 结束所有k线分析后，依然存在第三根K线，说明此时三根K线刚好构成顶底分型
        if self.third_k.is_some() {
            let k1 = self.first_k.take().unwrap();
            let k2 = self.second_k.take().unwrap();
            let k3 = self.third_k.take().unwrap();

            let parting = Parting {
                start_ts: k1.start_ts,
                end_ts: k3.end_ts,
                extremum_ts: k2.extremum_ts,
                extremum_price: if self.upward { k2.low } else { k2.high },
                n: k1.n + k2.n + k3.n,
                top: !self.upward,
            };
            self.body.push(parting);
            // 向左平移k2和k3
            self.first_k = Some(k2);
            self.second_k = Some(k3);
        }

        let mut tail = vec![];
        // 将剩余k线加入尾部，必定不会出现三根K线
        if let Some(fk) = self.first_k {
            tail.push(fk);
        }
        if let Some(sk) = self.second_k {
            tail.push(sk);
        }
        Ok(PartingSeq {
            body: self.body,
            tail,
        })
    }

    /// 辅助函数，将单个K线转化为合并K线
    #[inline]
    fn k_to_ck(k: K) -> CK {
        CK {
            start_ts: k.ts,
            end_ts: k.ts,
            extremum_ts: k.ts,
            high: k.high,
            low: k.low,
            n: 1,
        }
    }

    /// 辅助函数，判断相邻K线是否符合包含关系，并在符合情况下返回包含后的合并K线
    fn inclusive_neighbor_k(k1: CK, k2: K, upward: bool) -> Option<CK> {
        let extremum_ts = if k1.high >= k2.high && k1.low <= k2.low {
            k1.extremum_ts
        } else if k2.high >= k1.high && k2.low <= k1.low {
            k2.ts
        } else {
            return None;
        };

        let start_ts = k1.start_ts;
        let end_ts = k2.ts;
        let n = k1.n + 1;

        let (high, low) = if upward {
            (
                if k1.high > k2.high { k1.high } else { k2.high },
                if k1.low > k2.low { k1.low } else { k2.low },
            )
        } else {
            (
                if k1.high < k2.high { k1.high } else { k2.high },
                if k1.low < k2.low { k1.low } else { k2.low },
            )
        };
        Some(CK {
            start_ts,
            end_ts,
            extremum_ts,
            high,
            low,
            n,
        })
    }
}

/// 将分型序列解析为笔序列
///
/// 步骤：
/// 1. 选择起始点。
/// 2. 选择下一个点。
///    若异型：邻接或交叉则忽略，不邻接则成笔
///    若同型：顶更高/底更低则修改当前笔，反之则忽略
pub fn pts_to_sks<T>(pts: &PartingSeq, tts: &T) -> Result<StrokeSeq>
where
    T: TradingTimestamps,
{
    StrokeShaper::new(pts, tts).run()
}

struct StrokeShaper<'p, 't, T> {
    pts: &'p PartingSeq,
    tts: &'t T,
    sks: Vec<Stroke>,
    tail: Vec<Parting>,
    start: Option<Parting>,
}

impl<'p, 't, T: TradingTimestamps> StrokeShaper<'p, 't, T> {
    fn new(pts: &'p PartingSeq, tts: &'t T) -> Self {
        StrokeShaper {
            pts,
            tts,
            sks: Vec::new(),
            tail: Vec::new(),
            start: None,
        }
    }

    fn run(mut self) -> Result<StrokeSeq> {
        if self.pts.body.is_empty() {
            return Ok(StrokeSeq {
                body: Vec::new(),
                tail: Some(self.pts.clone()),
            });
        }
        let mut pts_iter = self.pts.body.iter();
        let first = *pts_iter.next().unwrap();
        self.start = Some(first);
        self.tail.push(first);
        while let Some(pt) = pts_iter.next() {
            self.consume(*pt);
        }
        Ok(StrokeSeq {
            body: self.sks,
            tail: Some(PartingSeq {
                body: self.tail,
                tail: self.pts.tail.clone(),
            }),
        })
    }

    fn consume(&mut self, pt: Parting) {
        self.tail.push(pt);
        if pt.top != self.start().top {
            self.consume_diff_dir(pt);
        } else {
            self.consume_same_dir(pt);
        }
    }

    fn consume_diff_dir(&mut self, pt: Parting) {
        if self.is_start_neighbor(pt) {
            // 这里不做变化
            // 可以保留的可能性是起点跳至pt点
            return;
        }
        // 顶比底低
        if (pt.top && pt.extremum_price <= self.start().extremum_price)
            || (self.start().top && self.start().extremum_price <= pt.extremum_price)
        {
            return;
        }
        // 成笔
        let new_sk = Stroke {
            start_pt: self.start.take().unwrap(),
            end_pt: pt,
        };
        self.start = Some(pt);
        self.tail.clear();
        self.sks.push(new_sk);
    }

    fn consume_same_dir(&mut self, pt: Parting) {
        if self.is_start_neighbor(pt) {
            return;
        }
        // 顶比起点低，底比起点高
        if (pt.top && pt.extremum_price < self.start().extremum_price)
            || (!pt.top && pt.extremum_price > self.start().extremum_price)
        {
            return;
        }

        if let Some(last_sk) = self.sks.last_mut() {
            // 有笔，需要修改笔终点
            last_sk.end_pt = pt;
        }
        self.start.replace(pt);
        self.tail.clear();
    }

    fn is_start_neighbor(&self, pt: Parting) -> bool {
        if let Some(start) = self.start.as_ref() {
            if let Some(indep_ts) = self.tts.next_tick(start.end_ts) {
                if indep_ts < pt.start_ts {
                    return false;
                }
            }
        }
        true
    }

    #[inline]
    fn start(&self) -> Parting {
        self.start.unwrap()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::K;
    use chrono::NaiveDateTime;
    use tanglism_utils::LOCAL_TRADING_TS_1_MIN;

    #[test]
    fn test_shaper_no_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.25, 10.15),
            new_k("2020-02-01 10:04", 10.30, 10.20),
        ];
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        let r = ks_to_pts(&ks)?;
        assert_eq!(0, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:03"), r.tail[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04"), r.tail[1].start_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.10, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r.body[0].extremum_ts);
        assert_eq!(10.20, r.body[0].extremum_price);
        assert_eq!(true, r.body[0].top);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting_inclusive() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        // panic!(json);
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:04"), r.body[0].end_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.10),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(true, r.body[0].top);
        assert_eq!(new_ts("2020-02-01 10:02"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04"), r.body[1].end_ts);
        assert_eq!(false, r.body[1].top);
        assert_eq!(2, r.tail.len());
        Ok(())
    }

    #[test]
    fn test_shaper_two_indep_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.10, 10.00),
            new_k("2020-02-01 10:05", 10.05, 9.95),
            new_k("2020-02-01 10:06", 10.00, 9.90),
            new_k("2020-02-01 10:07", 10.05, 9.95),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:05"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:07"), r.body[1].end_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_no_stroke() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:01", 10.10, true),
            new_pt1("2020-02-01 10:03", 9.50, false),
            new_pt1("2020-02-01 10:06", 9.80, true),
        ]);
        assert!(sks.body.is_empty());
        assert_eq!(4, sks.tail.unwrap().body.len());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.40, true),
            new_pt1("2020-02-01 10:13", 10.30, false),
        ]);
        assert_eq!(1, sks.body.len());
        assert_eq!(1, sks.tail.unwrap().body.len());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:02", 10.10, true),
            new_pt1("2020-02-01 10:04", 9.90, false),
            new_pt1("2020-02-01 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.body.len());
        assert_eq!(new_ts("2020-02-01 10:04"), sks.body[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks.body[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_non_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:02", 10.10, true),
            new_pt1("2020-02-01 10:04", 10.02, false),
            new_pt1("2020-02-01 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.body.len());
        assert_eq!(new_ts("2020-02-01 10:00"), sks.body[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks.body[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_strokes_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.10, true),
            new_pt1("2020-02-01 10:20", 10.02, false),
        ]);
        assert_eq!(2, sks.body.len());
        Ok(())
    }

    fn pts_to_sks_1_min(pts: Vec<Parting>) -> StrokeSeq {
        pts_to_sks(&new_pts(pts), &*LOCAL_TRADING_TS_1_MIN).unwrap()
    }

    fn new_pts(pts: Vec<Parting>) -> PartingSeq {
        PartingSeq {
            body: pts,
            tail: vec![],
        }
    }

    fn new_pt1(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 1, price, 3, top)
    }

    fn new_pt5(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 5, price, 3, top)
    }

    fn new_pt30(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 30, price, 3, top)
    }

    fn new_pt_fix_width(ts: &str, minutes: i64, extremum_price: f64, n: i32, top: bool) -> Parting {
        let extremum_ts = new_ts(ts);
        let start_ts = extremum_ts - chrono::Duration::minutes(minutes);
        let end_ts = extremum_ts + chrono::Duration::minutes(minutes);
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price,
            n,
            top,
        }
    }

    fn new_k(ts: &str, high: f64, low: f64) -> K {
        K {
            ts: new_ts(ts),
            high,
            low,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
