use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;

/// K线
///
/// 缠论的基础概念
/// 在缠论中，K线的开盘价和收盘价被忽略，仅包含时刻，最高点，最低点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K {
    pub ts: NaiveDateTime,
    pub low: BigDecimal,
    pub high: BigDecimal,
}

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
}

/// 分型
///
/// 缠论的基础概念
/// 由三根相邻K线构成
/// 顶分型：中间K线的最高点比两侧K线最高点高，且中间K线的最低点比两侧K线的最低点高
/// 底分型：中间K线的最高点比两侧K线最高点低，且中间K线的最低点比两侧K线的最低点低
/// 在判断分型时，不考虑K线的开盘价和收盘价，近考虑其最高和最低价。
/// 分型实际可由多于3根K线构成，只要两侧的K线满足包含原则。
/// 按照缠论的严格定义，分型仅适用与最小级别的K线图，即1分钟K线图上，后续分析都由
/// 1分钟K线图向上递归构成更大的形态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parting {
    // 分型起始时刻，已考虑K线包含关系
    pub start_ts: NaiveDateTime,
    // 分型结束时刻，已考虑K线包含关系
    pub end_ts: NaiveDateTime,
    // 分型转折时刻
    pub extremum_ts: NaiveDateTime,
    // 转折点价格
    pub extremum_price: BigDecimal,
    // 组成分型的K线数
    pub n: i32,
    // 是否顶分型，非顶即底分型
    pub top: bool,
}

/// 笔
///
/// 缠论的基础概念
/// 由相邻的顶分型与底分型构成，不可同底或同顶，同时需满足两分型间有至少1根独立K线，
/// 即存在1条K线，不属于两侧的分型，且不能因为包含原则属于两侧的分型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub start_pt: Parting,
    pub end_pt: Parting,
}

/// 合并笔
///
/// 在特征序列相邻笔出现包含关系时，合并为一笔
/// 此时笔并不具有方向性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CStroke {
    pub high_pt: Parting,
    pub low_pt: Parting,
}

/// 线段
///
/// 缠论的基础概念
/// 由至少3笔构成，但并不是任意3笔都可构成线段。
/// 一条线段的结束是由特征序列进行判断。
/// 在一条向上线段中，所有向下的笔构成该线段的特征序列。
/// 在一条向下线段中，所有向上的笔构成该线段的特征序列。
/// 将特征序列中每一笔看作一条K线，则可以根据分型判断逻辑，
/// 区别出顶分型和底分型。
/// 顶分型的顶即向上线段的结束。
/// 底分型的底即向下线段的结束。
/// 当确定线段终点后，该终点后的笔不再归属于该线段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start_pt: Parting,
    pub end_pt: Parting,
}

/// 中枢
///
/// 缠论的基础概念
/// 由至少3个存在重叠区间的次级别走势类型构成。
/// 1分钟K线图中走势类型由线段代替。
/// 即5分钟的中枢由至少3个1分钟级别的线段构成。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Center {
    // 起始时刻
    pub start_ts: NaiveDateTime,
    // 起始价格
    pub start_price: BigDecimal,
    // 结束时刻
    pub end_ts: NaiveDateTime,
    // 结束价格
    pub end_price: BigDecimal,
    // 共享最低点，即所有次级别走势类型的最低点中的最高点
    pub shared_low: BigDecimal,
    // 共享最高点，即所有次级别走势类型的最高点中的最低点
    pub shared_high: BigDecimal,
    // 是否起始于向上走势中
    pub upward: bool,
    // 级别
    // 1 -> 5分钟线中枢
    // 2 -> 30分钟线中枢
    // 3 -> 日线中枢
    // 4 -> 周线中枢
    // 5 -> 月线中枢
    pub level: u8,
}

/// 走势类型
///
/// 缠论的基础概念
/// 分为趋势和盘整
/// 趋势由至少2个没有价格区间重叠的中枢构成，趋势向上则为上涨，趋势向下则为下跌
/// 盘整由1个中枢构成
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Trend {
    // 起始时刻
    pub start_ts: NaiveDateTime,
    // 起始价格
    pub start_price: BigDecimal,
    // 结束时刻
    pub end_ts: NaiveDateTime,
    // 结束价格
    pub end_price: BigDecimal,
    // 中枢列表
    pub centers: Vec<Center>,
    // 级别
    // 1 -> 近似5分钟线走势类型
    // 2 -> 近似30分钟线走势类型
    // 3 -> 近似日线走势类型
    // 4 -> 近似周线走势类型
    // 5 -> 近似月线走势类型
    pub level: u8,
}

/// 单标的确定周期的数据来源
pub trait Source {
    // 给定代码和时刻，获取该时刻前的不多于limit条数的K线数据
    fn data_before(&self, ts: &str, limit: u32) -> Vec<K>;

    // 给定代码和时刻，获取该时刻后的不多余limit条数的K线数据
    fn data_after(&self, ts: &str, limit: u32) -> Vec<K>;
}
