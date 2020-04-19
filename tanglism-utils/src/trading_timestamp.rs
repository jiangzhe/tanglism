use crate::{Error, Result};
use crate::{TradingDates, TradingTimestamps};
use chrono::prelude::*;
use std::sync::Arc;

// 对交易日的范围进行全局限制
// 时间范围为：2010-01-01 ~ 2099-12-31
// 对时刻进行限制：上午9:30 ~ 11:30，下午13:00 ~ 15:00
lazy_static! {
    static ref FIRST_DAY: NaiveDate = NaiveDate::from_ymd(2010, 1, 1);
    static ref LAST_DAY: NaiveDate = NaiveDate::from_ymd(2099, 12, 31);
    static ref MORNING_START: NaiveTime = NaiveTime::from_hms(9, 30, 0);
    static ref MORNING_END: NaiveTime = NaiveTime::from_hms(11, 30, 0);
    static ref AFTERNOON_START: NaiveTime = NaiveTime::from_hms(13, 0, 0);
    static ref AFTERNOON_END: NaiveTime = NaiveTime::from_hms(15, 0, 0);
}

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

/// 判断是否是允许交易的时刻
fn permit_trade_time(tm: NaiveTime) -> bool {
    (tm >= *MORNING_START && tm <= *MORNING_END) || (tm >= *AFTERNOON_START && tm <= *AFTERNOON_END)
}

// 将索引转化为日期
// 结果必须在交易日范围内
fn idx_to_day(idx: i64) -> Option<NaiveDate> {
    let day = idx_to_day_unchecked(idx);
    if LAST_DAY.signed_duration_since(day).num_days() < 0 {
        return None;
    }
    if day.signed_duration_since(*FIRST_DAY).num_days() < 0 {
        return None;
    }
    Some(day)
}

fn idx_to_day_unchecked(idx: i64) -> NaiveDate {
    *FIRST_DAY + chrono::Duration::days(idx)
}

// 将日期转化为索引
// 结果必须在交易日范围内
fn day_to_idx(day: NaiveDate) -> Option<i64> {
    if LAST_DAY.signed_duration_since(day).num_days() < 0 {
        return None;
    }
    if day.signed_duration_since(*FIRST_DAY).num_days() < 0 {
        return None;
    }
    let idx = day_to_idx_unchecked(day);
    debug_assert!(idx >= 0);
    Some(idx)
}

fn day_to_idx_unchecked(day: NaiveDate) -> i64 {
    day.signed_duration_since(*FIRST_DAY).num_days()
}

// 位图比特数
const BITS: usize = 64;
const BITS_ONE: u64 = 1u64;
type Bits = u64;

// 交易日集合的位图实现
#[derive(Debug, Clone)]
pub struct LocalTradingDates {
    bm: Vec<Bits>,
}

impl LocalTradingDates {
    // 创建空实例
    pub fn empty() -> Self {
        LocalTradingDates { bm: Vec::new() }
    }

    // 通过字符串添加日期，若字符串无效则直接丢弃
    pub fn add_day_str(&mut self, day_str: &str) {
        if let Ok(td) = parse_date_from_str(day_str) {
            if let Some(idx) = day_to_idx(td) {
                self.add_day_idx(idx as usize);
            }
        }
    }

    fn ensure_capacity(&mut self, capacity: usize) {
        let buckets = capacity / BITS + 1;
        if self.bm.len() < buckets {
            for _i in self.bm.len()..buckets {
                self.bm.push(0);
            }
        }
    }

    fn add_day_idx(&mut self, idx: usize) {
        self.ensure_capacity(idx);
        let bucket_id = idx / BITS;
        let bit_pos = idx % BITS;
        // println!("{}, {}, {}", self.bm.len(), bucket_id, idx);
        self.bm[bucket_id] |= BITS_ONE << bit_pos;
    }

