use chrono::prelude::*;
use crate::{Result, Error};
use crate::trading_date::{TradingDates, TradingDateBitmap, LOCAL_TRADING_DATE_BITMAP};

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

}

lazy_static! {
    static ref MORNING_START: NaiveTime = NaiveTime::from_hms(9, 30, 0);
    static ref MORNING_END: NaiveTime = NaiveTime::from_hms(11, 30, 0);
    static ref AFTERNOON_START: NaiveTime = NaiveTime::from_hms(13, 0, 0);
    static ref AFTERNOON_END: NaiveTime = NaiveTime::from_hms(15, 0, 0);
}

/// 中国交易时刻集合
/// 
/// 早晨9:30 - 11:30
/// 下午13:00 - 15:00
/// 
/// 初始化1分钟，5分钟，和30分钟的交易时刻集合
/// LOCAL_TRADING_TS_1_MIN
/// LOCAL_TRADING_TS_5_MIN
/// LOCAL_TRADING_TS_30_MIN
#[derive(Debug, Clone)]
pub struct LocalTradingTimestamps {
    tick: String,
    tick_minutes: i32,
    tdbm: TradingDateBitmap,
}

lazy_static! {
    pub static ref LOCAL_TRADING_TS_1_MIN: LocalTradingTimestamps = 
        LocalTradingTimestamps::new("1m", LOCAL_TRADING_DATE_BITMAP.clone()).unwrap();
    pub static ref LOCAL_TRADING_TS_5_MIN: LocalTradingTimestamps = 
        LocalTradingTimestamps::new("5m", LOCAL_TRADING_DATE_BITMAP.clone()).unwrap();
    pub static ref LOCAL_TRADING_TS_30_MIN: LocalTradingTimestamps = 
        LocalTradingTimestamps::new("30m", LOCAL_TRADING_DATE_BITMAP.clone()).unwrap();
}

impl LocalTradingTimestamps {

    pub fn new(tick: &str, tdbm: TradingDateBitmap) -> Result<Self> {
        let tick_minutes = match tick {
            "1m" => 1,
            "5m" => 5,
            "30m" => 30,
            _ => return Err(Error(format!("tick {} not supported", tick))),
        };
        Ok(LocalTradingTimestamps{
            tick: tick.to_owned(),
            tick_minutes,
            tdbm,
        })
    }
}

impl TradingTimestamps for LocalTradingTimestamps {

    fn tick(&self) -> String {
        self.tick.clone()
    }

    fn tick_minutes(&self) -> i32 {
        self.tick_minutes
    }

    fn next_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        if ts.minute() % self.tick_minutes() as u32 != 0 {
            return None;
        }
        if ts.time() < *MORNING_START || ts.time() > *AFTERNOON_END || (ts.time() > *MORNING_END && ts.time() < *AFTERNOON_START) {
            return None;
        }
        // 如果ts被选择在了上午和下午开始时刻，对取下一个tick并无影响，不需要额外处理
        if ts.time() == *MORNING_END {
            let start_ts = NaiveDateTime::new(ts.date(), *AFTERNOON_START);
            let result = start_ts + chrono::Duration::minutes(self.tick_minutes() as i64);
            return Some(result);
        }
        if ts.time() == *AFTERNOON_END {
            if let Some(start_dt) = self.next_day(ts.date()) {
                let start_ts = NaiveDateTime::new(start_dt, *MORNING_START);
                let result = start_ts + chrono::Duration::minutes(self.tick_minutes() as i64);
                return Some(result);
            }
            return None;
        }
        let result = ts + chrono::Duration::minutes(self.tick_minutes() as i64);
        Some(result)
    }

    fn prev_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        if ts.minute() % self.tick_minutes() as u32 != 0 {
            return None;
        }
        if ts.time() < *MORNING_START || ts.time() > *AFTERNOON_END || (ts.time() > *MORNING_END && ts.time() < *AFTERNOON_START) {
            return None;
        }
        // 如果ts被选择在了上午和下午开始时刻，修正为前一tick的结束时刻
        let ts = if ts.time() == *MORNING_START {
            if let Some(prev_dt) = self.prev_day(ts.date()) {
                NaiveDateTime::new(prev_dt, *AFTERNOON_END)
            } else {
                return None;
            }
        } else if ts.time() == *AFTERNOON_START {
            NaiveDateTime::new(ts.date(), *MORNING_END)
        } else {
            ts
        };
        let prev_ts = ts - chrono::Duration::minutes(self.tick_minutes() as i64);
        if prev_ts.time() == *MORNING_START {
            if let Some(prev_dt) = self.prev_day(prev_ts.date()) {
                return Some(NaiveDateTime::new(prev_dt, *AFTERNOON_END));
            }
            return None;
        }
        if prev_ts.time() == *AFTERNOON_START {
            return Some(NaiveDateTime::new(prev_ts.date(), *MORNING_END));
        }
        Some(prev_ts)
    }

}

