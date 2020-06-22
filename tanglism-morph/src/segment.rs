use crate::shape::{Parting, Segment, Stroke};
use crate::stream::{Accumulator, Aggregator, Delta};
use crate::stroke::{stroke_to_cstroke, CStroke, StrokeDelta};
use crate::{Error, Result};
use bigdecimal::BigDecimal;
use serde_derive::*;

/// 将笔序列解析为线段序列
pub fn sks_to_sgs(sks: &[Stroke]) -> Result<Vec<Segment>> {
    // SegmentShaper::new(sks).run()
    SegmentAccumulator::new().aggregate(sks)
}

pub type SegmentDelta = Delta<Segment>;

#[derive(Debug, Clone)]
pub struct CSegment {
    sg: Segment,
    orig: Option<Box<CSegment>>,
}

/// 在累加过程中，存在某些步骤修改了临时变量无法回溯
/// 保存快照以应对。快照仅保存一份。
#[derive(Debug, Clone)]
pub struct SegmentAccState {
    // 累加器阶段
    stage: AccStage,
    // 最高最低点所在笔下标（笔终点）
    extremum_idx: usize,
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
    // 有条件地处理左包含关系
    // a)
    cs: Vec<CStroke>,
    // 用于在缺口回调后判断是否有一个相反分型，使前一段成立
    // 数组中依次存放回调后的顺势笔
    gap_cs: Vec<CStroke>,
    // 用于在第一次回调后判断一个不高于最高点分型是否可成段
    // 数组中依次存放回调后的顺势笔
    first_inv_cs: Vec<Stroke>,
}

impl SegmentAccState {
    fn new() -> Self {
        SegmentAccState {
            stage: AccStage::Empty,
            extremum_idx: 0,
            ms: Vec::new(),
            cs: Vec::new(),
            gap_cs: Vec::new(),
            first_inv_cs: Vec::new(),
        }
    }

    // 线段走向与第一笔走向一致
    fn upward(&self) -> Result<bool> {
        if self.ms.is_empty() {
            return Err(Error("empty stroke list".to_owned()));
        }
        let first = &self.ms[0];
        Ok(first.end_price() > first.start_price())
    }

    fn extremum_price(&self) -> Result<BigDecimal> {
        if let Some(sk) = self.ms.get(self.extremum_idx) {
            return Ok(sk.end_price().clone());
        }
        Err(Error(format!(
            "extremum index {} not mapped to stroke",
            self.extremum_idx
        )))
    }

    fn start_price(&self) -> Result<BigDecimal> {
        if let Some(sk) = self.ms.first() {
            return Ok(sk.start_price().clone());
        }
        Err(Error("no stroke in state".to_owned()))
    }

    fn reset_empty(&mut self) {
        self.stage = AccStage::Empty;
        self.extremum_idx = 0;
        self.ms.clear();
        self.cs.clear();
        self.gap_cs.clear();
        self.first_inv_cs.clear();
    }

