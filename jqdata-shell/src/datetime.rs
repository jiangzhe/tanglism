use crate::{Error, Result};
use chrono::{Local, NaiveDateTime, NaiveDate};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M";
const DATE_FORMAT: &str = "%Y-%m-%d";

/// 解析并返回时间戳（以及是否为天）
pub fn parse_ts_from_str(s: &str) -> Result<(NaiveDateTime, bool)> {
    match s.len() {
        10 => {
            let dt = NaiveDateTime::parse_from_str(&format!("{} 00:00", s), DATETIME_FORMAT)?;
            Ok((dt, true))
        }
        16 => {
            let dt = NaiveDateTime::parse_from_str(s, DATETIME_FORMAT)?;
            Ok((dt, false))
        }
        _ => Err(Error(format!("invalid datetime format: {}", s))),
    }
}

/// 解析并返回日期
pub fn parse_date_from_str(s: &str) -> Result<NaiveDate> {
    let dt = NaiveDate::parse_from_str(s, DATE_FORMAT)?;
    Ok(dt)
}


pub struct DatetimeUtil {
    pub unit: String,
    minutes: i64,
    day: bool,
}

/// 日期处理工具
#[allow(dead_code)]
impl DatetimeUtil {
    pub fn new(unit: &str) -> Result<Self> {
        let minutes = match unit {
            "1m" => 1,
            "5m" => 5,
            "30m" => 30,
            "1d" => 60 * 24,
            _ => return Err(Error(format!("unit {} not supported", unit))),
        };
        Ok(DatetimeUtil {
            unit: unit.to_owned(),
            minutes,
            day: unit == "1d",
        })
    }

    /// 需考虑开盘和收盘及午休的间隔
    /// 需考虑交易日与非交易日产生的间隔
    /// 输入日期应符合24小时制，且该时刻必定满足与开盘和收盘时间的整数单位间隔
    pub fn next(&self, ts: &str) -> Result<String> {
        let (curr_dt, day) = parse_ts_from_str(ts)?;
        if !day {
            let duration;
            if ts.ends_with(" 11:30:00") || ts.ends_with(" 11:30") {
                // 午休90分钟
                duration = self.minutes + 90;
            } else if ts.ends_with(" 15:00:00") || ts.ends_with(" 15:00") {
                // todo
            }
            
        }

        let next_dt = curr_dt
            .checked_add_signed(chrono::Duration::minutes(self.minutes))
            .unwrap();
        Ok(next_dt.format(self.fmt_str()).to_string())
    }

    pub fn prev(&self, ts: &str) -> Result<String> {
        let (curr_dt, day) = parse_ts_from_str(ts)?;
        let prev_dt = curr_dt
            .checked_sub_signed(chrono::Duration::minutes(self.minutes))
            .unwrap();
        Ok(prev_dt.format(self.fmt_str()).to_string())
    }

    fn fmt_str(&self) -> &'static str {
        if self.day {
            DATE_FORMAT
        } else {
            DATETIME_FORMAT
        }
    }
}

pub fn end_of_today() -> String {
    end_of_day(Local::today().naive_local())
}

pub fn end_of_day(day: NaiveDate) -> String {
    format!(
        "{} 23:59",
        day.format(DATETIME_FORMAT)
    )
}

pub fn start_of_today() -> String {
    start_of_day(Local::today().naive_local())
}

pub fn start_of_day(day: NaiveDate) -> String {
    format!(
        "{} 00:00",
        day.format(DATETIME_FORMAT)
    )
}

pub(crate) struct DatetimeRange {
    min: NaiveDateTime,
    max: NaiveDateTime,
}

#[allow(dead_code)]
impl DatetimeRange {
    pub(crate) fn new(min: &str, max: &str) -> Result<Self> {
        let (min, _) = parse_ts_from_str(min)?;
        let (max, _) = parse_ts_from_str(max)?;
        if min > max {
            return Err(Error(format!(
                "invalid datetime range: min={}, max={}",
                min, max
            )));
        }
        Ok(DatetimeRange { min, max })
    }

    pub(crate) fn include(&self, dt: &str) -> Result<bool> {
        let (dt, _) = parse_ts_from_str(dt)?;
        Ok(self.min <= dt && dt <= self.max)
    }

    pub(crate) fn min_after(&self, dt: &str) -> Result<bool> {
        let (dt, _) = parse_ts_from_str(dt)?;
        Ok(self.min > dt)
    }

    pub(crate) fn max_after(&self, dt: &str) -> Result<bool> {
        let (dt, _) = parse_ts_from_str(dt)?;
        Ok(self.max > dt)
    }

    pub(crate) fn min_before(&self, dt: &str) -> Result<bool> {
        let (dt, _) = parse_ts_from_str(dt)?;
        Ok(self.min < dt)
    }

    pub(crate) fn max_before(&self, dt: &str) -> Result<bool> {
        let (dt, _) = parse_ts_from_str(dt)?;
        Ok(self.max < dt)
    }

    pub(crate) fn min(&self) -> String {
        self.min.format(DATETIME_FORMAT).to_string()
    }

    pub(crate) fn max(&self) -> String {
        self.max.format(DATETIME_FORMAT).to_string()
    }
}

