mod error;
pub mod trading_timestamp;

#[macro_use]
extern crate lazy_static;

// pub use datetime::*;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use trading_timestamp::*;

use chrono::{NaiveDate, NaiveDateTime};

/// 交易日集合
///
/// 用于获取交易日，计算前一个或后一个交易日，计算某天是否为交易日等
pub trait TradingDates {
    // 集合中的第一个交易日
    fn first_day(&self) -> Option<NaiveDate>;

    // 集合中的最后一个交易日
    fn last_day(&self) -> Option<NaiveDate>;

    // 指定日期的下一个交易日，不包含
    fn next_day(&self, day: NaiveDate) -> Option<NaiveDate>;

    // 指定日期的前一个交易日，不包含
    fn prev_day(&self, day: NaiveDate) -> Option<NaiveDate>;

    // 指定日期是否为交易日
    fn contains_day(&self, day: NaiveDate) -> bool;

    // 获取集合内的所有交易日
    fn all_days(&self) -> Vec<NaiveDate>;

    // 向集合内添加指定交易日
    fn add_day(&mut self, day: NaiveDate) -> Result<()>;
}

/// 交易时刻集合
///
/// 提供日内交易时刻的相关操作，包括查询前后时刻
/// 日级别操作参考TradingDates
pub trait TradingTimestamps {
    /// 返回时刻集合的基础单位，支持1m, 5m, 30m, 1d
    fn tick(&self) -> String;

    /// 返回时刻集合的基础单位的分钟数
    fn tick_minutes(&self) -> i32;

    /// 后一个交易时刻
    ///
    /// 给定的时刻必须符合tick规则，例如当tick=5m时，ts分钟数必须为5的整数倍
    fn next_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime>;

    /// 前一个交易时刻
    ///
    /// 给定的时刻必须符合tick规则，例如当tick=5m时，ts分钟数必须为5的整数倍
    fn prev_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime>;

    /// 给定任意时刻，对其到tick对应的交易时刻
    ///
    /// 如果该时刻可交易，将对齐到所在tick的结束时刻
    /// 例如，tick="5m", ts="2020-02-17 09:34:00", 将返回"2020-02-17 09-35:00"
    fn aligned_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime>;
}

/// 当天起始时刻
pub fn start_of_day_str(dt: NaiveDate) -> String {
    let mut s = dt.format("%Y-%m-%d").to_string();
    s.push_str(" 00:00:00");
    s
}

/// 当天结束时刻
pub fn end_of_day_str(dt: NaiveDate) -> String {
    let mut s = dt.format("%Y-%m-%d").to_string();
    s.push_str(" 23:59:59");
    s
}

/// 解析并返回时间戳（以及是否为天）
pub fn parse_ts_from_str(s: &str) -> Result<(NaiveDateTime, bool)> {
    match s.len() {
        10 => {
            let dt = NaiveDateTime::parse_from_str(&format!("{} 00:00", s), "%Y-%m-%d %H:%M")?;
            Ok((dt, true))
        }
        13 => {
            let dt = NaiveDateTime::parse_from_str(&format!("{}:00", s), "%Y-%m-%d %H:%M")?;
            Ok((dt, false))
        }
        16 => {
            let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")?;
            Ok((dt, false))
        }
        19 => {
            let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")?;
            Ok((dt, false))
        }
        _ => Err(Error(format!("invalid datetime format: {}", s))),
    }
}

/// 解析并返回日期
pub fn parse_date_from_str(s: &str) -> Result<NaiveDate> {
    let dt = NaiveDate::parse_from_str(s, "%Y-%m-%d")?;
    Ok(dt)
}