    // 创新高或新低，构建新线段
    // 可以在FirstInverse, Inverse和GapInverse复用该方法
    fn switch_inverse_to_continue(&mut self, item: &Stroke) -> MustUse<Segment> {
        self.add_main_stroke(item);
        self.stage = AccStage::Continue;
        self.extremum_idx = self.ms.len() - 1;
        self.gap_cs.clear();
        self.first_inv_cs.clear();
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: item.end_pt.clone(),
        })
    }

    // Inverse => next Continue
    fn switch_inverse_to_next_continue(&mut self, item: &Stroke) -> MustUse<Segment> {
        let new_strokes: Vec<_> = self.ms.drain(self.extremum_idx + 1..).collect();
        self.reset_empty();
        for (idx, sk) in new_strokes.iter().enumerate() {
            self.add_main_stroke(&sk);
            if (idx & 1) == 0 {
                self.add_cs_stroke(&sk, true);
            }
        }
        // 添加当前笔
        self.add_main_stroke(item);
        self.stage = AccStage::Continue;
        self.extremum_idx = self.ms.len() - 1;
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: item.end_pt.clone(),
        })
    }

    // GapInverse => next Continue 复用Inverse => next Continue
    fn switch_gap_inverse_to_next_continue(&mut self, item: &Stroke) -> MustUse<Segment> {
        self.switch_inverse_to_next_continue(item)
    }

    // 起始 => 第一笔
    fn switch_empty_to_first_stroke(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.stage = AccStage::FirstStroke;
    }

    // 第一笔 => 第一次回调
    fn switch_first_stroke_to_first_inverse(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_cs_stroke(item, false);
        self.stage = AccStage::FirstInverse;
    }

    // 第一次回调 => 缺口回调
    // 需要移动极值点
    fn switch_first_inverse_to_gap_inverse(&mut self, item: &Stroke) -> MustUse<Segment> {
        // todo
        self.add_main_stroke(item);
        // 一定不包含
        self.add_cs_stroke(item, false);
        self.stage = AccStage::GapInverse;
        self.extremum_idx = self.ms.len() - 2;
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: item.start_pt.clone(),
        })
    }

    fn switch_first_inverse_to_curr_continue(&mut self, item: &Stroke) -> MustUse<Segment> {
        self.add_main_stroke(item);
        self.stage = AccStage::Continue;
        self.extremum_idx = self.ms.len() - 1;
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: item.end_pt.clone(),
        })
    }

    fn switch_first_inverse_to_next_continue(&mut self, item: &Stroke) -> MustUse<Segment> {
        // 将起点向后移动一位
        let new_strokes: Vec<_> = self.ms.drain(1..).collect();
        self.reset_empty();
        for (idx, sk) in new_strokes.iter().enumerate() {
            self.add_main_stroke(&sk);
            if (idx & 1) == 0 {
                self.add_cs_stroke(&sk, true);
            }
        }
        // 添加当前笔
        self.add_main_stroke(item);
        self.stage = AccStage::Continue;
        self.extremum_idx = self.ms.len() - 1;
        self.first_inv_cs.clear();
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: self.ms.last().unwrap().end_pt.clone(),
        })
    }

    fn switch_first_inverse_to_next_first_stroke(&mut self, item: &Stroke) {
        self.reset_empty();
        self.add_main_stroke(item);
        self.stage = AccStage::FirstStroke;
        self.first_inv_cs.clear();
    }

    // 第一次回调中的逆势笔
    fn keep_first_inverse_inv(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        // 当特征序列只有一笔（即第一次回调笔）时，不做包含处理
        // 当大于一笔时，需要进行左包含处理
        self.add_cs_stroke(item, self.cs.len() > 1);
    }

    // 第一次回调中的顺势笔
    fn keep_first_inverse_cont(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_first_inv_cs_stroke(item);
    }

    // 顺势 => 缺口回调
    fn switch_continue_to_gap_inverse(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_cs_stroke(item, false);
        self.stage = AccStage::GapInverse;
    }

    // 顺势 => 普通回调
    fn switch_continue_to_inverse(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_cs_stroke(item, false);
        self.stage = AccStage::Inverse(self.ms.len() - 1);
    }

    // 缺口回调 => 下一段的普通回调
    fn switch_gap_inverse_to_next_inverse(&mut self, item: &Stroke) -> MustUse<Segment> {
        let new_strokes: Vec<_> = self.ms.drain(self.extremum_idx + 1..).collect();
        self.reset_empty();
        for (idx, sk) in new_strokes.iter().enumerate() {
            self.add_main_stroke(&sk);
            if (idx & 1) == 0 {
                self.add_cs_stroke(&sk, true);
            }
        }
        // 添加当前笔
        self.add_main_stroke(item);
        self.add_cs_stroke(item, true);
        self.stage = AccStage::Inverse(self.ms.len() - 1);
        self.extremum_idx = self.ms.len() - 2;
        MustUse(Segment {
            start_pt: self.ms[0].start_pt.clone(),
            end_pt: item.start_pt.clone(),
        })
    }

    fn keep_inverse_cont(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
    }

    fn keep_inverse_inv(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_cs_stroke(item, true);
    }

    fn keep_gap_inverse_cont(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_gap_cs_stroke(item);
    }

    fn keep_gap_inverse_inv(&mut self, item: &Stroke) {
        self.add_main_stroke(item);
        self.add_cs_stroke(item, true);
    }

    fn add_main_stroke(&mut self, item: &Stroke) {
        self.ms.push(item.clone());
    }

    // 添加特征序列笔
    // 特征序列应只处理左包含
    fn add_cs_stroke(&mut self, item: &Stroke, inclusive_left: bool) {
        if !inclusive_left {
            self.cs.push(stroke_to_cstroke(item));
            return;
        }
        // 做包含处理
        if let Some(last_sk) = self.cs.last() {
            if nondirectional_inclusive_left(&last_sk.sk, item).is_some() {
                // 左包含，忽略当前笔
                return;
            }
        }
        // 无包含关系
        self.cs.push(stroke_to_cstroke(item));
    }

    fn add_first_inv_cs_stroke(&mut self, item: &Stroke) {
        self.first_inv_cs.push(item.clone());
    }

    fn add_gap_cs_stroke(&mut self, item: &Stroke) {
        if let Some(mut last_gap_csk) = self.gap_cs.pop() {
            if let Some(inc_sk) = nondirectional_inclusive(&last_gap_csk.sk, item) {
                // 与前一特征序列存在包含关系
                last_gap_csk.orig.take();
                self.gap_cs.push(CStroke {
                    sk: inc_sk,
                    orig: Some(Box::new(last_gap_csk)),
                });
                return;
            }
        }
        self.gap_cs.push(stroke_to_cstroke(item));
    }
}

