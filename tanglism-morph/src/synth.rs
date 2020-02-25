use crate::{Parting, Stroke, Segment, Center, Trend};

/// K线
/// 
/// 最低级别K线，仅包含时刻，最高点，最低点
pub struct K {
    pub ts: String,
    pub low: f64,
    pub high: f64,
}

/// 单标的确定周期的数据来源
pub trait Source {
    // 给定代码和时刻，获取该时刻前的不多于limit条数的K线数据
    fn data_before(&self, ts: &str, limit: u32) -> Vec<K>;

    // 给定代码和时刻，获取该时刻后的不多余limit条数的K线数据
    fn data_after(&self, ts: &str, limit: u32) -> Vec<K>;
}

pub struct Synthesizer<S: Source> {
    pub code: String,
    pub unit: String,
    pub source: S,
}

impl<S: Source> Synthesizer<S> {

    // pub fn synthesize(&self, ts: &str) -> 
}

