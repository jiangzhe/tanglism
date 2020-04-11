use crate::Result;
use crate::shape::{Parting, Stroke};
use tanglism_utils::TradingTimestamps;

/// 将分型序列解析为笔序列
///
/// 步骤：
/// 1. 选择起始点。
/// 2. 选择下一个点。
///    若异型：邻接或交叉则忽略，不邻接则成笔
///    若同型：顶更高/底更低则修改当前笔，反之则忽略
pub fn pts_to_sks<T>(pts: &[Parting], tts: &T) -> Result<Vec<Stroke>>
where
    T: TradingTimestamps,
{
    StrokeShaper::new(pts, tts).run()
}

struct StrokeShaper<'p, 't, T> {
    pts: &'p [Parting],
    tts: &'t T,
    sks: Vec<Stroke>,
    tail: Vec<Parting>,
    start: Option<Parting>,
}

impl<'p, 't, T: TradingTimestamps> StrokeShaper<'p, 't, T> {
    fn new(pts: &'p [Parting], tts: &'t T) -> Self {
        StrokeShaper {
            pts,
            tts,
            sks: Vec::new(),
            tail: Vec::new(),
            start: None,
        }
    }

    fn run(mut self) -> Result<Vec<Stroke>> {
        if self.pts.is_empty() {
            return Ok(Vec::new());
        }
        let mut pts_iter = self.pts.iter();
        let first = pts_iter.next().cloned().unwrap();
        self.start = Some(first.clone());
        self.tail.push(first);
        while let Some(pt) = pts_iter.next() {
            self.consume(pt.clone());
        }
        Ok(self.sks)
    }


    fn consume(&mut self, pt: Parting) {
        self.tail.push(pt.clone());
        if pt.top != self.start().top {
            self.consume_diff_dir(pt);
        } else {
            self.consume_same_dir(pt);
        }
    }

    fn consume_diff_dir(&mut self, pt: Parting) {
        if self.is_start_neighbor(&pt) {
            // 这里不做变化
            // 可以保留的可能性是起点跳至pt点
            return;
        }
        // 顶比底低
        if (pt.top && pt.extremum_price <= self.start().extremum_price)
            || (self.start().top && self.start().extremum_price <= pt.extremum_price)
        {
            return;
        }
        // 成笔
        let new_sk = Stroke {
            start_pt: self.start.take().unwrap(),
            end_pt: pt.clone(),
        };
        self.start = Some(pt);
        self.tail.clear();
        self.sks.push(new_sk);
    }

    fn consume_same_dir(&mut self, pt: Parting) {
        if self.is_start_neighbor(&pt) {
            return;
        }
        // 顶比起点低，底比起点高
        if (pt.top && pt.extremum_price < self.start().extremum_price)
            || (!pt.top && pt.extremum_price > self.start().extremum_price)
        {
            return;
        }

        if let Some(last_sk) = self.sks.last_mut() {
            // 有笔，需要修改笔终点
            last_sk.end_pt = pt.clone();
        }
        self.start.replace(pt);
        self.tail.clear();
    }

    fn is_start_neighbor(&self, pt: &Parting) -> bool {
        if let Some(start) = self.start.as_ref() {
            if let Some(indep_ts) = self.tts.next_tick(start.end_ts) {
                if indep_ts < pt.start_ts {
                    return false;
                }
            }
        }
        true
    }

    #[inline]
    fn start(&self) -> &Parting {
        self.start.as_ref().unwrap()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tanglism_utils::LOCAL_TS_1_MIN;
    use chrono::NaiveDateTime;
    use bigdecimal::BigDecimal;
    
    #[test]
    fn test_shaper_no_stroke() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:01", 10.10, true),
            new_pt1("2020-02-01 10:03", 9.50, false),
            new_pt1("2020-02-01 10:06", 9.80, true),
        ]);
        assert!(sks.is_empty());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.40, true),
            new_pt1("2020-02-01 10:13", 10.30, false),
        ]);
        assert_eq!(1, sks.len());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:02", 10.10, true),
            new_pt1("2020-02-01 10:04", 9.90, false),
            new_pt1("2020-02-01 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-02-01 10:04"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_non_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:02", 10.10, true),
            new_pt1("2020-02-01 10:04", 10.02, false),
            new_pt1("2020-02-01 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-02-01 10:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_strokes_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.10, true),
            new_pt1("2020-02-01 10:20", 10.02, false),
        ]);
        assert_eq!(2, sks.len());
        Ok(())
    }

    fn pts_to_sks_1_min(pts: Vec<Parting>) -> Vec<Stroke> {
        pts_to_sks(&pts, &*LOCAL_TS_1_MIN).unwrap()
    }

    fn new_pt1(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 1, price, 3, top)
    }

    fn new_pt5(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 5, price, 3, top)
    }

    fn new_pt30(ts: &str, price: f64, top: bool) -> Parting {
        new_pt_fix_width(ts, 30, price, 3, top)
    }

    fn new_pt_fix_width(ts: &str, minutes: i64, extremum_price: f64, n: i32, top: bool) -> Parting {
        let extremum_ts = new_ts(ts);
        let start_ts = extremum_ts - chrono::Duration::minutes(minutes);
        let end_ts = extremum_ts + chrono::Duration::minutes(minutes);
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(extremum_price),
            n,
            top,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
