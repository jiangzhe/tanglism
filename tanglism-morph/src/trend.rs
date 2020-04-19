//! 走势类型
//!
//! 缠论的基础概念
//!
//! 分为趋势和盘整
//! 趋势由至少2个没有价格区间重叠的中枢构成，趋势向上则为上涨，趋势向下则为下跌
//! 盘整由1个中枢构成
//!
//! 走势分解定理一：任何级别的任何走势，都可以分解成同级别盘整、下跌与上涨三种走势类型的连接。
//! 走势分解定理二：任何级别的任何走势类型，都至少由三段以上次级别走势类型构成。
//!
//! 目前的实现是直接使用次级别段作为次级别走势，而次级别笔作为次级别以下走势。

use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_derive::*;

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
    // 最低点
    pub low: BigDecimal,
    // 最高点
    pub high: BigDecimal,
    // 中枢扩展
    pub extension: Option<Extension>,
    // 级别
    // 1 -> 5分钟线中枢
    // 2 -> 30分钟线中枢
    // 3 -> 日线中枢
    // 4 -> 周线中枢
    // 5 -> 月线中枢
    pub level: u8,
}

/// 中枢扩展
///
/// 先不考虑扩展
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Extension {}

/// 次级别走势
///
/// 当前实现使用次级别K线图中的线段和笔（次级别以下）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum SubTrend {
    /// 次级别段
    ///
    /// 将从次级别K线图中分析得出的线段作为次级别走势的一个实现
    Segment {
        start_ts: NaiveDateTime,
        start_price: BigDecimal,
        end_ts: NaiveDateTime,
        end_price: BigDecimal,
    },
    /// 次级别笔
    ///
    /// 将从次级别K线图中的笔作为次级别以下走势的一个实现
    Stroke {
        start_ts: NaiveDateTime,
        start_price: BigDecimal,
        end_ts: NaiveDateTime,
        end_price: BigDecimal,
    },
}
