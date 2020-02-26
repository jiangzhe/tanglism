use crate::{Result, Error, Parting, Stroke, Segment, Center, Trend};

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

/// 潜在分型
/// 
/// 在形态合成过程中，可能出现潜在分型，该分型符合分型定义，单并不一定可以跟相邻分型形成笔
pub struct PotentialParting {
    pub op: Option<Parting>,
    pub neighbor_too_close: bool,
}

/// 潜在分型序列
///
/// 包含潜在分型的序列，以及未能形成分型的尾部K线
/// 可通过输入最新K线，更新或延长已有序列
pub struct PotentialPartingSeq {
    pub pts: Vec<PotentialParting>,
    pub tail: Vec<K>,
}

/// 笔序列
/// 
/// 包含笔序列，以及未形成笔的尾部分型
pub struct StrokeSeq {
    pub sks: Vec<Stroke>,
    pub tail: Vec<PotentialParting>,
}

pub struct SegmentSeq {
    pub determined: Vec<Segment>,
    pub undetermined: Vec<Stroke>,
    pub undetermined_top: bool,
}

pub trait Synthesizer {
    // 给定一组连续K线，合成相应的分型序列
    fn init_pts(&mut self, ks: &[K]) -> Result<PotentialPartingSeq>;

    // 给定潜在分型序列，合成相应的笔序列
    fn init_strokes(&mut self, pts: &PotentialPartingSeq) -> Result<StrokeSeq>;

    fn init_segments(&mut self, sks: &[StrokeSeq]) -> Result<SegmentSeq>;
}

pub trait Appender<'a> {
    // 添加最新K线，提供的K线必须满足与序列尾部K线延续
    fn append_ks(&mut self, latest_ks: &[K]) -> Result<()>; 
}