/// 合并笔
///
/// 在特征序列相邻笔出现包含关系时，合并为一笔
/// 此时笔并不具有方向性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChStroke {
    pub high_pt: Parting,
    pub low_pt: Parting,
}

/// 辅助类型
///
/// 用于在状态切换时生成新的线段
/// 确保被忽略时进行警告
#[must_use]
#[derive(Debug, Clone)]
struct MustUse<T>(T);

pub struct SegmentAccumulator {
    // 当前线段状态
    state: Vec<CSegment>,
    // 当前线段变更状态
    state_change: Vec<SegmentDelta>,
    // 快照，用于Stroke更新或删除时进行回溯
    // 快照最多保存一份
    prev: Option<Box<SegmentAccState>>,
    // 当前状态
    curr: SegmentAccState,
}

/// 线段累加器有以下状态
/// 1. 起始状态
/// 2. 顺势状态
///    在顺势状态中，结束笔与该未完成线段走向一致
///    且结束笔必定结束在最高/最低点
/// 3. 逆势状态
///    在逆势状态中，结束笔与该未完成线段走向相反
///    假设线段向上（向下情况与之对称），则记录线段最高点Pmax。
///    1) 若其后的任一向上笔超过Pmax，状态切换为顺势，线段延续。
///    2) 若其后（不包含）的向上笔序列构成底分型（不检测包含关系）
///    则起点到最高点的线段结束。
///    3) 若其后的若干连续笔构成了向下的线段，则起点到最高点的线段
///    结束。
/// 4. 缺口逆势状态
///    仍假设线段向上，记最高点Pmax。
///    与逆势状态类似，最高点后向下一笔与之前的向下一笔形成
///    缺口。
///    1) 若其后的任一向上笔超过Pmax，状态切换为顺势，线段延续。
///    2) 若其后（不包含）的向上笔序列构成底分型（检测包含关系）
///    则起点到最高点的线段结束。
///    3) 若其后的若干连续笔构成了向下的线段，则起点到最高点的线段
///    结束。
impl SegmentAccumulator {
    pub fn new() -> Self {
        SegmentAccumulator {
            state: Vec::new(),
            state_change: Vec::new(),
            prev: None,
            curr: SegmentAccState::new(),
        }
    }

    fn make_snapshot(&mut self) {
        self.prev.replace(Box::new(self.curr.clone()));
    }

    fn add_segment(&mut self, sg: Segment) {
        if let Some(last_sg) = self.state.last() {
            if last_sg.sg.start_pt.extremum_ts == sg.start_pt.extremum_ts {
                let mut orig_sg = self.state.pop().unwrap();
                // 去除之前的快照
                orig_sg.orig.take();
                self.state.push(CSegment {
                    sg: sg.clone(),
                    orig: Some(Box::new(orig_sg)),
                });
                self.state_change.push(SegmentDelta::Update(sg));
                return;
            }
        }
        self.state.push(CSegment {
            sg: sg.clone(),
            orig: None,
        });
        self.state_change.push(SegmentDelta::Add(sg));
    }

