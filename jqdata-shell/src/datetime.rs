use crate::{Result, Error};
use chrono::{NaiveDateTime, Local};

pub(crate) struct DatetimeProcessor {
    pub unit: String,
    minutes: i64,
    day: bool,
}

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M";
const DATE_FORMAT: &str = "%Y-%m-%d";

#[allow(dead_code)]
impl DatetimeProcessor {
    pub(crate) fn new(unit: &str) -> Result<Self> {
        let minutes = match unit {
            "1m" => 1,
            "5m" => 5,
            "30m" => 30,
            "1d" => 60 * 24,
            _ => return Err(Error(format!("unit {} not supported", unit))),
        };
        Ok(DatetimeProcessor{
            unit: unit.to_owned(),
            minutes,
            day: unit == "1d",
        })
    }

    pub(crate) fn next(&self, ts: &str) -> Result<String> {
        let curr_dt = parse_from_str(ts)?;
        let next_dt = curr_dt.checked_add_signed(chrono::Duration::minutes(self.minutes)).unwrap();
        Ok(next_dt.format(self.fmt_str()).to_string())        
    }

    pub(crate) fn prev(&self, ts: &str) -> Result<String> {
        let curr_dt = parse_from_str(ts)?;
        let prev_dt = curr_dt.checked_sub_signed(chrono::Duration::minutes(self.minutes)).unwrap();
        Ok(prev_dt.format(self.fmt_str()).to_string())        
    }

    pub(crate) fn end_of_today(&self) -> String {
        format!("{} 23:59", Local::today().naive_local().format(DATETIME_FORMAT))
    }

    pub(crate) fn start_of_today(&self) -> String {
        format!("{} 00:00", Local::today().naive_local().format(DATETIME_FORMAT))
    }

    fn fmt_str(&self) -> &'static str {
        if self.day {
            DATE_FORMAT
        } else {
            DATETIME_FORMAT
        }
    }
}

pub(crate) struct DatetimeRange {
    min: NaiveDateTime,
    max: NaiveDateTime,
}

#[allow(dead_code)]
impl DatetimeRange {
    pub(crate) fn new(min: &str, max: &str) -> Result<Self> {
        let min = parse_from_str(min)?;
        let max = parse_from_str(max)?;
        if min > max {
            return Err(Error(format!("invalid datetime range: min={}, max={}", min, max)));
        }
        Ok(DatetimeRange{ min, max })
    }

    pub(crate) fn include(&self, dt: &str) -> Result<bool> {
        let dt = parse_from_str(dt)?;
        Ok(self.min <= dt && dt <= self.max)
    }

    pub(crate) fn min_after(&self, dt: &str) -> Result<bool> {
        let dt = parse_from_str(dt)?;
        Ok(self.min > dt)
    }

    pub (crate) fn max_after(&self, dt: &str) -> Result<bool> {
        let dt = parse_from_str(dt)?;
        Ok(self.max > dt)
    }

    pub (crate) fn min_before(&self, dt: &str) -> Result<bool> {
        let dt = parse_from_str(dt)?;
        Ok(self.min < dt)
    }

    pub(crate) fn max_before(&self, dt: &str) -> Result<bool> {
        let dt = parse_from_str(dt)?;
        Ok(self.max < dt)
    }

    pub(crate) fn min(&self) -> String {
        self.min.format(DATETIME_FORMAT).to_string()
    }

    pub(crate) fn max(&self) -> String {
        self.max.format(DATETIME_FORMAT).to_string()
    }
}


fn parse_from_str(s: &str) -> Result<NaiveDateTime> {
    match s.len() {
        10 => {
            let dt = NaiveDateTime::parse_from_str(&format!("{} 00:00", s), DATETIME_FORMAT)?;
            Ok(dt)
        },
        16 => {
            let dt = NaiveDateTime::parse_from_str(s, DATETIME_FORMAT)?;
            Ok(dt)
        },
        _ => Err(Error(format!("invalid datetime format: {}", s)))
    }
}