/// 代理TradingDates方法
impl TradingDates for LocalTradingTimestamps {

    fn first_day(&self) -> Option<NaiveDate> {
        self.tdbm.first_day()
    }

    fn last_day(&self) -> Option<NaiveDate> {
        self.tdbm.last_day()
    }

    fn next_day(&self, day: NaiveDate) -> Option<NaiveDate> {
        self.tdbm.next_day(day)
    }

    fn prev_day(&self, day: NaiveDate) -> Option<NaiveDate> {
        self.tdbm.prev_day(day)
    }

    fn contains_day(&self, day: NaiveDate) -> bool {
        self.tdbm.contains_day(day)
    }

    fn all_days(&self) -> Vec<NaiveDate> {
        self.tdbm.all_days()
    }

    fn add_day(&mut self, day: NaiveDate) -> Result<()> {
        self.tdbm.add_day(day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_trading_ts_tick_and_minutes() -> Result<()> {
        let ltts1 = LocalTradingTimestamps::new("1m", TradingDateBitmap::empty())?;
        assert_eq!("1m".to_owned(), ltts1.tick());
        assert_eq!(1, ltts1.tick_minutes());
        let ltts2 = LocalTradingTimestamps::new("5m", TradingDateBitmap::empty())?;
        assert_eq!("5m".to_owned(), ltts2.tick());
        assert_eq!(5, ltts2.tick_minutes());
        let ltts3 = LocalTradingTimestamps::new("30m", TradingDateBitmap::empty())?;
        assert_eq!("30m".to_owned(), ltts3.tick());
        assert_eq!(30, ltts3.tick_minutes());
        Ok(())
    }

    #[test]
    fn test_trading_ts_prev_and_next_tick() -> Result<()> {
        let mut tdbm = TradingDateBitmap::empty();
        tdbm.add_day_str("2020-02-01");
        tdbm.add_day_str("2020-02-02");
        let ltts = LocalTradingTimestamps::new("30m", tdbm)?;
        let ts_02010800 = NaiveDateTime::from_str("2020-02-01T08:00:00")?;
        let ts_02010930 = NaiveDateTime::from_str("2020-02-01T09:30:00")?;
        let ts_02011000 = NaiveDateTime::from_str("2020-02-01T10:00:00")?;
        let ts_02011030 = NaiveDateTime::from_str("2020-02-01T10:30:00")?;
        let ts_02011100 = NaiveDateTime::from_str("2020-02-01T11:00:00")?;
        let ts_02011130 = NaiveDateTime::from_str("2020-02-01T11:30:00")?;
        let ts_02011300 = NaiveDateTime::from_str("2020-02-01T13:00:00")?;
        let ts_02011330 = NaiveDateTime::from_str("2020-02-01T13:30:00")?;
        let ts_02011400 = NaiveDateTime::from_str("2020-02-01T14:00:00")?;
        let ts_02011430 = NaiveDateTime::from_str("2020-02-01T14:30:00")?;
        let ts_02011500 = NaiveDateTime::from_str("2020-02-01T15:00:00")?;
        let ts_02020930 = NaiveDateTime::from_str("2020-02-02T09:30:00")?;
        let ts_02021000 = NaiveDateTime::from_str("2020-02-02T10:00:00")?;
        
        assert_eq!(None, ltts.prev_tick(ts_02010800));
        assert_eq!(None, ltts.next_tick(ts_02010800));
        
        assert_eq!(None, ltts.prev_tick(ts_02010930));
        assert_eq!(Some(ts_02011000), ltts.next_tick(ts_02010930));
        
        assert_eq!(None, ltts.prev_tick(ts_02011000));
        assert_eq!(Some(ts_02011030), ltts.next_tick(ts_02011000));
        
        assert_eq!(Some(ts_02011130), ltts.next_tick(ts_02011100));

        assert_eq!(Some(ts_02011100), ltts.prev_tick(ts_02011130));
        assert_eq!(Some(ts_02011330), ltts.next_tick(ts_02011130));

        assert_eq!(Some(ts_02011330), ltts.next_tick(ts_02011300));

        assert_eq!(Some(ts_02011130), ltts.prev_tick(ts_02011330));
        assert_eq!(Some(ts_02011400), ltts.next_tick(ts_02011330));
        
        assert_eq!(Some(ts_02011430), ltts.prev_tick(ts_02011500));
        assert_eq!(Some(ts_02021000), ltts.next_tick(ts_02011500));

        assert_eq!(Some(ts_02011430), ltts.prev_tick(ts_02020930));
        assert_eq!(Some(ts_02021000), ltts.next_tick(ts_02020930));

        assert_eq!(Some(ts_02011500), ltts.prev_tick(ts_02021000));

        Ok(())
    }
}