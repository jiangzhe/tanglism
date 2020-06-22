use crate::shape::{Gap, Parting, PriceRange, K};
use crate::stream::{Accumulator, Aggregator, Delta, Replicator};
use crate::Result;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;

/// 合并K线
///
/// 合并K线，当相邻K线出现包含关系时，合并为一根K线
/// 包含原则简述：假设a, b为相邻K线，当a的最高价比b的最高价高，且a的最低价比b的最
/// 低价低时，满足包含原则，两K线可视为1条K线。在上升时，取两高点的高点为新K线高点，
/// 取两低点的高点为新K线低点。在下降时，取两高点的低点为新K线高点，取两低点的低点
/// 为新K线的低点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CK {
    pub start_ts: NaiveDateTime,
    pub end_ts: NaiveDateTime,
    pub extremum_ts: NaiveDateTime,
    pub low: BigDecimal,
    pub high: BigDecimal,
    pub n: i32,
    // 价格区间，用于进行缺口判断
    pub price_range: Option<Box<PriceRange>>,
    // 合并前复制
    pub orig: Option<Box<CK>>,
}

impl CK {
    #[inline]
    pub fn start_high(&self) -> &BigDecimal {
        self.price_range
            .as_ref()
            .map(|pr| &pr.start_high)
            .unwrap_or(&self.high)
    }

    #[inline]
    pub fn start_low(&self) -> &BigDecimal {
        self.price_range
            .as_ref()
            .map(|pr| &pr.start_low)
            .unwrap_or(&self.low)
    }

    #[inline]
    pub fn end_high(&self) -> &BigDecimal {
        self.price_range
            .as_ref()
            .map(|pr| &pr.end_high)
            .unwrap_or(&self.high)
    }

    #[inline]
    pub fn end_low(&self) -> &BigDecimal {
        self.price_range
            .as_ref()
            .map(|pr| &pr.end_low)
            .unwrap_or(&self.low)
    }
}

/// 将K线图解析为分型序列
pub fn ks_to_pts(ks: &[K]) -> Result<Vec<Parting>> {
    PartingAccumulator::new().aggregate(ks)
}

/// 暂时留空
#[derive(Debug, Clone, Default)]
pub struct PartingConfig {
    pub inclusive_k: bool,
}

pub type KDelta = Delta<K>;
pub type PartingDelta = Delta<Parting>;

/// 实现分型累加器
#[derive(Debug, Clone)]
pub struct PartingAccumulator {
    state: Vec<Parting>,
    /// 暂存K线数组，当数组中存在3根K线时，必定与前一分型对应
    tmp: Vec<CK>,
    upward: bool,
}

impl PartingAccumulator {
    pub fn new() -> Self {
        PartingAccumulator {
            state: Vec::new(),
            tmp: Vec::new(),
            upward: true,
        }
    }

    #[allow(dead_code)]
    pub fn delta_agg(self) -> PartingAggregator {
        PartingAggregator {
            acc: self,
            ds: Vec::new(),
        }
    }

    fn accumulate_add(&mut self, item: &K) -> Result<PartingDelta> {
        // k1不存在
        if self.tmp.is_empty() {
            return self.insert1(item);
        }

        // k2不存在
        if self.tmp.len() < 2 {
            return self.insert2(item);
        }

        // k3不存在
        if self.tmp.len() < 3 {
            return self.insert3(item);
        }

        self.insert4(item)
    }

    // 插入第一根K线
    fn insert1(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert!(self.tmp.is_empty());
        self.tmp.push(k_to_ck(item));
        Ok(PartingDelta::None)
    }

    // 插入第二根K线
    fn insert2(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(1, self.tmp.len());
        debug_assert!(self.tmp.last().unwrap().end_ts < item.ts);
        let k1 = self.tmp.first().unwrap();
        if let Some(ck) = inclusive_neighbor_k(k1, item, self.upward) {
            // 更新
            *self.tmp.last_mut().unwrap() = ck;
            return Ok(PartingDelta::None);
        }
        // 插入
        self.upward = item.high > k1.high;
        self.tmp.push(k_to_ck(item));
        Ok(PartingDelta::None)
    }