    fn contains_day_idx(&self, idx: usize) -> bool {
        let bucket_id = idx / BITS;
        let bit_pos = idx % BITS;
        bucket_id < self.bm.len() && self.bm[bucket_id] & (BITS_ONE << bit_pos) != 0
    }

    fn prev_day_idx_inclusive(&self, idx: i64) -> Option<i64> {
        let mut bucket_id = idx / BITS as i64;
        let mut bit_pos = idx % BITS as i64;
        while bucket_id >= 0 {
            let bucket = self.bm[bucket_id as usize];
            // 优化，仅检查非空bucket
            if bucket > 0 {
                while bit_pos >= 0 {
                    if bucket & (BITS_ONE << bit_pos) != 0 {
                        return Some(bucket_id * BITS as i64 + bit_pos);
                    }
                    bit_pos -= 1;
                }
            }
            // 跳至下一个bucket
            bucket_id -= 1;
            bit_pos = 63;
        }
        None
    }

    fn next_day_idx_inclusive(&self, idx: i64) -> Option<i64> {
        debug_assert!(idx >= 0);
        debug_assert!((idx / BITS as i64) < self.bm.len() as i64);
        let mut idx_iter = IndexIter {
            inner: &self,
            bucket_id: idx / BITS as i64,
            bit_pos: idx % BITS as i64,
        };
        idx_iter.next()
    }

    fn dates(&self) -> usize {
        self.bm.len() * BITS
    }

    fn all_indices(&self) -> IndexIter {
        IndexIter {
            inner: &self,
            bucket_id: 0,
            bit_pos: 0,
        }
    }
}

struct IndexIter<'a> {
    inner: &'a LocalTradingDates,
    bucket_id: i64,
    bit_pos: i64,
}

impl<'a> Iterator for IndexIter<'a> {
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.inner.bm.len() as i64;
        while self.bucket_id < len {
            let bucket = self.inner.bm[self.bucket_id as usize];
            // 优化，仅检查非空bucket
            if bucket > 0 {
                while self.bit_pos < 64 {
                    if bucket & (BITS_ONE << self.bit_pos) != 0 {
                        let idx = self.bucket_id * BITS as i64 + self.bit_pos;
                        self.bit_pos += 1;
                        return Some(idx);
                    } else {
                        self.bit_pos += 1;
                    }
                }
            }
            // 跳至下一个bucket
            self.bucket_id += 1;
            self.bit_pos = 0;
        }
        None
    }
}

impl TradingDates for LocalTradingDates {
    fn first_day(&self) -> Option<NaiveDate> {
        if self.bm.is_empty() {
            return None;
        }
        if let Some(next_idx) = self.next_day_idx_inclusive(0) {
            return idx_to_day(next_idx);
        }
        None
    }

    fn last_day(&self) -> Option<NaiveDate> {
        if self.bm.is_empty() {
            return None;
        }
        let idx = self.dates() - 1;
        if let Some(prev_idx) = self.prev_day_idx_inclusive(idx as i64) {
            return idx_to_day(prev_idx);
        }
        None
    }

    fn prev_day(&self, day: NaiveDate) -> Option<NaiveDate> {
        if self.bm.is_empty() {
            return None;
        }
        if let Some(idx) = day_to_idx(day) {
            if idx == 0 {
                return None;
            }
            let idx = if idx as usize >= self.dates() {
                self.dates() - 1
            } else {
                idx as usize - 1
            };
            if let Some(prev_idx) = self.prev_day_idx_inclusive(idx as i64) {
                let prev_day = idx_to_day_unchecked(prev_idx);
                return Some(prev_day);
            }
        }
        None
    }

    fn next_day(&self, day: NaiveDate) -> Option<NaiveDate> {
        if self.bm.is_empty() {
            return None;
        }
        if let Some(idx) = day_to_idx(day) {
            if idx as usize == self.dates() - 1 {
                return None;
            }
            if let Some(next_idx) = self.next_day_idx_inclusive(idx + 1) {
                let next_day = idx_to_day_unchecked(next_idx);
                return Some(next_day);
            }
        }
        None
    }

