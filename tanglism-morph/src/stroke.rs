use crate::shape::{Parting, Stroke};
use crate::Result;
use chrono::NaiveDateTime;
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
    StrokeShaper::new(pts, tts, StrokeConfig::default()).run()
}

#[derive(Debug, Clone)]
pub struct StrokeConfig {
    // 是否检查独立K线的存在
    pub indep_k: bool,
}

impl Default for StrokeConfig {
    fn default() -> Self {
        StrokeConfig { indep_k: true }
    }
}

/// 可使用更精细的生成配置进行笔分析
pub struct StrokeShaper<'p, 't, T> {
    pts: &'p [Parting],
    tts: &'t T,
    sks: Vec<Stroke>,
    // 表示未成笔的起点序列
    pending: Vec<Parting>,
    cfg: StrokeConfig,
}

impl<'p, 't, T: TradingTimestamps> StrokeShaper<'p, 't, T> {
    pub fn new(pts: &'p [Parting], tts: &'t T, cfg: StrokeConfig) -> Self {
        StrokeShaper {
            pts,
            tts,
            sks: Vec::new(),
            pending: Vec::new(),
            cfg,
        }
    }

    pub fn run(mut self) -> Result<Vec<Stroke>> {
        if self.pts.is_empty() {
            return Ok(Vec::new());
        }
        let mut pts_iter = self.pts.iter();
        let first = pts_iter.next().cloned().unwrap();
        self.pending = vec![first];
        for pt in pts_iter {
            self.consume(pt.clone());
        }
        Ok(self.sks)
    }

    // 依序消费每个分型
    // 可以根据当前起点的分型，分为同类型，和不同类型
    // 1. 顶底、底顶：可连成一笔
    // 2. 顶顶、底底：无法连成一笔，单需要考虑如果底比前底低，
    //    或者顶比前顶高，则需要修改前一笔的终点为该分型
    fn consume(&mut self, pt: Parting) {
        // 存在前一笔时，比较当前的分型是否与前一笔的终点分型类型一致
        // 如果一致，则比较高低，并根据情况修改笔或丢弃
        if let Some(sk) = self.sks.last() {
            // 比较方向
            if sk.end_pt.top == pt.top {
                // 顶比前顶高，或者底比前底低，直接修改该笔
                if (pt.top && pt.extremum_price > sk.end_pt.extremum_price)
                    || (!pt.top && pt.extremum_price < sk.end_pt.extremum_price)
                {
                    self.sks.last_mut().unwrap().end_pt = pt;
                    // sk.end_pt = pt;
                }
            } else {
                // 异向顶底间满足顶比底高，且有独立K线
                if (pt.top && pt.extremum_price > sk.end_pt.extremum_price)
                    || (!pt.top && pt.extremum_price < sk.end_pt.extremum_price)
                {
                    if self.indep_check(sk.end_pt.end_ts, pt.start_ts) {
                        // 成笔
                        let new_sk = Stroke {
                            start_pt: sk.end_pt.clone(),
                            end_pt: pt,
                        };
                        self.sks.push(new_sk);
                    } else {
                        // 当不存在独立K线时，如果超越了当前笔的起始点（高于顶分型或低于底分型）
                        // 则修改当前笔的前一笔
                        if self.sks.len() >= 2
                            && ((pt.top && pt.extremum_price > sk.start_pt.extremum_price)
                                || (!pt.top && pt.extremum_price < sk.start_pt.extremum_price))
                        {
                            self.sks.pop().unwrap();
                            self.sks.last_mut().unwrap().end_pt = pt;
                        }
                    }
                    // if let Some(indep_ts) = self.tts.next_tick(sk.end_pt.end_ts) {
                    //     let indep_check = if self.cfg.indep_k {
                    //         indep_ts < pt.start_ts
                    //     } else {
                    //         indep_ts <= pt.start_ts
                    //     };
                    //     if indep_check {
                    //         // 成笔
                    //         let new_sk = Stroke{
                    //             start_pt: sk.end_pt.clone(),
                    //             end_pt: pt,
                    //         };
                    //         self.sks.push(new_sk);
                    //     } else {
                    //         // 当不存在独立K线时，如果超越了当前笔的起始点（高于顶分型或低于底分型）
                    //         // 则修改当前笔的前一笔
                    //         if self.sks.len() >= 2 && ((pt.top && pt.extremum_price > sk.start_pt.extremum_price) || (!pt.top && pt.extremum_price < sk.start_pt.extremum_price)) {
                    //             self.sks.pop().unwrap();
                    //             self.sks.last_mut().unwrap().end_pt = pt;
                    //         }
                    //     }
                    // }
                }
            }
            // 不满足任一成笔条件则丢弃
            return;
        }

        // 不存在前一笔，则需要和未成笔的潜在起点序列进行比较
        let mut matches = Vec::new();
        for p in &self.pending {
            // 方向不同且顶比底高
            if pt.top != p.top
                && ((pt.top && pt.extremum_price > p.extremum_price)
                    || (!pt.top && pt.extremum_price < p.extremum_price))
            {
                // 比较独立K线
                if self.indep_check(p.end_ts, pt.start_ts) {
                    // 成笔
                    let new_sk = Stroke {
                        start_pt: p.clone(),
                        end_pt: pt.clone(),
                    };
                    matches.push(new_sk);
                }
            }
        }
        // 与未成笔序列无法成笔时，加入未成笔序列
        if matches.is_empty() {
            self.pending.push(pt);
            return;
        }
        // 在是否成笔的判断中，我们取差距更大的分型作为起点，
        // 即如果有多个底可以和顶分型构成一笔，这里取较低的底。
        // 反之亦然。
        let mut r = matches.pop().unwrap();
        while let Some(m) = matches.pop() {
            if (&r.start_pt.extremum_price - &r.end_pt.extremum_price).abs()
                < (&m.start_pt.extremum_price - &m.end_pt.extremum_price).abs()
            {
                r = m;
            }
        }
        self.sks.push(r);
        self.pending.clear();
    }