    // 更新第一根K线
    fn update1(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(1, self.tmp.len());
        debug_assert!(self.tmp.last().unwrap().end_ts == item.ts);
        *self.tmp.first_mut().unwrap() = k_to_ck(item);
        Ok(PartingDelta::None)
    }

    // 插入第三根K线
    fn insert3(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(2, self.tmp.len());
        debug_assert!(self.tmp.last().unwrap().end_ts < item.ts);

        let k2 = self.tmp.last().unwrap();
        // 检查k2与k的包含关系
        if let Some(ck) = inclusive_neighbor_k(k2, item, self.upward) {
            // 更新k2
            *self.tmp.last_mut().unwrap() = ck;
            return Ok(PartingDelta::None);
        }

        let k1 = self.tmp.first().unwrap();
        // 检查k1, k2与k是否形成顶/底分型
        if (self.upward && item.low < k2.low) || (!self.upward && item.high > k2.high) {
            // 形成顶/底分型，更新k2和k3，并将走势颠倒
            let ck = k_to_ck(item);
            let parting = create_parting(k1, k2, &ck, self.upward);
            self.state.push(parting.clone());
            self.tmp.push(ck);
            self.upward = !self.upward;
            return Ok(PartingDelta::Add(parting));
        }
        // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变
        self.tmp.remove(0);
        self.tmp.push(k_to_ck(item));
        Ok(PartingDelta::None)
    }

    // 更新第二根K线
    fn update2(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(2, self.tmp.len());
        debug_assert!(self.tmp.last().unwrap().end_ts == item.ts);
        // 等价于删除并插入第二根
        self.tmp.pop();
        self.insert2(item)
    }

    // 插入第四根K线，前三根K线必定已形成分型
    fn insert4(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(3, self.tmp.len());
        debug_assert!(!self.state.is_empty());
        debug_assert!(self.state.last().unwrap().end_ts < item.ts);

        let k3 = self.tmp.last().unwrap();
        let k2 = self.tmp.get(1).unwrap();
        // 检查k3与k的包含关系
        if let Some(ck) = inclusive_neighbor_k(k3, item, self.upward) {
            let orig_upward = !self.upward;
            let k1 = self.tmp.get(0).unwrap();
            // 使用新合并的K线构造新分型，此时的走向是与K线走向相反的
            let parting = create_parting(k1, k2, &ck, orig_upward);
            *self.state.last_mut().unwrap() = parting.clone();
            // 更新k3
            *self.tmp.last_mut().unwrap() = ck;
            // return Ok(PartingDelta::None);
            return Ok(PartingDelta::Update(parting));
        }

        // 不包含
        // 当k2, k3, k形成顶底分型时，左移1位
        if (self.upward && item.low < k3.low) || (!self.upward && item.high > k3.high) {
            let ck = k_to_ck(item);
            let parting = create_parting(k2, k3, &ck, self.upward);
            self.state.push(parting.clone());
            self.tmp.remove(0);
            self.tmp.push(ck);
            self.upward = !self.upward;
            return Ok(PartingDelta::Add(parting));
        }

        // 不形成分型时，将k3, k向左移两位
        self.upward = item.high > k3.high;
        drop(self.tmp.drain(0..2));
        self.tmp.push(k_to_ck(item));
        Ok(PartingDelta::None)
    }

    // 更新第三根K线，注意
    fn update3(&mut self, item: &K) -> Result<PartingDelta> {
        debug_assert_eq!(3, self.tmp.len());
        debug_assert!(self.state.last().unwrap().end_ts == item.ts);
        let deleted = self.state.pop().unwrap();
        self.tmp.pop();
        self.upward = !self.upward;
        let rst = match self.insert3(item)? {
            PartingDelta::None => PartingDelta::Delete(deleted),
            PartingDelta::Add(pt) => PartingDelta::Update(pt),
            _ => unreachable!(),
        };
        Ok(rst)
    }