    fn all_days(&self) -> Vec<NaiveDate> {
        self.all_indices().filter_map(idx_to_day).collect()
    }

    fn contains_day(&self, day: NaiveDate) -> bool {
        if let Some(idx) = day_to_idx(day) {
            return self.contains_day_idx(idx as usize);
        }
        false
    }

    fn add_day(&mut self, day: NaiveDate) -> Result<()> {
        if let Some(idx) = day_to_idx(day) {
            self.add_day_idx(idx as usize);
            return Ok(());
        }
        Err(Error("day not in range".to_owned()))
    }
}

impl TradingTimestamps for LocalTradingDates {
    fn tick(&self) -> String {
        "1d".to_owned()
    }

    // 名义的分钟数，不使用
    fn tick_minutes(&self) -> i32 {
        24 * 60
    }

    fn prev_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        self.prev_day(ts.date()).map(|t| t.and_hms(15, 0, 0))
    }

    fn next_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        self.next_day(ts.date()).map(|t| t.and_hms(15, 0, 0))
    }

    fn aligned_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        if self.contains_day(ts.date()) && permit_trade_time(ts.time()) {
            return Some(NaiveDateTime::new(ts.date(), *AFTERNOON_END));
        }
        None
    }
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
    // 只读交易日集合，可多线程共享
    tdbm: Arc<LocalTradingDates>,
}

lazy_static! {
    pub static ref LOCAL_DATES: Arc<LocalTradingDates> = {
        let mut tdbm = LocalTradingDates::empty();
        for d in tanglism_data::LOCAL_TRADE_DAYS.iter() {
            tdbm.add_day_str(d);
        }
        Arc::new(tdbm)
    };
    pub static ref LOCAL_TS_1_MIN: LocalTradingTimestamps =
        LocalTradingTimestamps::new("1m", Arc::clone(&LOCAL_DATES)).unwrap();
    pub static ref LOCAL_TS_5_MIN: LocalTradingTimestamps =
        LocalTradingTimestamps::new("5m", Arc::clone(&LOCAL_DATES)).unwrap();
    pub static ref LOCAL_TS_30_MIN: LocalTradingTimestamps =
        LocalTradingTimestamps::new("30m", Arc::clone(&LOCAL_DATES)).unwrap();
}

