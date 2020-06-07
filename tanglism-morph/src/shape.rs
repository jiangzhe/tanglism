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
    // 价格区间，用于进行缺口判断
    pub price_range: Option<Box<PriceRange>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRange {
    // 起始最高价
    pub start_high: BigDecimal,
    // 起始最低价
    pub start_low: BigDecimal,
    // 结束最高价
    pub end_high: BigDecimal,
    // 结束最高价
    pub end_low: BigDecimal,
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
    // 左侧缺口
    pub left_gap: Option<Box<Gap>>,
    // 右侧缺口
    pub right_gap: Option<Box<Gap>>,
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

/// 缺口
///
/// 缠论的基础概念
/// 在该单位K线图上两相邻的K线间出现没有成交的区间（77课）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gap {
    pub ts: NaiveDateTime,
    pub start_price: BigDecimal,
    pub end_price: BigDecimal,
}

/// 中枢元素
/// 
/// 中枢与分型，笔，线段有很大的不同。
/// 在一张连续的K线图中，中枢通常是离散分布。
/// 中枢与中枢之间通过次级别（或以下）走势相连。
/// 因此中枢分析产生的序列既包含中枢，也包含次级别走势乃至缺口
/// 这里统一使用CenterElement枚举进行建模
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum CenterElement {
    Center(Center),
    SubTrend(SubTrend),
    // 类中枢
    //
    // 类中枢同样由三段重叠的次级别走势构成
    // 将三段走势分别编号为s1, s2, s3，
    // 则从整体走势上看，中枢s1, s3都是逆向走势
    // 而类中枢的s2是逆向走势。
    // 即类中枢与中枢的起始方向是相反的。
    SemiCenter(SemiCenter),
}

/// 中枢
///
/// 缠论的基础概念
/// 由至少3个存在重叠区间的次级别走势类型构成。
/// 1分钟K线图中走势类型由线段代替。
/// 1分钟K线图的笔即可视为1分钟“中枢”，极端如20课所说，
/// 连续多天开盘封涨停仍只形成1分钟中枢。
/// 5分钟的中枢由至少3个1分钟级别的线段构成。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Center {
    // 起始点
    pub start: ValuePoint,
    // 结束点
    pub end: ValuePoint,
    // 共享最低点，即所有次级别走势类型的最低点中的最高点
    pub shared_low: ValuePoint,
    // 共享最高点，即所有次级别走势类型的最高点中的最低点
    pub shared_high: ValuePoint,
    // 最低点
    pub low: ValuePoint,
    // 最高点
    pub high: ValuePoint,
    // 中枢级别
    pub level: i32,
    // 方向，由第一个走势确定
    // 一般的，在趋势中的中枢方向总与趋势相反
    // 即上升时，中枢总是由下上下三段次级别走势构成
    // 盘整时，相邻连个中枢方向不一定一致
    pub upward: bool,
    // 组成该中枢的次级别走势个数
    pub n: usize,
}

impl Center {
    pub fn contains_price(&self, price: &BigDecimal) -> bool {
        price >= &self.shared_low.value && price <= &self.shared_high.value
    }

    // 两个价格分别位于中枢区间两侧
    pub fn split_prices(&self, price1: &BigDecimal, price2: &BigDecimal) -> bool {
        (price1 < &self.shared_low.value && price2 > &self.shared_high.value) || (price1 > &self.shared_high.value && price2 < &self.shared_low.value)
    }

    // 是否是类中枢形态。类中枢的最高最低点分别为起始和结束点。
    pub fn semi(&self) -> bool {
        (self.start.value < self.shared_low.value && self.end.value > self.shared_high.value) || (self.start.value > self.shared_high.value && self.end.value < self.shared_low.value)
    }
}

/// 类中枢
/// 
/// 这里的类中枢与缠论略有不同。
/// 连接两个中枢间的多段走势未构成标准中枢，则归为类中枢。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SemiCenter {
    pub start: ValuePoint,
    pub end: ValuePoint,
    pub level: i32,
    pub upward: bool,
    pub n: usize,
    // 是否与前一个中枢共享起始次级别走势
    pub shared_start: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValuePoint {
    pub ts: NaiveDateTime,
    pub value: BigDecimal,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SubTrendType {
    Normal,
    // 由缺口形成的次级别
    Gap,
    // 前后两段被笔破坏
    Divider,
    // 由多条线段组合而成
    Combination,
}

/// 次级别走势
///
/// 当前实现使用次级别K线图中的线段和笔（次级别以下）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubTrend {
    pub start: ValuePoint,
    pub end: ValuePoint,
    pub level: i32,
    pub typ: SubTrendType,
}

impl SubTrend {
    pub fn sorted(&self) -> (&BigDecimal, &BigDecimal) {
        if self.start.value < self.end.value {
            (&self.start.value, &self.end.value)
        } else {
            (&self.end.value, &self.start.value)
        }
    }

    pub fn sorted_points(&self) -> (ValuePoint, ValuePoint) {
        if self.start.value < self.end.value {
            (self.start.clone(), self.end.clone())
        } else {
            (self.end.clone(), self.start.clone())
        }
    }
}