    // todo
    // update时，对包含关系的处理可能导致不同的结果，需要对CK进行还原
    fn accumulate_update(&mut self, item: &K) -> Result<PartingDelta> {
        // k1不存在
        if self.tmp.is_empty() {
            panic!("no k to update");
        }

        // k2不存在
        if self.tmp.len() < 2 {
            if let Some(orig) = self.tmp.last_mut().unwrap().orig.take() {
                // 回溯
                *self.tmp.last_mut().unwrap() = *orig;
                return self.insert2(item);
            }
            // 无需回溯
            return self.update1(item);
        }

        // k3不存在
        if self.tmp.len() < 3 {
            if let Some(orig) = self.tmp.last_mut().unwrap().orig.take() {
                // 回溯
                *self.tmp.last_mut().unwrap() = *orig;
                return self.insert3(item);
            }
            // 无需回溯
            return self.update2(item);
        }

        // k3存在
        if let Some(orig) = self.tmp.last_mut().unwrap().orig.take() {
            // 回溯
            *self.tmp.last_mut().unwrap() = *orig;
            // 回溯前的三根K线必定构成分型
            let k1 = self.tmp.first().unwrap();
            let k2 = self.tmp.get(1).unwrap();
            let k3 = self.tmp.last().unwrap();
            let orig_upward = !self.upward;
            let orig_pt = create_parting(k1, k2, k3, orig_upward);
            *self.state.last_mut().unwrap() = orig_pt;
            self.upward = orig_upward;
            return self.insert4(item);
        }

        // 无需回溯
        self.update3(item)
    }
}

/// 接收K线变更的累加器
impl Accumulator<KDelta> for PartingAccumulator {
    type Delta = PartingDelta;
    type State = Vec<Parting>;