    // // 在前一线段成立后，需要重播转折点后的所有笔
    // // 重播最多仅增加一段
    // fn reset_and_replay_strokes(&mut self, strokes: Vec<Stroke>) -> Result<()> {
    //     println!("replay on stroke list: {} {} - {} {}",
    //         strokes[0].start_pt.extremum_ts, strokes[0].start_pt.extremum_price,
    //         strokes[strokes.len()-1].end_pt.extremum_ts, strokes[strokes.len()-1].end_pt.extremum_price);
    //     self.curr.reset_empty();
    //     // 重播时候state和state_change并没有被重置
    //     let state_len = self.state.len();
    //     let state_change_len = self.state_change.len();
    //     // 将之后的笔依次加入
    //     for sk in strokes {
    //         self.acc_add(&sk)?;
    //     }
    //     // 断言：最多只增加一段
    //     if self.state.len() - state_len > 1 {
    //         println!("state.len()={}, state_len={}", self.state.len(), state_len);
    //         if self.state.len() > 2 {
    //             println!("last two segments:");
    //             println!("{:#?}", self.state[self.state.len()-2]);
    //             println!("{:#?}", self.state[self.state.len()-1]);
    //         }
    //     }
    //     debug_assert!(self.state.len() - state_len <= 1);
    //     // 断言：变更最多只有一个add
    //     debug_assert!(
    //         self.state_change
    //             .iter()
    //             .skip(state_change_len)
    //             .filter_map(|d| d.add())
    //             .count()
    //             <= 1,
    //         "more than one segment added after reset"
    //     );
    //     while let Some(d) = self.state_change.pop() {
    //         match d {
    //             add @ SegmentDelta::Add(_) => {
    //                 debug_assert!(self.state_change.len() == state_change_len);
    //                 self.state_change.push(add);
    //                 break;
    //             }
    //             SegmentDelta::Update(update) => {
    //                 drop(self.state_change.drain(state_change_len..));
    //                 // 将update转化为add
    //                 self.state_change.push(SegmentDelta::Add(update));
    //                 break;
    //             }
    //             SegmentDelta::Delete(delete) => {
    //                 panic!("unexpected segment deletion: {:?}", delete);
    //             }
    //             SegmentDelta::None => (),
    //         }
    //     }
    //     Ok(())
    // }

    fn acc(&mut self, item: &StrokeDelta) -> Result<SegmentDelta> {
        match item {
            StrokeDelta::None => (),
            StrokeDelta::Add(sk) => self.acc_add(sk)?,
            StrokeDelta::Update(sk) => self.acc_update(sk)?,
            StrokeDelta::Delete(sk) => self.acc_delete(sk)?,
        }
        self.pop_delta()
    }

