use crate::shape::{Parting, Stroke};
use crate::Result;
use tanglism_utils::TradingTimestamps;
use bigdecimal::BigDecimal;
use std::str::FromStr;
use lazy_static::*;

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
    pub judge: StrokeJudge,
    pub backtrack: StrokeBacktrack,
}

impl Default for StrokeConfig {
    fn default() -> Self {
        StrokeConfig{
            judge: StrokeJudge::IndepK,
            backtrack: StrokeBacktrack::None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StrokeJudge {
    // 存在独立K线成笔
    IndepK,
    // 不需要存在独立K线即可成笔
    NonIndepK,
    // 开盘缺口，是否包含下午盘开盘
    GapOpening(bool),
    // 比例缺口
    GapRatio(BigDecimal),
}

#[derive(Debug, Clone)]
pub enum StrokeBacktrack {
    None,
    Diff(BigDecimal),
}

lazy_static! {
    static ref GAP_MINIMAL_BASE: BigDecimal = BigDecimal::from_str("0.01").unwrap();
    static ref GAP_ZERO: BigDecimal = BigDecimal::from(0);
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
                }
            } else {
                // 异向顶底间满足顶比底高，且有独立K线
                if (pt.top && pt.extremum_price > sk.end_pt.extremum_price)
                    || (!pt.top && pt.extremum_price < sk.end_pt.extremum_price)
                {
                    if self.stroke_completed(&sk.end_pt, &pt) {
                        // 成笔
                        let new_sk = Stroke {
                            start_pt: sk.end_pt.clone(),
                            end_pt: pt,
                        };
                        self.sks.push(new_sk);
                    } else if self.backtrack_last_stroke(&pt, sk) {
                        self.sks.pop().unwrap();
                        self.sks.last_mut().unwrap().end_pt = pt;
                    } 
                    
                    // else if let StrokeBacktrack::Diff(ref d) = self.cfg.backtrack {
                    //     // 当不存在独立K线，且开启回溯(backtrack)模式时，如果超越了当前笔的起始点（高于顶分型或低于底分型）
                    //     // 则修改当前笔的前一笔
                    //     if self.sks.len() >= 2
                    //         && ((pt.top && pt.extremum_price > sk.start_pt.extremum_price)
                    //             || (!pt.top && pt.extremum_price < sk.start_pt.extremum_price))
                    //     {
                    //         self.sks.pop().unwrap();
                    //         self.sks.last_mut().unwrap().end_pt = pt;
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
                if self.stroke_completed(&p, &pt) {
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

    // 成笔逻辑检查
    // p1为前分型，p2为后分型
    // 兜底策略为独立K线
    #[inline]
    fn stroke_completed(&self, p1: &Parting, p2: &Parting) -> bool {
        use tanglism_utils::{AFTERNOON_END, MORNING_END};

        match self.cfg.judge {
            StrokeJudge::NonIndepK => {
                if let Some(indep_ts) = self.tts.next_tick(p1.end_ts) {
                    return indep_ts <= p2.start_ts;
                }
                return false;
            }
            StrokeJudge::GapOpening(afternoon) => {
                if p1.right_gap.is_some() {
                    // 最高/低价恰好收盘
                    if p1.extremum_ts.time() == *AFTERNOON_END {
                        return true;
                    }
                    // 中午收盘
                    if afternoon && p1.extremum_ts.time() == *MORNING_END {
                        return true;
                    }
                }
                if p2.left_gap.is_some() {
                    // 最高/低价恰好收盘
                    if let Some(prev_tick) = self.tts.prev_tick(p2.extremum_ts) {
                        if prev_tick.time() == *AFTERNOON_END {
                            return true;
                        }
                        // 中午收盘
                        if afternoon && prev_tick.time() == *MORNING_END {
                            return true;
                        }
                    }
                }
            }
            StrokeJudge::GapRatio(ref ratio) => {
                if let Some(ref g1) = p1.right_gap {
                    let ratio = ratio.clone();
                    let mut diff = &g1.end_price - &g1.start_price;
                    if &diff < &*GAP_ZERO {
                        diff = - diff;
                    }
                    if &g1.start_price == &*GAP_ZERO {
                        return diff / &*GAP_MINIMAL_BASE >= ratio;
                    }
                    return diff / &g1.start_price >= ratio;
                }
            }
            _ => (),
        }
        // 兜底策略
        if let Some(indep_ts) = self.tts.next_tick(p1.end_ts) {
            return indep_ts < p2.start_ts;
        }
        false
    }

    fn backtrack_last_stroke(&self, pt: &Parting, sk: &Stroke) -> bool {
        if self.sks.len() >= 2 {
            if let StrokeBacktrack::Diff(ref d) = self.cfg.backtrack {
                if &sk.start_pt.extremum_price == &*GAP_ZERO {
                    return false;
                }
                if pt.top {
                    return &pt.extremum_price - &sk.start_pt.extremum_price > &pt.extremum_price * d;
                } else {
                    return &sk.start_pt.extremum_price - &pt.extremum_price > &pt.extremum_price * d;
                }
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
    use tanglism_utils::{TradingTimestamps, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN};
    use crate::shape::*;

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
        let pts = vec![
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
        ];
        // 不回溯
        let sks1 = pts_to_sks(&pts, &*LOCAL_TS_30_MIN)?;
        assert_eq!(3, sks1.len());
        assert_eq!(new_ts("2020-02-10 11:00"), sks1[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-10 15:00"), sks1[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-11 14:00"), sks1[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-14 14:30"), sks1[2].end_pt.extremum_ts);
        // 回溯，差值1%
        let sks2 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::IndepK, backtrack: StrokeBacktrack::Diff(BigDecimal::from_str("0.01").unwrap())}).run()?;
        assert_eq!(3, sks1.len());
        assert_eq!(new_ts("2020-02-10 11:00"), sks2[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-10 15:00"), sks2[0].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-13 10:00"), sks2[1].end_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-14 14:30"), sks2[2].end_pt.extremum_ts);
        Ok(())
    }

    // 测试不同的成笔逻辑选项
    #[test]
    fn test_shaper_one_stroke_gap() -> Result<()> {
        let mut pt1 = new_pt30("2020-02-13 15:00", 10.00, false);
        pt1.right_gap = Some(Gap{ts: new_ts("2020-02-14 10:00"), start_price: BigDecimal::from(10.00), end_price: BigDecimal::from(10.50)});
        let mut pt2 = new_pt30("2020-02-14 10:00", 10.50, true);
        pt2.left_gap = Some(Gap{ts: new_ts("2020-02-13 15:00"), start_price: BigDecimal::from(10.00), end_price: BigDecimal::from(10.50)});
        let pts = vec![pt1, pt2];
        let sks1 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::IndepK, backtrack: StrokeBacktrack::None}).run().unwrap();
        assert_eq!(0, sks1.len());
        let sks2 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::NonIndepK, backtrack: StrokeBacktrack::None}).run().unwrap();
        assert_eq!(0, sks2.len());
        let sks3 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::GapOpening(false), backtrack: StrokeBacktrack::None}).run().unwrap();
        assert_eq!(1, sks3.len());
        let sks4 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::GapRatio(BigDecimal::from(0.01)), backtrack: StrokeBacktrack::None}).run().unwrap();
        assert_eq!(1, sks4.len());
        let sks5 = StrokeShaper::new(&pts, &*LOCAL_TS_30_MIN, StrokeConfig{judge: StrokeJudge::GapRatio(BigDecimal::from(0.08)), backtrack: StrokeBacktrack::None}).run().unwrap();
        assert_eq!(0, sks5.len());
        Ok(())
    }

    // // 中粮糖业2020.03.30 ~ 2020.03.21
    // #[test]
    // fn test_shaper_two_strokes_gap() -> Result<()> {
    //     let pts = vec![
    //         new_pt("2020-03-30 14:50", "2020-03-30 14:30", "2020-03-31 09:35", 8, 8.45, false),
    //         new_pt("2020-03-31 13:55", "2020-03-31 13:50", "2020-03-31 14:05", 4, 8.87, true),
    //         new_pt("2020-03-31 14:10", "2020-03-31 14:05", "2020-03-31 14:15", 3, 8.72, false),
    //         new_pt("2020-03-31 14:20", "2020-03-31 14:15", "2020-03-31 14:25", 3, 8.85, true),
    //         new_pt("2020-03-31 14:35", "2020-03-31 14:25", "2020-03-31 14:55", 7, 8.78, false),
    //         new_pt("2020-03-31 15:00", "2020-03-31 14:40", "2020-04-01 09:40", 7, 9.16, true),
    //         new_pt("2020-04-01 09:55", "2020-04-01 09:45", "2020-04-01 10:15", 7, 8.69, false),
    //         new_pt("2020-04-01 10:25", "2020-04-01 10:20", "2020-04-01 10:35", 4, 8.89, true),
    //         new_pt("2020-04-01 11:25", "2020-04-01 10:45", "2020-04-01 13:10", 12, 8.75, false),
    //     ];
    //     let sks = pts_to_sks(&pts, &*LOCAL_TS_5_MIN)?;
    //     println!("{} {}", sks[0].end_pt.extremum_ts, sks[0].end_pt.extremum_price);
    //     Ok(())
    // }

    #[test]
    fn test_shaper_stroke_backtrack() -> Result<()> {
        let pts = vec![
            new_pt1("2020-03-30 09:40", 9.90, false),
            new_pt1("2020-03-30 09:50", 10.00, true),
            new_pt1("2020-03-30 10:00", 9.95, false),
            new_pt1("2020-03-30 10:01", 10.15, true),
        ];
        // 不开启回溯
        let sks1 = pts_to_sks(&pts, &*LOCAL_TS_1_MIN)?;
        assert_eq!(2, sks1.len());
        assert_eq!(new_ts("2020-03-30 10:00"), sks1[1].end_pt.extremum_ts);
        // 开启回溯，价差为1%
        let sks2 = StrokeShaper::new(&pts, &*LOCAL_TS_1_MIN, StrokeConfig{judge: StrokeJudge::IndepK, backtrack: StrokeBacktrack::Diff(BigDecimal::from_str("0.01").unwrap())}).run()?;
        assert_eq!(1, sks2.len());
        assert_eq!(new_ts("2020-03-30 10:01"), sks2[0].end_pt.extremum_ts);
        // 开启回溯，价差为3%
        let sks3 = StrokeShaper::new(&pts, &*LOCAL_TS_1_MIN, StrokeConfig{judge: StrokeJudge::IndepK, backtrack: StrokeBacktrack::Diff(BigDecimal::from_str("0.03").unwrap())}).run()?;
        assert_eq!(2, sks3.len());
        assert_eq!(new_ts("2020-03-30 10:00"), sks3[1].end_pt.extremum_ts);
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
            left_gap: None,
            right_gap: None,
        }
    }
    fn new_pt(extremum_ts: &str, start_ts: &str, end_ts: &str, n: i32, price: f64, top: bool) -> Parting {
        let extremum_ts = new_ts(extremum_ts);
        let start_ts = new_ts(start_ts);
        let end_ts = new_ts(end_ts);
        Parting {
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price: BigDecimal::from(price),
            n,
            top,
            left_gap: None,
            right_gap: None,
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
            left_gap: None,
            right_gap: None,
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
            left_gap: None,
            right_gap: None,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
