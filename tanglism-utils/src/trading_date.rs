use chrono::prelude::*;
use crate::{Result, Error};

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

// 对交易日的范围进行全局限制
// 时间范围为：2010-01-01 ~ 2099-12-31
lazy_static! {
    static ref FIRST_DAY: NaiveDate = NaiveDate::from_ymd(2010, 1, 1);
    static ref LAST_DAY: NaiveDate = NaiveDate::from_ymd(2099, 12, 31);
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

// 位图比特数
const BITS: usize = 64;
const BITS_ONE: u64 = 1u64;
type Bits = u64;

lazy_static! {
    pub static ref LOCAL_TRADING_DATE_BITMAP: TradingDateBitmap = {
        let mut tdbm = TradingDateBitmap::empty();
        for d in tanglism_data::LOCAL_TRADE_DAYS.iter() {
            tdbm.add_day_str(d);
        }
        tdbm
    };
}

// 交易日集合的位图实现
#[derive(Debug, Clone)]
pub struct TradingDateBitmap {
    bm: Vec<Bits>,
}

impl TradingDateBitmap {

    // 创建空实例
    pub fn empty() -> Self {
        TradingDateBitmap{
            bm: Vec::new(),
        }
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
        let buckets = (capacity + BITS - 1) / BITS;
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
        let mut idx_iter = IndexIter{
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
        IndexIter{
            inner: &self,
            bucket_id: 0,
            bit_pos: 0,
        }
    }
    
}

struct IndexIter<'a> {
    inner: &'a TradingDateBitmap,
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

impl TradingDates for TradingDateBitmap {

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Result;
    use chrono::prelude::*;

    #[test]
    fn test_duration_between_days() -> Result<()> {
        let d1 = NaiveDate::parse_from_str("2020-02-01", "%Y-%m-%d")?;
        let d2 = NaiveDate::parse_from_str("2020-02-02", "%Y-%m-%d")?;
        assert_eq!(NaiveDate::signed_duration_since(d1, d2), chrono::Duration::days(-1));

        let d3 = NaiveDate::parse_from_str("2020-03-01", "%Y-%m-%d")?;
        let d4 = NaiveDate::parse_from_str("2020-02-28", "%Y-%m-%d")?;
        assert_eq!(NaiveDate::signed_duration_since(d3, d4), chrono::Duration::days(2));
        Ok(())
    }

    #[test]
    fn test_trading_dates_add_and_contain() -> Result<()> {
        let mut tdbm = TradingDateBitmap::empty();
        let d1 = NaiveDate::parse_from_str("2020-01-02", "%Y-%m-%d")?;
        tdbm.add_day(d1)?;
        assert!(tdbm.contains_day(d1));
        Ok(())
    }

    #[test]
    fn test_trading_dates_prev_and_next() -> Result<()> {
        let mut tdbm = TradingDateBitmap::empty();
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
        let mut tdbm = TradingDateBitmap::empty();
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
        let mut tdbm = TradingDateBitmap::empty();
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
            let mut r = TradingDateBitmap::empty();
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

}