    fn acc_add(&mut self, item: &Stroke) -> Result<()> {
        match &self.curr.stage {
            AccStage::Empty => {
                // 起始
                self.make_snapshot();
                self.curr.switch_empty_to_first_stroke(item);
                Ok(())
            }
            AccStage::FirstStroke => {
                let upward = self.curr.upward()?;
                let start_price = self.curr.ms[0].start_price();
                if cmp_prices(start_price, item.end_price(), !upward) {
                    // 第二笔破了第一笔的起点
                    self.make_snapshot();
                    // 清空第一笔
                    self.curr.reset_empty();
                    // 重播第二笔
                    return self.acc_add(item);
                }
                self.make_snapshot();
                self.curr.switch_first_stroke_to_first_inverse(item);
                Ok(())
            }
            AccStage::FirstInverse => {
                let upward = self.curr.upward()?;
                let extremum_price = self.curr.extremum_price()?;
                if cmp_prices(&extremum_price, item.end_price(), upward) {
                    // 顺势的新高/新低
                    self.make_snapshot();
                    let new_sg = self.curr.switch_inverse_to_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }
                if cmp_prices(item.start_price(), item.end_price(), upward) {
                    // 顺势不创新高/新低
                    if let Some(last_inv_csk) = &self.curr.first_inv_cs.last() {
                        if cmp_prices(last_inv_csk.start_price(), item.start_price(), upward)
                            && cmp_prices(last_inv_csk.end_price(), item.end_price(), upward)
                        {
                            // 形成顺势两笔递进
                            self.make_snapshot();
                            let new_sg = self.curr.switch_first_inverse_to_curr_continue(item);
                            self.add_segment(new_sg.0);
                            return Ok(());
                        }
                        // 不形成递进
                    }

                    // 顺势的第一笔
                    self.curr.keep_first_inverse_cont(item);
                    return Ok(());
                }

                let start_price = self.curr.start_price()?;
                if cmp_prices(&start_price, item.end_price(), !upward) {
                    // 逆势越过起点
                    self.make_snapshot();
                    if self.curr.ms.len() == 1 {
                        self.curr.switch_first_inverse_to_next_first_stroke(item);
                    } else {
                        let new_sg = self.curr.switch_first_inverse_to_next_continue(item);
                        self.add_segment(new_sg.0);
                    }
                    return Ok(());
                }
                // 在逆势状态中，且始终在第一笔的区间内震荡
                self.curr.keep_first_inverse_inv(item);
                Ok(())
            }
            AccStage::Continue => {
                // 顺势
                // 当前段走向
                let upward = self.curr.upward()?;
                if cmp_prices(
                    self.curr.ms.last().unwrap().end_price(),
                    item.end_price(),
                    upward,
                ) {
                    // 在continue状态，只接受逆势笔
                    return Err(Error("not an inverse stroke".to_owned()));
                }
                // 检查是否形成了特征序列的缺口
                if let Some(last_csk) = self.curr.cs.last() {
                    // 检查缺口
                    if cmp_prices(last_csk.sk.start_price(), &item.end_price(), upward) {
                        // 缺口存在时，进入缺口回调状态
                        self.make_snapshot();
                        self.curr.switch_continue_to_gap_inverse(item);
                        return Ok(());
                    }
                }
                // 无缺口，进入普通回调状态
                self.make_snapshot();
                self.curr.switch_continue_to_inverse(item);
                Ok(())
            }
            AccStage::Inverse(idx) => {
                // 普通回调
                let upward = self.curr.upward()?;
                let extremum_price = self.curr.extremum_price()?;
                if cmp_prices(&extremum_price, item.end_price(), upward) {
                    // 顺势笔超越极值
                    self.make_snapshot();
                    let new_sg = self.curr.switch_inverse_to_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }
                if cmp_prices(item.start_price(), item.end_price(), upward) {
                    // 顺势笔没有超过极值
                    self.curr.keep_inverse_cont(item);
                    return Ok(());
                }
                // 逆势笔
                // 设走势向上，检查当前逆势笔与普通回调第一笔是否形成了顶分型
                let sk1 = &self.curr.ms[*idx];
                if cmp_prices(sk1.start_price(), item.start_price(), !upward)
                    && cmp_prices(sk1.end_price(), item.end_price(), !upward)
                {
                    // 分型必成立
                    self.make_snapshot();
                    let new_sg = self.curr.switch_inverse_to_next_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }
                // 检查当前逆势笔i，与前逆势笔i-2，以及普通回调第一笔(j)的前一笔j-2是否形成了顶分型
                let pre_sk1 = &self.curr.ms[*idx - 2];
                let pre_item = &self.curr.ms[self.curr.ms.len() - 2];
                if cmp_prices(pre_sk1.start_price(), pre_item.start_price(), upward)
                    && cmp_prices(pre_item.start_price(), item.start_price(), !upward)
                    && cmp_prices(pre_item.end_price(), item.end_price(), !upward)
                {
                    // 分型必成立
                    self.make_snapshot();
                    let new_sg = self.curr.switch_inverse_to_next_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }

                // 分型不成立
                self.curr.keep_inverse_inv(item);
                Ok(())
            }
            AccStage::GapInverse => {
                // 缺口回调
                let upward = self.curr.upward()?;
                let extremum_price = self.curr.extremum_price()?;
                if cmp_prices(&extremum_price, item.end_price(), upward) {
                    // 顺势笔超越极值
                    self.make_snapshot();
                    let new_sg = self.curr.switch_inverse_to_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }
                if cmp_prices(item.start_price(), item.end_price(), upward) {
                    // 顺势笔没有超过极值
                    if let Some(last_gap_csk) = self.curr.gap_cs.last() {
                        if cmp_prices(last_gap_csk.sk.start_price(), item.start_price(), !upward)
                            && cmp_prices(last_gap_csk.sk.end_price(), item.end_price(), !upward)
                        {
                            // 虽然仅两笔，但已必定形成逆分型
                            self.make_snapshot();
                            let new_sg = self.curr.switch_gap_inverse_to_next_inverse(item);
                            self.add_segment(new_sg.0);
                            return Ok(());
                        }
                        // 没有形成逆分型
                    }
                    // 笔数不足
                    self.curr.keep_gap_inverse_cont(item);
                    return Ok(());
                }

                // 逆势笔
                let start_price = self.curr.start_price()?;
                if cmp_prices(&start_price, item.end_price(), !upward) {
                    // 逆势笔越过起点
                    self.make_snapshot();
                    let new_sg = self.curr.switch_gap_inverse_to_next_continue(item);
                    self.add_segment(new_sg.0);
                    return Ok(());
                }
                self.curr.keep_gap_inverse_inv(item);
                Ok(())
            }
        }
    }