    fn accumulate(&mut self, item: &KDelta) -> Result<Self::Delta> {
        match item {
            KDelta::Add(add) => self.accumulate_add(add),
            KDelta::Update(update) => self.accumulate_update(update),
            KDelta::None => Ok(PartingDelta::None),
            KDelta::Delete(_) => unreachable!(),
        }
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

/// 接收K线（仅支持新增）的累加器
impl Accumulator<K> for PartingAccumulator {
    type Delta = PartingDelta;
    type State = Vec<Parting>;

    fn accumulate(&mut self, item: &K) -> Result<Self::Delta> {
        self.accumulate_add(item)
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

/// 接收K线变更数组的聚合器
impl Aggregator<&[KDelta], Vec<Parting>> for PartingAccumulator {
    fn aggregate(mut self, input: &[KDelta]) -> Result<Vec<Parting>> {
        for item in input {
            self.accumulate(item)?;
        }
        Ok(self.state)
    }
}

/// 接收K线数组的聚合器
impl Aggregator<&[K], Vec<Parting>> for PartingAccumulator {
    fn aggregate(mut self, input: &[K]) -> Result<Vec<Parting>> {
        for item in input {
            self.accumulate(item)?;
        }
        Ok(self.state)
    }
}

pub struct PartingAggregator {
    acc: PartingAccumulator,
    ds: Vec<PartingDelta>,
}

/// 过滤并输出PartingDelta
impl Aggregator<&[KDelta], Vec<PartingDelta>> for PartingAggregator {
    fn aggregate(mut self, input: &[KDelta]) -> Result<Vec<PartingDelta>> {
        for item in input {
            match self.acc.accumulate(item)? {
                PartingDelta::None => (),
                delta => self.ds.push(delta),
            }
        }
        Ok(self.ds)
    }
}

pub struct PartingReplicator {
    state: Vec<Parting>,
}

impl PartingReplicator {
    #[allow(dead_code)]
    pub fn new() -> Self {
        PartingReplicator { state: Vec::new() }
    }
}

impl Replicator for PartingReplicator {
    type Delta = PartingDelta;
    type State = Vec<Parting>;

    fn replicate(&mut self, delta: Self::Delta) -> Result<()> {
        match delta {
            PartingDelta::None => {}
            PartingDelta::Add(pt) => self.state.push(pt),
            PartingDelta::Update(pt) => *self.state.last_mut().unwrap() = pt,
            PartingDelta::Delete(_) => {
                self.state.pop();
            }
        }
        Ok(())
    }

    fn state(&self) -> &Self::State {
        &self.state
    }
}

fn create_parting(k1: &CK, k2: &CK, k3: &CK, top: bool) -> Parting {
    let left_gap = if top && k1.end_high() < k2.start_low() {
        // 顶分型，k1结束最高价小于k2起始最低价
        Some(Box::new(Gap {
            ts: k2.start_ts,
            start_price: k1.end_high().clone(),
            end_price: k2.start_low().clone(),
        }))
    } else if !top && k1.end_low() > k2.start_high() {
        // 底分型，k1结束最低价大于k2起始最高价
        Some(Box::new(Gap {
            ts: k2.start_ts,
            start_price: k1.end_low().clone(),
            end_price: k2.start_high().clone(),
        }))
    } else {
        None
    };
    let right_gap = if top && k2.end_low() > k3.start_high() {
        // 顶分型，k2结束最低价大于k3起始最高价
        Some(Box::new(Gap {
            ts: k3.start_ts,
            start_price: k2.end_low().clone(),
            end_price: k3.start_high().clone(),
        }))
    } else if !top && k2.end_high() < k3.start_low() {
        // 底分型，k2结束最高价小于k3起始最低价
        Some(Box::new(Gap {
            ts: k3.start_ts,
            start_price: k2.end_high().clone(),
            end_price: k3.start_low().clone(),
        }))
    } else {
        None
    };
    Parting {
        start_ts: k1.start_ts,
        end_ts: k3.end_ts,
        extremum_ts: k2.extremum_ts,
        extremum_price: if top { k2.high.clone() } else { k2.low.clone() },
        n: k1.n + k2.n + k3.n,
        top,
        left_gap,
        right_gap,
    }
}

#[inline]
fn k_to_ck(k: &K) -> CK {
    CK {
        start_ts: k.ts,
        end_ts: k.ts,
        extremum_ts: k.ts,
        high: k.high.clone(),
        low: k.low.clone(),
        n: 1,
        price_range: None,
        orig: None,
    }
}

/// 辅助函数，判断相邻K线是否符合包含关系，并在符合情况下返回包含后的合并K线
fn inclusive_neighbor_k(k1: &CK, k2: &K, upward: bool) -> Option<CK> {
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
            if k1.high > k2.high {
                k1.high.clone()
            } else {
                k2.high.clone()
            },
            if k1.low > k2.low {
                k1.low.clone()
            } else {
                k2.low.clone()
            },
        )
    } else {
        (
            if k1.high < k2.high {
                k1.high.clone()
            } else {
                k2.high.clone()
            },
            if k1.low < k2.low {
                k1.low.clone()
            } else {
                k2.low.clone()
            },
        )
    };

    let price_range = PriceRange {
        start_high: k1
            .price_range
            .as_ref()
            .map(|pr| &pr.start_high)
            .unwrap_or(&k1.high)
            .clone(),
        start_low: k1
            .price_range
            .as_ref()
            .map(|pr| &pr.start_low)
            .unwrap_or(&k1.low)
            .clone(),
        end_high: k2.high.clone(),
        end_low: k2.low.clone(),
    };

    Some(CK {
        start_ts,
        end_ts,
        extremum_ts,
        high,
        low,
        n,
        price_range: Some(Box::new(price_range)),
        orig: Some(Box::new(k1.clone())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;

    #[test]
    fn test_parting_none() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.25, 10.15),
            new_k("2020-02-01 10:04", 10.30, 10.20),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(0, r.len());
        Ok(())
    }

    #[test]
    fn test_parting_one_simple() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.10, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r[0].extremum_ts);
        assert_eq!(BigDecimal::from(10.20), r[0].extremum_price);
        assert_eq!(true, r[0].top);
        Ok(())
    }

    #[test]
    fn test_parting_one_inclusive() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.len());
        assert_eq!(new_ts("2020-02-01 10:04"), r[0].end_ts);
        Ok(())
    }

    #[test]
    fn test_parting_two_simple() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.10),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(true, r[0].top);
        assert_eq!(new_ts("2020-02-01 10:02"), r[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04"), r[1].end_ts);
        assert_eq!(false, r[1].top);
        Ok(())
    }

    #[test]
    fn test_parting_two_indep() -> Result<()> {
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
        assert_eq!(2, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:05"), r[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:07"), r[1].end_ts);
        Ok(())
    }

    #[test]
    fn test_parting_long_inclusive() -> Result<()> {
        let ks = vec![
            new_k("2020-04-01 10:45", 8.85, 8.77),
            new_k("2020-04-01 10:50", 8.84, 8.80),
            new_k("2020-04-01 10:55", 8.83, 8.78),
            new_k("2020-04-01 11:00", 8.83, 8.80),
            new_k("2020-04-01 11:05", 8.82, 8.78),
            new_k("2020-04-01 11:10", 8.81, 8.78),
            // above is one stroke
            new_k("2020-04-01 11:15", 8.82, 8.78),
            new_k("2020-04-01 11:20", 8.82, 8.78),
            new_k("2020-04-01 11:25", 8.82, 8.75),
            new_k("2020-04-01 11:30", 8.79, 8.77),
            new_k("2020-04-01 13:05", 8.79, 8.75),
            // above is one stroke
            new_k("2020-04-01 13:30", 8.83, 8.78),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.len());
        Ok(())
    }

    #[test]
    fn test_parting_delta_simple() -> Result<()> {
        let deltas = vec![
            KDelta::Add(new_k("2020-02-01 10:00", 10.10, 10.00)),
            KDelta::Add(new_k("2020-02-01 10:01", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:02", 10.20, 10.10)),
            KDelta::Add(new_k("2020-02-01 10:03", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:04", 10.10, 10.00)),
        ];
        let pa = PartingAccumulator::new();
        let r: Vec<Parting> = pa.aggregate(deltas.as_slice())?;
        assert_eq!(1, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r[0].extremum_ts);
        assert_eq!(BigDecimal::from(10.20), r[0].extremum_price);
        assert_eq!(true, r[0].top);
        Ok(())
    }

    #[test]
    fn test_parting_delta_update() -> Result<()> {
        let deltas = vec![
            KDelta::Add(new_k("2020-02-01 10:00", 10.10, 10.00)),
            KDelta::Add(new_k("2020-02-01 10:01", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:02", 10.10, 10.00)),
            KDelta::Update(new_k("2020-02-01 10:02", 10.20, 10.10)),
            KDelta::Add(new_k("2020-02-01 10:03", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:04", 10.10, 10.00)),
        ];
        let pa = PartingAccumulator::new();
        let r: Vec<Parting> = pa.aggregate(deltas.as_slice())?;
        assert_eq!(1, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r[0].extremum_ts);
        assert_eq!(BigDecimal::from(10.20), r[0].extremum_price);
        assert_eq!(true, r[0].top);
        Ok(())
    }

    #[test]
    fn test_parting_delta_multi_updates() -> Result<()> {
        let deltas = vec![
            KDelta::Add(new_k("2020-02-01 10:00", 10.10, 10.00)),
            KDelta::Add(new_k("2020-02-01 10:01", 10.10, 10.05)),
            KDelta::Update(new_k("2020-02-01 10:01", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:02", 10.10, 10.00)),
            KDelta::Update(new_k("2020-02-01 10:02", 10.10, 10.05)),
            KDelta::Update(new_k("2020-02-01 10:02", 10.20, 10.10)),
            KDelta::Add(new_k("2020-02-01 10:03", 10.15, 10.05)),
            KDelta::Add(new_k("2020-02-01 10:04", 10.10, 10.00)),
        ];
        let pa = PartingAccumulator::new();
        let r: Vec<Parting> = pa.aggregate(deltas.as_slice())?;
        assert_eq!(1, r.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r[0].extremum_ts);
        assert_eq!(BigDecimal::from(10.20), r[0].extremum_price);
        assert_eq!(true, r[0].top);
        Ok(())
    }

    fn new_k(ts: &str, high: f64, low: f64) -> K {
        K {
            ts: new_ts(ts),
            high: BigDecimal::from(high),
            low: BigDecimal::from(low),
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