impl LocalTradingTimestamps {
    pub fn new(tick: &str, tdbm: Arc<LocalTradingDates>) -> Result<Self> {
        let tick_minutes = match tick {
            "1m" => 1,
            "5m" => 5,
            "30m" => 30,
            _ => return Err(Error(format!("tick {} not supported", tick))),
        };
        Ok(LocalTradingTimestamps {
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
        if ts.time() < *MORNING_START
            || ts.time() > *AFTERNOON_END
            || (ts.time() > *MORNING_END && ts.time() < *AFTERNOON_START)
        {
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
        if ts.time() < *MORNING_START
            || ts.time() > *AFTERNOON_END
            || (ts.time() > *MORNING_END && ts.time() < *AFTERNOON_START)
        {
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

    fn aligned_tick(&self, ts: NaiveDateTime) -> Option<NaiveDateTime> {
        if self.contains_day(ts.date()) && permit_trade_time(ts.time()) {
            let rem = ts.minute() as i32 % self.tick_minutes();
            return Some(if rem == 0 {
                ts
            } else {
                ts + chrono::Duration::minutes((self.tick_minutes() - rem) as i64)
            });
        }
        None
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

    // 禁止向集合内插入日期
    fn add_day(&mut self, _day: NaiveDate) -> Result<()> {
        Err(Error(
            "insertion of trading dates forbidden on ts collections".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_duration_between_days() -> Result<()> {
        let d1 = NaiveDate::parse_from_str("2020-02-01", "%Y-%m-%d")?;
        let d2 = NaiveDate::parse_from_str("2020-02-02", "%Y-%m-%d")?;
        assert_eq!(
            NaiveDate::signed_duration_since(d1, d2),
            chrono::Duration::days(-1)
        );

        let d3 = NaiveDate::parse_from_str("2020-03-01", "%Y-%m-%d")?;
        let d4 = NaiveDate::parse_from_str("2020-02-28", "%Y-%m-%d")?;
        assert_eq!(
            NaiveDate::signed_duration_since(d3, d4),
            chrono::Duration::days(2)
        );
        Ok(())
    }

    #[test]
    fn test_trading_dates_add_and_contain() -> Result<()> {
        let mut tdbm = LocalTradingDates::empty();
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d1)?;
        assert!(tdbm.contains_day(d1));
        Ok(())
    }

    #[test]
    fn test_trading_dates_prev_and_next() -> Result<()> {
        let mut tdbm = LocalTradingDates::empty();
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d1)?;
        let d2 = NaiveDate::parse_from_str("2020-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d2)?;
        let d3 = NaiveDate::parse_from_str("2021-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d3)?;
        let d4 = NaiveDate::parse_from_str("2021-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d4)?;

        // old days before all inserted days
        let old1 = NaiveDate::parse_from_str("2019-01-01", "%Y-%m-%d")?;
        assert_eq!(d1, tdbm.next_day(old1).unwrap());
        let old2 = NaiveDate::parse_from_str("2009-01-01", "%Y-%m-%d")?;
        assert_eq!(None, tdbm.next_day(old2));
        // new days after all inserted days
        let new1 = NaiveDate::parse_from_str("2030-01-01", "%Y-%m-%d")?;
        assert_eq!(d4, tdbm.prev_day(new1).unwrap());
        let new2 = NaiveDate::parse_from_str("2130-01-01", "%Y-%m-%d")?;
        assert_eq!(None, tdbm.prev_day(new2));
        // in between
        let m1 = NaiveDate::parse_from_str("2020-06-01", "%Y-%m-%d")?;
        assert_eq!(d2, tdbm.prev_day(m1).unwrap());
        assert_eq!(d3, tdbm.next_day(m1).unwrap());
        Ok(())
    }

    #[test]
    fn test_trading_dates_first_and_last() -> Result<()> {
        let mut tdbm = LocalTradingDates::empty();
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d1)?;
        let d2 = NaiveDate::parse_from_str("2020-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d2)?;
        let d3 = NaiveDate::parse_from_str("2021-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d3)?;
        let d4 = NaiveDate::parse_from_str("2021-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d4)?;

        assert_eq!(d1, tdbm.first_day().unwrap());
        assert_eq!(d4, tdbm.last_day().unwrap());

        Ok(())
    }

    #[test]
    fn test_trading_dates_all() -> Result<()> {
        let mut tdbm = LocalTradingDates::empty();
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d1)?;
        let d2 = NaiveDate::parse_from_str("2020-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d2)?;
        let d3 = NaiveDate::parse_from_str("2021-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d3)?;
        let d4 = NaiveDate::parse_from_str("2021-02-04", "%Y-%m-%d")?;
        tdbm.add_day(d4)?;

        assert_eq!(vec![d1, d2, d3, d4], tdbm.all_days());
        Ok(())
    }

    #[test]
    fn test_trading_add_day_str() -> Result<()> {
        let tdbm = {
            let mut r = LocalTradingDates::empty();
            r.add_day_str("2020-01-02");
            r.add_day_str("2020-02-04");
            r.add_day_str("2021-01-02");
            r.add_day_str("2021-02-04");
            r
        };
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        let d2 = NaiveDate::parse_from_str("2020-02-04", "%Y-%m-%d")?;
        let d3 = NaiveDate::parse_from_str("2021-01-02", "%Y-%m-%d")?;
        let d4 = NaiveDate::parse_from_str("2021-02-04", "%Y-%m-%d")?;
        assert_eq!(vec![d1, d2, d3, d4], tdbm.all_days());
        Ok(())
    }

    #[test]
    fn test_trading_ts_tick_and_minutes() -> Result<()> {
        let ltts1 = LocalTradingTimestamps::new("1m", Arc::new(LocalTradingDates::empty()))?;
        assert_eq!("1m".to_owned(), ltts1.tick());
        assert_eq!(1, ltts1.tick_minutes());
        let ltts2 = LocalTradingTimestamps::new("5m", Arc::new(LocalTradingDates::empty()))?;
        assert_eq!("5m".to_owned(), ltts2.tick());
        assert_eq!(5, ltts2.tick_minutes());
        let ltts3 = LocalTradingTimestamps::new("30m", Arc::new(LocalTradingDates::empty()))?;
        assert_eq!("30m".to_owned(), ltts3.tick());
        assert_eq!(30, ltts3.tick_minutes());
        Ok(())
    }

    #[test]
    fn test_trading_ts_prev_and_next_tick() -> Result<()> {
        let mut tdbm = LocalTradingDates::empty();
        tdbm.add_day_str("2020-02-01");
        tdbm.add_day_str("2020-02-02");
        let ltts = LocalTradingTimestamps::new("30m", Arc::new(tdbm))?;
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

    #[test]
    fn test_trading_dates_align() -> Result<()> {
        let ts1 = NaiveDateTime::from_str("2020-02-17T09:00:00")?;
        assert_eq!(None, LOCAL_DATES.aligned_tick(ts1));
        let ts2 = NaiveDateTime::from_str("2020-02-17T09:40:00")?;
        assert_eq!(
            Some(NaiveDateTime::from_str("2020-02-17T15:00:00")?),
            LOCAL_DATES.aligned_tick(ts2)
        );
        let ts3 = NaiveDateTime::from_str("2020-02-17T19:00:00")?;
        assert_eq!(None, LOCAL_DATES.aligned_tick(ts3));
        Ok(())
    }

    #[test]
    fn test_trading_ts_align() -> Result<()> {
        let ts1 = NaiveDateTime::from_str("2020-02-17T09:00:00")?;
        assert_eq!(None, LOCAL_TS_1_MIN.aligned_tick(ts1));
        assert_eq!(None, LOCAL_TS_5_MIN.aligned_tick(ts1));
        assert_eq!(None, LOCAL_TS_30_MIN.aligned_tick(ts1));
        let ts2 = NaiveDateTime::from_str("2020-02-17T19:00:00")?;
        assert_eq!(None, LOCAL_TS_1_MIN.aligned_tick(ts2));
        assert_eq!(None, LOCAL_TS_5_MIN.aligned_tick(ts2));
        assert_eq!(None, LOCAL_TS_30_MIN.aligned_tick(ts2));
        let ts3 = NaiveDateTime::from_str("2020-02-17T09:41:00")?;
        assert_eq!(
            Some(NaiveDateTime::from_str("2020-02-17T09:41:00")?),
            LOCAL_TS_1_MIN.aligned_tick(ts3)
        );
        assert_eq!(
            Some(NaiveDateTime::from_str("2020-02-17T09:45:00")?),
            LOCAL_TS_5_MIN.aligned_tick(ts3)
        );
        assert_eq!(
            Some(NaiveDateTime::from_str("2020-02-17T10:00:00")?),
            LOCAL_TS_30_MIN.aligned_tick(ts3)
        );
        let ts4 = NaiveDateTime::from_str("2020-02-17T10:00:00")?;
        assert_eq!(Some(ts4), LOCAL_TS_1_MIN.aligned_tick(ts4));
        assert_eq!(Some(ts4), LOCAL_TS_5_MIN.aligned_tick(ts4));
        assert_eq!(Some(ts4), LOCAL_TS_30_MIN.aligned_tick(ts4));
        Ok(())
    }
}