    fn acc_update(&mut self, _item: &Stroke) -> Result<()> {
        unimplemented!()
    }

    fn acc_delete(&mut self, _item: &Stroke) -> Result<()> {
        unimplemented!()
    }

    fn pop_delta(&mut self) -> Result<SegmentDelta> {
        if let Some(delta) = self.state_change.pop() {
            return Ok(delta);
        }
        Ok(SegmentDelta::None)
    }
}

/// 方向性的包含关系检查
///
/// 上包含：最高点取高，最低点取高
/// 下包含：最高点取低，最低点取低
#[allow(dead_code)]
fn directional_inclusive(left: &Stroke, right: &Stroke) -> Option<CStroke> {
    if let Some(csk) = directional_inclusive_left(left, right) {
        return Some(csk);
    } else if let Some(csk) = directional_inclusive_right(left, right) {
        return Some(csk);
    }
    None
}

/// 方向性的包含关系检查，右笔包含左笔
/// 两笔同向
fn directional_inclusive_right(left: &Stroke, right: &Stroke) -> Option<CStroke> {
    // 特征序列笔方向与走向是相反的
    let upward = left.start_price() < left.end_price();
    if cmp_prices(left.start_price(), right.start_price(), !upward)
        && cmp_prices(left.end_price(), right.end_price(), upward)
    {
        // 合并后的笔的时间是反的
        let new_sk = Stroke {
            start_pt: right.start_pt.clone(),
            end_pt: left.end_pt.clone(),
        };
        return Some(CStroke {
            sk: new_sk,
            orig: Some(Box::new(CStroke {
                sk: left.clone(),
                orig: None,
            })),
        });
    }
    None
}

/// 方向性的包含关系检查，左笔包含右笔
/// 两笔同向
#[inline]
fn directional_inclusive_left(left: &Stroke, right: &Stroke) -> Option<CStroke> {
    let upward = left.start_price() < left.end_price();
    if cmp_prices(left.start_price(), right.start_price(), upward)
        && cmp_prices(left.end_price(), right.end_price(), !upward)
    {
        let new_sk = Stroke {
            start_pt: left.start_pt.clone(),
            end_pt: right.end_pt.clone(),
        };
        return Some(CStroke {
            sk: new_sk,
            orig: Some(Box::new(CStroke {
                sk: left.clone(),
                orig: None,
            })),
        });
    }
    None
}

/// 无方向的包含关系检查
fn nondirectional_inclusive(left: &Stroke, right: &Stroke) -> Option<Stroke> {
    if let Some(csk) = nondirectional_inclusive_left(left, right) {
        return Some(csk);
    } else if let Some(csk) = nondirectional_inclusive_right(left, right) {
        return Some(csk);
    }
    None
}

