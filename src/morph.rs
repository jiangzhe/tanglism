//! morphology of tanglism
//! this module defines several important concepts in morphology of tanglism
//! Parting
//! Line
//! Segment
//! Pivot
//! Trend

use crate::{TTimestamp, TPrice, TQuantity};

/// model of parting
pub enum Parting {
    TopParting {
        top_price: TPrice,
        bottom_price: TPrice,
        quantity: TQuantity,
        ts: TTimestamp,
    },
    BottomParting {
        top_price: TPrice,
        bottom_price: TPrice,
        quantity: TQuantity,
        ts: TTimestamp,
    },
}

/// model of line
pub struct Line {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end_ts: TTimestamp,
    pub end_price: TPrice,
    pub quantity: TQuantity,
}

/// model of segment
pub struct Segment {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end_ts: TTimestamp,
    pub end_price: TPrice,
    pub quantity: TQuantity,
}

// model of pivot
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
pub struct Trend {
    pub start_ts: TTimestamp,
    pub start_price: TPrice,
    pub end: Option<TrendEnd>,
}

pub struct TrendEnd {
    pub ts: TTimestamp,
    pub price: TPrice,
}