    // 独立K线检查逻辑
    // t1为前分型的结束时刻，t2位后分型的开始时刻
    #[inline]
    fn indep_check(&self, end_ts: NaiveDateTime, start_ts: NaiveDateTime) -> bool {
        if let Some(indep_ts) = self.tts.next_tick(end_ts) {
            if self.cfg.indep_k {
                return indep_ts < start_ts;
            } else {
                return indep_ts <= start_ts;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDateTime;
    use tanglism_utils::{TradingTimestamps, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN};

    #[test]
    fn test_shaper_no_stroke() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:01", 10.10, true),
            new_pt1("2020-01-07 10:03", 9.50, false),
            new_pt1("2020-01-07 10:06", 9.80, true),
        ]);
        assert!(sks.is_empty());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:10", 10.40, true),
            new_pt1("2020-01-07 10:13", 10.30, false),
        ]);
        assert_eq!(1, sks.len());
        Ok(())
    }

    // 一笔，起点移动，因为起点不是最低的底分型
    #[test]
    fn test_shaper_one_stroke_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:02", 10.10, true),
            new_pt1("2020-01-07 10:04", 9.90, false),
            new_pt1("2020-01-07 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:04"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-07 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 一笔，起点不一定，因为起点是最低的底分型
    #[test]
    fn test_shaper_one_stroke_non_moving_start() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:02", 10.10, true),
            new_pt1("2020-01-07 10:04", 10.02, false),
            new_pt1("2020-01-07 10:10", 10.30, true),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-07 10:10"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 简单的两笔
    #[test]
    fn test_shaper_two_strokes_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-01-07 10:00", 10.00, false),
            new_pt1("2020-01-07 10:10", 10.10, true),
            new_pt1("2020-01-07 10:20", 10.02, false),
        ]);
        assert_eq!(2, sks.len());
        Ok(())
    }

    // 一笔，分型跨天
    #[test]
    fn test_shaper_one_stroke_across_days() -> Result<()> {
        let sks = pts_to_sks_30_min(vec![
            new_pt30("2020-01-07 10:00", 10.00, true),
            new_pt30("2020-01-08 10:00", 9.50, false),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-01-07 10:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-01-08 10:00"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 一笔，开盘跳空，2020.3.11 ~ 2020.3.13 贵州茅台
    // 1. 后底低于前底
    // 2. K线包含
    #[test]
    fn test_shaper_one_complex_stroke_across_days() -> Result<()> {
        let sks = pts_to_sks_30_min(vec![
            ts_pt30(
                "2020-03-11 13:30",
                1169.50,
                true,
                "2020-03-11 11:00",
                "2020-03-11 14:00",
            ),
            ts_pt30(
                "2020-03-11 14:00",
                1156.70,
                false,
                "2020-03-11 11:30",
                "2020-03-11 14:30",
            ),
            ts_pt30(
                "2020-03-11 15:00",
                1167.40,
                true,
                "2020-03-11 14:30",
                "2020-03-12 10:00",
            ),
            ts_pt30(
                "2020-03-12 10:30",
                1125.10,
                false,
                "2020-03-12 10:00",
                "2020-03-12 14:00",
            ),
            ts_pt30(
                "2020-03-12 11:00",
                1147.98,
                true,
                "2020-03-12 10:30",
                "2020-03-12 15:00",
            ),
            ts_pt30(
                "2020-03-13 10:00",
                1080.00,
                false,
                "2020-03-12 14:30",
                "2020-03-13 14:00",
            ),
            ts_pt30(
                "2020-03-13 13:30",
                1128.92,
                true,
                "2020-03-13 10:00",
                "2020-03-13 15:00",
            ),
        ]);
        assert_eq!(1, sks.len());
        assert_eq!(new_ts("2020-03-11 13:30"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-03-13 10:00"), sks[0].end_pt.extremum_ts);
        Ok(())
    }

    // 三笔，2020.2.10 ~ 2020.2.14 贵州茅台
    // todo
    #[test]
    fn test_shaper_three_strokes() -> Result<()> {
        let sks = pts_to_sks_30_min(vec![
            ts_pt30(
                "2020-02-10 11:00",
                1074.56,
                true,
                "2020-02-10 10:30",
                "2020-02-10 11:30",
            ),
            ts_pt30(
                "2020-02-10 13:30",
                1061.80,
                false,
                "2020-02-10 11:30",
                "2020-02-10 14:00",
            ),
            ts_pt30(
                "2020-02-10 14:00",
                1067.00,
                true,
                "2020-02-10 13:30",
                "2020-02-10 15:00",
            ),
            ts_pt30(
                "2020-02-10 15:00",
                1062.01,
                false,
                "2020-02-10 14:00",
                "2020-02-11 10:00",
            ),
            ts_pt30(
                "2020-02-11 14:00",
                1099.66,
                true,
                "2020-02-11 11:00",
                "2020-02-12 10:00",
            ),
            ts_pt30(
                "2020-02-12 10:30",
                1085.88,
                false,
                "2020-02-12 10:00",
                "2020-02-12 11:00",
            ),
            ts_pt30(
                "2020-02-12 11:30",
                1098.79,
                true,
                "2020-02-12 11:00",
                "2020-02-12 14:00",
            ),
            ts_pt30(
                "2020-02-12 13:30",
                1090.30,
                false,
                "2020-02-12 11:30",
                "2020-02-12 14:30",
            ),
            ts_pt30(
                "2020-02-13 10:00",
                1113.83,
                true,
                "2020-02-12 15:00",
                "2020-02-13 11:00",
            ),
            ts_pt30(
                "2020-02-13 13:30",
                1088.21,
                false,
                "2020-02-13 11:30",
                "2020-02-13 15:00",
            ),
            ts_pt30(
                "2020-02-13 14:30",
                1093.64,
                true,
                "2020-02-13 13:30",
                "2020-02-14 11:00",
            ),
            ts_pt30(
                "2020-02-14 10:00",
                1086.01,
                false,
                "2020-02-13 14:30",
                "2020-02-14 11:30",
            ),
            ts_pt30(
                "2020-02-14 11:30",
                1092.00,
                true,
                "2020-02-14 10:00",
                "2020-02-14 13:30",
            ),
            ts_pt30(
                "2020-02-14 14:30",
                1083.11,
                false,
                "2020-02-14 13:30",
                "2020-02-14 15:00",
            ),
        ]);
        assert_eq!(3, sks.len());
        assert_eq!(new_ts("2020-02-10 11:00"), sks[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-10 15:00"), sks[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-13 10:00"), sks[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-14 14:30"), sks[2].end_pt.extremum_ts);
        Ok(())
    }

    fn pts_to_sks_1_min(pts: Vec<Parting>) -> Vec<Stroke> {
        pts_to_sks(&pts, &*LOCAL_TS_1_MIN).unwrap()
    }

    fn new_pt1(ts: &str, price: f64, top: bool) -> Parting {
        let extremum_ts = new_ts(ts);
        let start_ts = LOCAL_TS_1_MIN.prev_tick(extremum_ts).unwrap();
        let end_ts = LOCAL_TS_1_MIN.next_tick(extremum_ts).unwrap();
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n: 3,
            top,
        }
    }

    fn pts_to_sks_30_min(pts: Vec<Parting>) -> Vec<Stroke> {
        pts_to_sks(&pts, &*LOCAL_TS_30_MIN).unwrap()
    }

    fn new_pt30(ts: &str, price: f64, top: bool) -> Parting {
        let extremum_ts = new_ts(ts);
        let start_ts = LOCAL_TS_30_MIN.prev_tick(extremum_ts).unwrap();
        let end_ts = LOCAL_TS_30_MIN.next_tick(extremum_ts).unwrap();
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n: 3,
            top,
        }
    }

    fn ts_pt30(ts: &str, price: f64, top: bool, start_ts: &str, end_ts: &str) -> Parting {
        let start_ts = new_ts(start_ts);
        let end_ts = new_ts(end_ts);
        let extremum_ts = new_ts(ts);
        let mut start = start_ts;
        let mut n = 1;
        while let Some(next_ts) = LOCAL_TS_30_MIN.next_tick(start) {
            n += 1;
            if next_ts == end_ts {
                break;
            }
            if n > 100 {
                panic!("exceeds max n of single parting");
            }
            start = next_ts;
        }

        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n,
            top,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