/// 右笔包含左笔，返回右笔
fn nondirectional_inclusive_right(left: &Stroke, right: &Stroke) -> Option<Stroke> {
    let upward = left.start_price() < left.end_price();
    if cmp_prices(left.start_price(), right.start_price(), !upward)
        && cmp_prices(left.end_price(), right.end_price(), upward)
    {
        return Some(right.clone());
    }
    None
}

// 右笔包含左笔，返回右笔
#[inline]
fn nondirectional_inclusive_left(left: &Stroke, right: &Stroke) -> Option<Stroke> {
    let upward = left.start_price() < left.end_price();
    if cmp_prices(left.start_price(), right.start_price(), upward)
        && cmp_prices(left.end_price(), right.end_price(), !upward)
    {
        return Some(left.clone());
    }
    None
}

// 比较两个价格是否与输入方向相同
#[inline]
fn cmp_prices(p1: &BigDecimal, p2: &BigDecimal, upward: bool) -> bool {
    if upward {
        return p1 < p2;
    }
    p1 > p2
}

fn csegment_to_segment(csg: &CSegment) -> Segment {
    csg.sg.clone()
}
/// 状态机转换
///
/// Empty -> FirstStroke
/// FirstStroke -> FirstInverse, FirstStroke(逆笔低于起始笔，移动起点)
/// FirstInverse -> Empty, Continue, GapInverse, Inverse
/// Continue -> Inverse, GapInverse
/// Inverse -> Continue, Empty
/// GapInverse -> Continue, Empty
#[derive(Debug, Clone)]
enum AccStage {
    // 起始状态
    Empty,
    // 第一笔
    FirstStroke,
    // 第一逆笔
    FirstInverse,
    //延续走势并创新高/新低
    Continue,
    // 逆势状态
    // 保存逆势笔索引
    Inverse(usize),
    // 缺口逆势状态
    GapInverse,
}

impl Accumulator<Stroke> for SegmentAccumulator {
    type Delta = SegmentDelta;
    type State = Vec<CSegment>;

