//! Morphology of tanglism
//! 
//! this module defines several important concepts in morphology of tanglism
//! Parting
//! Line
//! Segment
//! Pivot
//! Trend

use crate::{TTimestamp, TPrice, TQuantity};
use serde::{Serialize, Deserialize};

/// Parting is composed of three k lines.
/// 
/// Suppose three lines are k1, k2, k3.
/// And their top prices and bottom prices are named t1, b1, t2, b2, t3, b3
/// 
/// The definition of TopParting is: t1 < t2 && t3 < t2 && b1 < b2 && b3 < b2.
/// Then we call the three k lines compose a TopParting, abbr top.
/// 
/// The defnition of BottomParting is: t1 > t2 && t3 > t2 && b1 > b2 && b3 > b2.
/// Then we call the three k lines compose a BottomParting, abbr bottom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parting {
    pub top_price: TPrice,
    pub bottom_price: TPrice,
    pub quantity: TQuantity,
    pub ts: TTimestamp,
    pub kind: PartingKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartingKind {
    Top, 
    Bottom,
}

/// model of line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end_ts: TTimestamp,
    pub end_price: TPrice,
    pub quantity: TQuantity,
}

/// model of segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end_ts: TTimestamp,
    pub end_price: TPrice,
    pub quantity: TQuantity,
}

// model of pivot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pivot {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub second_ts: TTimestamp,
    pub second_price: TPrice,
    pub third_ts: TTimestamp,
    pub third_price: TPrice,
    pub end_ts: TTimestamp,
    pub end_price: TPrice,
    pub quantity: TQuantity,
}

// model of trend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trend {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end: Option<TrendEnd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendEnd {
    pub ts: TTimestamp,
    pub price: TPrice,
}