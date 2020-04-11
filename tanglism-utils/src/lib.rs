mod error;
pub mod trading_date;
pub mod trading_timestamp;

#[macro_use]
extern crate lazy_static;

// pub use datetime::*;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use trading_date::{TradingDates, LOCAL_DATES};
pub use trading_timestamp::{
    TradingTimestamps, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN,
};

use chrono::{NaiveDate, NaiveDateTime};

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