    fn accumulate(&mut self, item: &Stroke) -> Result<SegmentDelta> {
        self.acc_add(item)?;
        self.pop_delta()
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl Aggregator<&[Stroke], Vec<Segment>> for SegmentAccumulator {
    fn aggregate(mut self, input: &[Stroke]) -> Result<Vec<Segment>> {
        for item in input {
            self.acc_add(item)?;
        }
        Ok(self.state.iter().map(csegment_to_segment).collect())
    }
}

impl Accumulator<StrokeDelta> for SegmentAccumulator {
    type Delta = SegmentDelta;
    type State = Vec<CSegment>;

    fn accumulate(&mut self, item: &StrokeDelta) -> Result<SegmentDelta> {
        self.acc(item)
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;

    // 未确定线段
    #[test]
    fn test_segment_undetermined() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:40", 10.30),
            ("2020-02-02 11:00", 11.00),
        ]
        .build();

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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:40", 10.30),
            ("2020-02-02 11:00", 11.00),
            ("2020-02-02 11:20", 9.00),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;

        assert!(!sgs.is_empty());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:00"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 未形成线段被笔破坏，起点后移
    #[test]
    fn test_segment_incomplete_broken_by_stroke() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.80),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:30", 10.70),
            ("2020-02-02 10:40", 9.50),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.80),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.30),
            ("2020-02-02 10:50", 10.60),
            ("2020-02-02 11:00", 9.50),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.80),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 11.00),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 10.40),
            ("2020-02-02 11:10", 11.50),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.70),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 10.80),
            ("2020-02-02 11:10", 11.50),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.70),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 10.80),
            ("2020-02-02 11:10", 10.90),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    // 跳空缺口形成底分型
    #[test]
    fn test_segment_gap_with_parting_simple() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.90),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 10.20),
            ("2020-02-02 11:10", 10.90),
            ("2020-02-02 11:20", 10.80),
            ("2020-02-02 11:30", 11.40),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.60),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 10.70),
            ("2020-02-02 11:10", 11.00),
            ("2020-02-02 11:20", 10.40),
            ("2020-02-02 11:30", 10.80),
            ("2020-02-02 13:10", 10.60),
            ("2020-02-02 13:20", 11.15),
        ]
        .build();

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

    // 缺口时逆分型由于包含关系不成立，则线段延续
    #[test]
    fn test_segment_gap_cs_all_inclusive() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.60),
            ("2020-02-02 10:50", 10.80),
            ("2020-02-02 11:00", 10.40),
            ("2020-02-02 11:10", 11.00),
            ("2020-02-02 11:20", 10.70),
            ("2020-02-02 11:30", 11.30),
        ]
        .build();

        let sgs = sks_to_sgs(&sks)?;
        assert_eq!(1, sgs.len());
        Ok(())
    }

    // 跳空缺口被笔破坏
    #[test]
    fn test_segment_gap_broken_by_stroke() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 11.20),
            ("2020-02-02 10:40", 10.90),
            ("2020-02-02 10:50", 11.10),
            ("2020-02-02 11:00", 9.80),
        ]
        .build();
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
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 10.50),
            ("2020-02-02 10:20", 10.30),
            ("2020-02-02 10:30", 10.80),
            ("2020-02-02 10:40", 10.40),
            ("2020-02-02 10:50", 11.30),
            ("2020-02-02 11:00", 10.30),
            ("2020-02-02 11:10", 11.00),
            ("2020-02-02 11:20", 10.70),
            ("2020-02-02 11:30", 11.00),
            ("2020-02-02 13:10", 10.10),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;

        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 13:10"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_segment_first_inverse_to_inverse() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 12.00),
            ("2020-02-02 10:20", 10.20),
            ("2020-02-02 10:30", 11.00),
            ("2020-02-02 10:40", 10.50),
            ("2020-02-02 10:50", 11.50),
            ("2020-02-02 11:00", 10.80),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_segment_first_inverse_to_gap_inverse() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 12.00),
            ("2020-02-02 10:20", 10.20),
            ("2020-02-02 10:30", 11.00),
            ("2020-02-02 10:40", 10.50),
            ("2020-02-02 10:50", 11.50),
            ("2020-02-02 11:00", 11.20),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;
        assert_eq!(1, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:50"), sgs[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_segment_inverse_first_long_stroke_not_inclusive() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 11.00),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:30", 12.00),
            ("2020-02-02 10:40", 10.70),
            ("2020-02-02 10:50", 11.50),
            ("2020-02-02 11:00", 11.00),
            ("2020-02-02 11:10", 11.20),
            ("2020-02-02 11:20", 10.60),
            ("2020-02-02 11:30", 11.00),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;
        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:20"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_segment_inverse_first_long_stroke_inclusive() -> Result<()> {
        let sks = vec![
            ("2020-02-02 10:00", 10.00),
            ("2020-02-02 10:10", 11.00),
            ("2020-02-02 10:20", 10.50),
            ("2020-02-02 10:30", 12.00),
            ("2020-02-02 10:40", 10.70),
            ("2020-02-02 10:50", 11.50),
            ("2020-02-02 11:00", 11.00),
            ("2020-02-02 11:10", 11.20),
            ("2020-02-02 11:20", 10.80),
            ("2020-02-02 11:30", 11.00),
        ]
        .build();
        let sgs = sks_to_sgs(&sks)?;
        assert_eq!(2, sgs.len());
        assert_eq!(new_ts("2020-02-02 10:00"), sgs[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 10:30"), sgs[1].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-02 11:20"), sgs[1].end_pt.extremum_ts);
        Ok(())
    }

    fn new_sk(start_ts: &str, start_price: f64, end_ts: &str, end_price: f64) -> Stroke {
        let upward = start_price < end_price;
        let start_pt = new_pt_fix_width(start_ts, 1, start_price, 3, !upward);
        let end_pt = new_pt_fix_width(end_ts, 1, end_price, 3, upward);
        Stroke { start_pt, end_pt }
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
            left_gap: None,
            right_gap: None,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    trait BuildStrokeVec {
        fn build(self) -> Vec<Stroke>;
    }

    impl BuildStrokeVec for Vec<(&str, f64)> {
        fn build(self) -> Vec<Stroke> {
            self.iter()
                .zip(self.iter().skip(1))
                .map(|(left, right)| new_sk(left.0, left.1, right.0, right.1))
                .collect()
        }
    }
}
