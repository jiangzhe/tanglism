use crate::parting::PartingDelta;
use crate::shape::{Parting, Stroke};
use crate::stream::{Accumulator, Aggregator, Delta};
use crate::Result;
use bigdecimal::BigDecimal;
use lazy_static::*;
use serde_derive::*;
use std::str::FromStr;
use tanglism_utils::{LocalTradingTimestamps, TradingTimestamps};

/// 将分型序列解析为笔序列
///
/// 步骤：
/// 1. 选择起始点。
/// 2. 选择下一个点。
///    若异型：邻接或交叉则忽略，不邻接则成笔
///    若同型：顶更高/底更低则修改当前笔，反之则忽略
pub fn pts_to_sks(pts: &[Parting], tick: &str, cfg: StrokeConfig) -> Result<Vec<Stroke>> {
    StrokeAccumulator::new(tick, cfg)?.aggregate(pts)
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeConfig {
    pub indep_k: bool,
    pub judge: StrokeJudge,
}

impl Default for StrokeConfig {
    fn default() -> Self {
        StrokeConfig {
            indep_k: false,
            judge: StrokeJudge::GapOpening(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrokeJudge {
    None,
    // 开盘缺口，是否包含下午盘开盘
    GapOpening(bool),
    // 比例缺口
    GapRatio(BigDecimal),
}

lazy_static! {
    static ref GAP_MINIMAL_BASE: BigDecimal = BigDecimal::from_str("0.01").unwrap();
    static ref GAP_ZERO: BigDecimal = BigDecimal::from(0);
}

pub type StrokeDelta = Delta<Stroke>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CStroke {
    pub sk: Stroke,
    pub orig: Option<Box<CStroke>>,
}

pub struct StrokeAccumulator<T> {
    tts: T,
    state: Vec<CStroke>,
    pending: Vec<Parting>,
    cfg: StrokeConfig,
}

impl StrokeAccumulator<LocalTradingTimestamps> {
    // only 1m, 5m, 30m, 1d are allowed
    pub fn new(tick: &str, cfg: StrokeConfig) -> Result<Self> {
        let tts = LocalTradingTimestamps::new(tick)?;
        Ok(StrokeAccumulator {
            tts,
            state: Vec::new(),
            pending: Vec::new(),
            cfg,
        })
    }

    pub fn delta_agg(self) -> StrokeAggregator<LocalTradingTimestamps> {
        StrokeAggregator {
            acc: self,
            ds: Vec::new(),
        }
    }
}

impl<T: TradingTimestamps> StrokeAccumulator<T> {
    pub fn new_with_tts(tts: T, cfg: StrokeConfig) -> Result<StrokeAccumulator<T>> {
        Ok(StrokeAccumulator {
            tts,
            state: Vec::new(),
            pending: Vec::new(),
            cfg,
        })
    }

    fn accumulate_add(&mut self, item: &Parting) -> Result<StrokeDelta> {
        // 存在前一笔时，比较当前的分型是否与前一笔的终点分型类型一致
        // 如果一致，则比较高低，并根据情况修改笔或丢弃
        if let Some(csk) = self.state.last() {
            // 比较方向
            if csk.sk.end_pt.top == item.top {
                // 顶比前顶高，或者底比前底低，直接修改该笔
                if (item.top && item.extremum_price > csk.sk.end_pt.extremum_price)
                    || (!item.top && item.extremum_price < csk.sk.end_pt.extremum_price)
                {
                    let csk = self.state.pop().unwrap();
                    let new_sk = CStroke {
                        sk: Stroke {
                            start_pt: csk.sk.start_pt.clone(),
                            end_pt: item.clone(),
                        },
                        orig: Some(Box::new(csk)),
                    };
                    self.state.push(new_sk);
                    return Ok(StrokeDelta::Update(
                        self.state.last().map(cstroke_to_stroke).unwrap(),
                    ));
                }
                // 顶比前顶低，或底比前底高，则忽略
                return Ok(StrokeDelta::None);
            }
            // 异向顶底间满足顶比底高，且符合成笔条件（如存在独立K线）
            if (item.top && item.extremum_price > csk.sk.end_pt.extremum_price)
                || (!item.top && item.extremum_price < csk.sk.end_pt.extremum_price)
            {
                if self.stroke_completed(&csk.sk.end_pt, &item) {
                    // 成笔
                    let new_sk = Stroke {
                        start_pt: csk.sk.end_pt.clone(),
                        end_pt: item.clone(),
                    };
                    self.state.push(stroke_to_cstroke(&new_sk));
                    return Ok(StrokeDelta::Add(new_sk));
                }
                // 在不成笔时，不考虑回溯，因为回溯将影响之前已经完成的两笔
                return Ok(StrokeDelta::None);
            }
            // 不满足任一成笔条件则丢弃
            return Ok(StrokeDelta::None);
        }

        // 不存在前一笔，则需要和未成笔的潜在起点序列进行比较
        let mut matches = Vec::new();
        for p in &self.pending {
            // 方向不同且顶比底高
            if item.top != p.top
                && ((item.top && item.extremum_price > p.extremum_price)
                    || (!item.top && item.extremum_price < p.extremum_price))
            {
                // 成笔逻辑
                if self.stroke_completed(&p, &item) {
                    // 成笔
                    let new_sk = CStroke {
                        sk: Stroke {
                            start_pt: p.clone(),
                            end_pt: item.clone(),
                        },
                        orig: None,
                    };
                    matches.push(new_sk);
                }
            }
        }
        // 与未成笔序列无法成笔时，加入未成笔序列
        if matches.is_empty() {
            self.pending.push(item.clone());
            return Ok(StrokeDelta::None);
        }
        // 在是否成笔的判断中，我们取差距更大的分型作为起点，
        // 即如果有多个底可以和顶分型构成一笔，这里取较低的底。
        // 反之亦然。
        let mut r = matches.pop().unwrap();
        while let Some(m) = matches.pop() {
            if (&r.sk.start_pt.extremum_price - &r.sk.end_pt.extremum_price).abs()
                < (&m.sk.start_pt.extremum_price - &m.sk.end_pt.extremum_price).abs()
            {
                r = m;
            }
        }
        self.state.push(r);
        // 不删除pending队列，仅第一笔使用
        // 收到分型更新时需要回溯该队列
        Ok(StrokeDelta::Add(
            self.state.last().map(cstroke_to_stroke).unwrap(),
        ))
    }

    fn accumulate_update(&mut self, item: &Parting) -> Result<StrokeDelta> {
        if let Some(csk) = self.state.last() {
            // 存在上一笔时，检查上一笔的结束分型是否匹配
            // 使用start_ts比较，start_ts不变
            if csk.sk.end_pt.start_ts == item.start_ts {
                // 匹配则删除上一笔
                let mut deleted = self.state.pop().unwrap();
                if self.state.is_empty() {
                    if let Some(last_pending) = self.pending.last() {
                        if last_pending.start_ts == item.start_ts {
                            self.pending.pop();
                        }
                    }
                }
                match self.accumulate_add(item)? {
                    StrokeDelta::None => {
                        if let Some(orig) = deleted.orig.take() {
                            let delta = StrokeDelta::Update(cstroke_to_stroke(&orig));
                            self.state.push(*orig);
                            return Ok(delta);
                        } else {
                            return Ok(StrokeDelta::Delete(cstroke_to_stroke(&deleted)));
                        }
                    }
                    StrokeDelta::Add(new) => {
                        // 将前一笔更新进新笔的orig变量
                        self.state.last_mut().unwrap().orig = Some(Box::new(deleted));
                        return Ok(StrokeDelta::Update(new));
                    }
                    _ => unreachable!(),
                };
            }
            // 不匹配，按照add处理
            return self.accumulate_add(item);
        }
        // 不存在上一笔时，检查pending队列
        if let Some(last_pending) = self.pending.last() {
            if last_pending.start_ts == item.start_ts {
                self.pending.pop();
            }
        }
        self.accumulate_add(item)
    }

    fn accumulate_delete(&mut self, item: &Parting) -> Result<StrokeDelta> {
        if let Some(csk) = self.state.last_mut() {
            // 存在上一笔
            if csk.sk.end_pt.start_ts == item.start_ts {
                // 匹配笔的结束分型
                if let Some(orig) = csk.orig.take() {
                    // 该笔是有前一笔修改得来
                    let delta = StrokeDelta::Update(cstroke_to_stroke(&orig));
                    self.state.pop();
                    self.state.push(*orig);
                    return Ok(delta);
                }
                // 该笔不再完整，删除
                let deleted = self.state.pop().unwrap();
                return Ok(StrokeDelta::Delete(cstroke_to_stroke(&deleted)));
            }
        }
        // 不存在上一笔
        if let Some(last_pt) = self.pending.last() {
            // pending队列非空
            if last_pt.start_ts == item.start_ts {
                // 匹配pending队列最后以分型
                self.pending.pop();
                return Ok(StrokeDelta::None);
            }
        }
        unreachable!()
    }

    // 成笔逻辑检查
    // p1为前分型，p2为后分型
    // 兜底策略为独立K线
    #[inline]
    fn stroke_completed(&self, p1: &Parting, p2: &Parting) -> bool {
        use tanglism_utils::{AFTERNOON_END, MORNING_END};
        if self.cfg.indep_k {
            // 必须存在独立K线
            if let Some(indep_ts) = self.tts.next_tick(p1.end_ts) {
                if indep_ts < p2.start_ts {
                    return true;
                }
            }
        } else {
            // 不必存在独立K线
            if let Some(indep_ts) = self.tts.next_tick(p1.end_ts) {
                if indep_ts <= p2.start_ts {
                    return true;
                }
            }
        }
        // 特殊成笔逻辑
        match self.cfg.judge {
            StrokeJudge::GapOpening(afternoon) => {
                if p1.right_gap.is_some() {
                    // 最高/低价恰好收盘
                    if p1.extremum_ts.time() == *AFTERNOON_END {
                        return true;
                    }
                    // 中午收盘
                    if afternoon && p1.extremum_ts.time() == *MORNING_END {
                        return true;
                    }
                }
                if p2.left_gap.is_some() {
                    // 最高/低价恰好收盘
                    if let Some(prev_tick) = self.tts.prev_tick(p2.extremum_ts) {
                        if prev_tick.time() == *AFTERNOON_END {
                            return true;
                        }
                        // 中午收盘
                        if afternoon && prev_tick.time() == *MORNING_END {
                            return true;
                        }
                    }
                }
            }
            StrokeJudge::GapRatio(ref ratio) => {
                if let Some(ref g1) = p1.right_gap {
                    let ratio = ratio.clone();
                    let mut diff = &g1.end_price - &g1.start_price;
                    if diff < *GAP_ZERO {
                        diff = -diff;
                    }
                    if g1.start_price == *GAP_ZERO {
                        return diff / &*GAP_MINIMAL_BASE >= ratio;
                    }
                    return diff / &g1.start_price >= ratio;
                }
            }
            StrokeJudge::None => (),
        }
        false
    }
}

/// 接收分型变更的累加器
impl<T: TradingTimestamps> Accumulator<PartingDelta> for StrokeAccumulator<T> {
    type Delta = StrokeDelta;
    type State = Vec<CStroke>;

    // 依序消费每个分型
    // 可以根据当前起点的分型，分为同类型，和不同类型
    // 1. 顶底、底顶：可连成一笔
    // 2. 顶顶、底底：无法连成一笔，但需要考虑如果底比前底低，
    //    或者顶比前顶高，则需要修改前一笔的终点为该分型
    fn accumulate(&mut self, item: &PartingDelta) -> Result<StrokeDelta> {
        match item {
            PartingDelta::None => Ok(StrokeDelta::None),
            PartingDelta::Add(add) => self.accumulate_add(add),
            PartingDelta::Update(update) => self.accumulate_update(update),
            PartingDelta::Delete(delete) => self.accumulate_delete(delete),
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

/// 接收分型(只处理新增)的累加器
impl<T: TradingTimestamps> Accumulator<Parting> for StrokeAccumulator<T> {
    type Delta = StrokeDelta;
    type State = Vec<CStroke>;

    fn accumulate(&mut self, item: &Parting) -> Result<Self::Delta> {
        self.accumulate_add(item)
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

/// 接收分型数组的聚合器
impl<T: TradingTimestamps> Aggregator<&[Parting], Vec<Stroke>> for StrokeAccumulator<T> {
    fn aggregate(mut self, input: &[Parting]) -> Result<Vec<Stroke>> {
        for item in input {
            self.accumulate_add(item)?;
        }
        Ok(self.state.iter().map(cstroke_to_stroke).collect())
    }
}

pub struct StrokeAggregator<T> {
    acc: StrokeAccumulator<T>,
    ds: Vec<StrokeDelta>,
}

impl StrokeAggregator<LocalTradingTimestamps> {
    pub fn new(tick: &str, cfg: StrokeConfig) -> Result<Self> {
        Ok(StrokeAggregator {
            acc: StrokeAccumulator::new(tick, cfg)?,
            ds: Vec::new(),
        })
    }
}

impl<T: TradingTimestamps> Aggregator<&[PartingDelta], Vec<StrokeDelta>> for StrokeAggregator<T> {
    fn aggregate(mut self, input: &[PartingDelta]) -> Result<Vec<StrokeDelta>> {
        for item in input {
            match self.acc.accumulate(item)? {
                StrokeDelta::None => (),
                delta => self.ds.push(delta),
            }
        }
        Ok(self.ds)
    }
}

pub fn cstroke_to_stroke(csk: &CStroke) -> Stroke {
    csk.sk.clone()
}

pub fn stroke_to_cstroke(sk: &Stroke) -> CStroke {
    CStroke {
        sk: sk.clone(),
        orig: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;
    use tanglism_utils::TradingTimestamps;

    #[test]
    fn test_stroke_none() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:01", 10.10, true),
            new_pt1("2020-01-07 10:03", 9.50, false),
        ]);
        assert!(sks.is_empty());
        Ok(())
    }

    #[test]
    fn test_stroke_one_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:10", 10.40, true),
            new_pt1("2020-01-07 10:12", 10.30, false),
        ]);
        assert_eq!(1, sks.len());
        Ok(())
    }

    #[test]
    fn test_stroke_one_delta_update() -> Result<()> {
        let sds = pds_to_sds_1_min(vec![
            PartingDelta::Add(new_pt1("2020-01-07 10:00", 10.00, false)),
            PartingDelta::Add(new_pt1("2020-01-07 10:10", 11.00, true)),
            PartingDelta::Update(new_pt1("2020-01-07 10:10", 10.50, true)),
        ]);
        assert_eq!(2, sds.len());
        assert_eq!(
            BigDecimal::from(11.0),
            sds[0].add().unwrap().end_pt.extremum_price
        );
        assert_eq!(
            BigDecimal::from(10.5),
            sds[1].update().unwrap().end_pt.extremum_price
        );
        Ok(())
    }

    #[test]
    fn test_stroke_one_delta_delete() -> Result<()> {
        let sds = pds_to_sds_1_min(vec![
            PartingDelta::Add(new_pt1("2020-01-07 10:00", 10.00, false)),
            PartingDelta::Add(new_pt1("2020-01-07 10:10", 11.00, true)),
            PartingDelta::Delete(new_pt1("2020-01-07 10:10", 11.00, true)),
        ]);
        assert_eq!(2, sds.len());
        sds[0].add().unwrap();
        sds[1].delete().unwrap();
        Ok(())
    }

    #[test]
    fn test_stroke_one_delta_backtrack() -> Result<()> {
        let sds = pds_to_sds_1_min(vec![
            PartingDelta::Add(new_pt1("2020-01-07 10:00", 10.00, false)),
            PartingDelta::Add(new_pt1("2020-01-07 10:10", 11.00, true)),
            PartingDelta::Add(new_pt1("2020-01-07 10:20", 12.00, true)),
            PartingDelta::Delete(new_pt1("2020-01-07 10:20", 12.00, true)),
        ]);
        assert!(sds[0].add().is_some());
        assert!(sds[1].update().is_some());
        let update = sds[2].update().unwrap();
        assert_eq!(BigDecimal::from(11.0), update.end_pt.extremum_price);
        Ok(())
    }

    // 一笔，起点移动，因为起点不是最低的底分型
    #[test]
    fn test_stroke_one_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:02", 10.10, true),
            new_pt1("2020-01-07 10:04", 9.90, false),
            new_pt1("2020-01-07 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:04"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-07 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 一笔，起点不一定，因为起点是最低的底分型
    #[test]
    fn test_stroke_one_non_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:02", 10.10, true),
            new_pt1("2020-01-07 10:04", 10.02, false),
            new_pt1("2020-01-07 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-07 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 简单的两笔
    #[test]
    fn test_stroke_two_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:10", 10.10, true),
            new_pt1("2020-01-07 10:20", 10.02, false),
        ]);
        assert_eq!(2, sks.len());
        Ok(())
    }

    // 一笔，分型跨天
    #[test]
    fn test_stroke_one_across_days() -> Result<()> {
        let sks = pts_to_sks_30_min(vec![
            new_pt30("2020-01-07 10:00", 10.00, true),
            new_pt30("2020-01-08 10:00", 9.50, false),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-08 10:00"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 一笔，开盘跳空，2020.3.11 ~ 2020.3.13 贵州茅台
    // 1. 后底低于前底
    // 2. K线包含
    #[test]
    fn test_stroke_one_complex_across_days() -> Result<()> {
        let sks = pts_to_sks_30_min(vec![
            ts_pt30(
                "2020-03-11 13:30",
                1169.50,
                true,
                "2020-03-11 11:00",
                "2020-03-11 14:00",
            ),
            ts_pt30(
                "2020-03-11 14:00",
                1156.70,
                false,
                "2020-03-11 11:30",
                "2020-03-11 14:30",
            ),
            ts_pt30(
                "2020-03-11 15:00",
                1167.40,
                true,
                "2020-03-11 14:30",
                "2020-03-12 10:00",
            ),
            ts_pt30(
                "2020-03-12 10:30",
                1125.10,
                false,
                "2020-03-12 10:00",
                "2020-03-12 14:00",
            ),
            ts_pt30(
                "2020-03-12 11:00",
                1147.98,
                true,
                "2020-03-12 10:30",
                "2020-03-12 15:00",
            ),
            ts_pt30(
                "2020-03-13 10:00",
                1080.00,
                false,
                "2020-03-12 14:30",
                "2020-03-13 14:00",
            ),
            ts_pt30(
                "2020-03-13 13:30",
                1128.92,
                true,
                "2020-03-13 10:00",
                "2020-03-13 15:00",
            ),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-03-11 13:30"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-03-13 10:00"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 三笔，2020.2.10 ~ 2020.2.14 贵州茅台
    // todo
    #[test]
    fn test_stroke_three() -> Result<()> {
        let pts = vec![
            ts_pt30(
                "2020-02-10 11:00",
                1074.56,
                true,
                "2020-02-10 10:30",
                "2020-02-10 11:30",
            ),
            ts_pt30(
                "2020-02-10 13:30",
                1061.80,
                false,
                "2020-02-10 11:30",
                "2020-02-10 14:00",
            ),
            ts_pt30(
                "2020-02-10 14:00",
                1067.00,
                true,
                "2020-02-10 13:30",
                "2020-02-10 15:00",
            ),
            ts_pt30(
                "2020-02-10 15:00",
                1062.01,
                false,
                "2020-02-10 14:00",
                "2020-02-11 10:00",
            ),
            ts_pt30(
                "2020-02-11 14:00",
                1099.66,
                true,
                "2020-02-11 11:00",
                "2020-02-12 10:00",
            ),
            ts_pt30(
                "2020-02-12 10:30",
                1085.88,
                false,
                "2020-02-12 10:00",
                "2020-02-12 11:00",
            ),
            ts_pt30(
                "2020-02-12 11:30",
                1098.79,
                true,
                "2020-02-12 11:00",
                "2020-02-12 14:00",
            ),
            ts_pt30(
                "2020-02-12 13:30",
                1090.30,
                false,
                "2020-02-12 11:30",
                "2020-02-12 14:30",
            ),
            ts_pt30(
                "2020-02-13 10:00",
                1113.83,
                true,
                "2020-02-12 15:00",
                "2020-02-13 11:00",
            ),
            ts_pt30(
                "2020-02-13 13:30",
                1088.21,
                false,
                "2020-02-13 11:30",
                "2020-02-13 15:00",
            ),
            ts_pt30(
                "2020-02-13 14:30",
                1093.64,
                true,
                "2020-02-13 13:30",
                "2020-02-14 11:00",
            ),
            ts_pt30(
                "2020-02-14 10:00",
                1086.01,
                false,
                "2020-02-13 14:30",
                "2020-02-14 11:30",
            ),
            ts_pt30(
                "2020-02-14 11:30",
                1092.00,
                true,
                "2020-02-14 10:00",
                "2020-02-14 13:30",
            ),
            ts_pt30(
                "2020-02-14 14:30",
                1083.11,
                false,
                "2020-02-14 13:30",
                "2020-02-14 15:00",
            ),
        ];
        // 不回溯
        let sks1 = pts_to_sks(
            &pts,
            "30m",
            StrokeConfig {
                indep_k: true,
                judge: StrokeJudge::None,
            },
        )?;
        assert_eq!(3, sks1.len());
        assert_eq!(new_ts("2020-02-10 11:00"), sks1[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-10 15:00"), sks1[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-11 14:00"), sks1[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-14 14:30"), sks1[2].end_pt.extremum_ts);
        Ok(())
    }

    // 测试不同的成笔逻辑选项
    #[test]
    fn test_stroke_one_gap() -> Result<()> {
        let mut pt1 = new_pt30("2020-02-13 15:00", 10.00, false);
        pt1.right_gap = Some(Box::new(Gap {
            ts: new_ts("2020-02-14 10:00"),
            start_price: BigDecimal::from(10.00),
            end_price: BigDecimal::from(10.50),
        }));
        let mut pt2 = new_pt30("2020-02-14 10:00", 10.50, true);
        pt2.left_gap = Some(Box::new(Gap {
            ts: new_ts("2020-02-13 15:00"),
            start_price: BigDecimal::from(10.00),
            end_price: BigDecimal::from(10.50),
        }));
        let pts = vec![pt1, pt2];
        let sks1 = StrokeAccumulator::new(
            "30m",
            StrokeConfig {
                indep_k: true,
                judge: StrokeJudge::None,
            },
        )?
        .aggregate(&pts)
        .unwrap();
        assert_eq!(0, sks1.len());
        let sks2 = StrokeAccumulator::new(
            "30m",
            StrokeConfig {
                indep_k: false,
                judge: StrokeJudge::None,
            },
        )?
        .aggregate(&pts)
        .unwrap();
        assert_eq!(0, sks2.len());
        let sks3 = StrokeAccumulator::new(
            "30m",
            StrokeConfig {
                indep_k: true,
                judge: StrokeJudge::GapOpening(false),
            },
        )?
        .aggregate(&pts)
        .unwrap();
        assert_eq!(1, sks3.len());
        let sks4 = StrokeAccumulator::new(
            "30m",
            StrokeConfig {
                indep_k: true,
                judge: StrokeJudge::GapRatio(BigDecimal::from(0.01)),
            },
        )?
        .aggregate(&pts)
        .unwrap();
        assert_eq!(1, sks4.len());
        let sks5 = StrokeAccumulator::new(
            "30m",
            StrokeConfig {
                indep_k: true,
                judge: StrokeJudge::GapRatio(BigDecimal::from(0.08)),
            },
        )?
        .aggregate(&pts)
        .unwrap();
        assert_eq!(0, sks5.len());
        Ok(())
    }

    fn pts_to_sks_1_min(pts: Vec<Parting>) -> Vec<Stroke> {
        pts_to_sks(&pts, "1m", StrokeConfig::default()).unwrap()
    }

    fn pds_to_sds_1_min(pds: Vec<PartingDelta>) -> Vec<StrokeDelta> {
        StrokeAccumulator::new("1m", StrokeConfig::default())
            .unwrap()
            .delta_agg()
            .aggregate(&pds)
            .unwrap()
    }

    fn new_pt1(ts: &str, price: f64, top: bool) -> Parting {
        let ts1m = LocalTradingTimestamps::new("1m").unwrap();
        let extremum_ts = new_ts(ts);
        let start_ts = ts1m.prev_tick(extremum_ts).unwrap();
        let end_ts = ts1m.next_tick(extremum_ts).unwrap();
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n: 3,
            top,
            left_gap: None,
            right_gap: None,
        }
    }

    fn pts_to_sks_30_min(pts: Vec<Parting>) -> Vec<Stroke> {
        pts_to_sks(&pts, "30m", StrokeConfig::default()).unwrap()
    }

    fn new_pt30(ts: &str, price: f64, top: bool) -> Parting {
        let ts30m = LocalTradingTimestamps::new("30m").unwrap();
        let extremum_ts = new_ts(ts);
        let start_ts = ts30m.prev_tick(extremum_ts).unwrap();
        let end_ts = ts30m.next_tick(extremum_ts).unwrap();
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n: 3,
            top,
            left_gap: None,
            right_gap: None,
        }
    }

    fn ts_pt30(ts: &str, price: f64, top: bool, start_ts: &str, end_ts: &str) -> Parting {
        let start_ts = new_ts(start_ts);
        let end_ts = new_ts(end_ts);
        let extremum_ts = new_ts(ts);
        let mut start = start_ts;
        let mut n = 1;
        while let Some(next_ts) = LocalTradingTimestamps::new("30m").unwrap().next_tick(start) {
            n += 1;
            if next_ts == end_ts {
                break;
            }
            if n > 100 {
                panic!("exceeds max n of single parting");
            }
            start = next_ts;
        }

        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n,
            top,
            left_gap: None,
            right_gap: None,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
