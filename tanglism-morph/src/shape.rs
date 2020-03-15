use crate::{Error, Parting, Result, Segment, Stroke, CK, K};
use serde_derive::*;
use tanglism_utils::TradingTimestamps;

/// 单标的确定周期的数据来源
pub trait Source {
    // 给定代码和时刻，获取该时刻前的不多于limit条数的K线数据
    fn data_before(&self, ts: &str, limit: u32) -> Vec<K>;

    // 给定代码和时刻，获取该时刻后的不多余limit条数的K线数据
    fn data_after(&self, ts: &str, limit: u32) -> Vec<K>;
}

/// 分型序列
///
/// 包含潜在分型的序列，以及未能形成分型的尾部K线
/// 可通过输入最新K线，更新或延长已有序列
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PartingSeq {
    pub body: Vec<Parting>,
    pub tail: Vec<CK>,
}

/// 笔序列
///
/// 包含笔序列，以及未形成笔的尾部分型
pub struct StrokeSeq {
    pub body: Vec<Stroke>,
    // 笔尾
    // 包含未能成笔的顶底分型及合成K线
    pub tail: Option<PartingSeq>,
}

/// 线段序列
pub struct SegmentSeq {
    pub determined: Vec<Segment>,
    pub undetermined: Vec<Stroke>,
    pub undetermined_top: bool,
}

/// 将K线图解析为分型序列
pub fn ks_to_pts(ks: &[K]) -> Result<PartingSeq> {
    PartingShaper::new(ks).run()
}

struct PartingShaper<'k> {
    ks: &'k [K],
    body: Vec<Parting>,
    first_k: Option<CK>,
    second_k: Option<CK>,
    third_k: Option<CK>,
    upward: bool,
}

impl<'k> PartingShaper<'k> {
    fn new(ks: &'k [K]) -> Self {
        PartingShaper{
            ks,
            body: Vec::new(),
            first_k: None,
            second_k: None,
            third_k: None,
            upward: true,
        }
    }

    fn consume(&mut self, k: &K) {
        // k1不存在
        if self.first_k.is_none() {
            self.first_k = Some(Self::k_to_ck(k));
            return;
        }
        // k1存在
        let k1 = self.first_k.as_ref().unwrap();

        // k2不存在
        if self.second_k.is_none() {
            // 检查k1与k的包含关系
            match Self::inclusive_neighbor_k(k1, k, self.upward) {
                None => {
                    // 更新k2
                    self.second_k = Some(Self::k_to_ck(k));
                    self.upward = k.high > k1.high;
                    return;
                }
                ck => {
                    // 合并k1与k
                    self.first_k = ck;
                    return;
                }
            }
        }

        // k2存在
        let k2 = self.second_k.as_ref().unwrap();

        // k3不存在
        if self.third_k.is_none() {
            // 检查k2与k的包含关系
            let ck = Self::inclusive_neighbor_k(k2, k, self.upward);
            if ck.is_some() {
                // 更新k2
                self.second_k = ck;
                return;
            }
            // 检查k1, k2与k是否形成顶/底分型
            if (self.upward && k.low < k2.low) || (!self.upward && k.high > k2.high) {
                // 形成顶/底分型，更新k2和k3，并将走势颠倒
                self.third_k = Some(Self::k_to_ck(k));
                self.upward = !self.upward;
                return;
            }

            // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变
            self.first_k = self.second_k.take();
            self.second_k = Some(Self::k_to_ck(k));
            return;
        }

        let k3 = self.third_k.as_ref().unwrap();

        // 检查k3与k的包含关系
        let ck = Self::inclusive_neighbor_k(k3, k, self.upward);
        if ck.is_some() {
            // 更新k3
            self.third_k = ck;
            return;
        }

        //不包含，需构建分型并记录
        let parting = Parting {
            start_ts: k1.start_ts,
            end_ts: k3.end_ts,
            extremum_ts: k2.extremum_ts,
            extremum_price: if self.upward { k2.low } else { k2.high },
            n: k1.n + k2.n + k3.n,
            top: !self.upward,
        };
        self.body.push(parting);

        // 当k2, k3, k形成顶底分型时，左移1位
        if (self.upward && k.low < k3.low) || (!self.upward && k.high > k3.high) {
            self.first_k = self.second_k.take();
            self.second_k = self.third_k.take();
            self.third_k = Some(Self::k_to_ck(k));
            self.upward = !self.upward;
            return;
        }

        // 不形成分型时，将k3, k向左移两位
        self.upward = k.high > k3.high;
        self.first_k = Some(k3.clone());
        self.second_k = Some(Self::k_to_ck(k));
        self.third_k = None;
    }

    fn run(mut self) -> Result<PartingSeq> {
        for k in self.ks.iter() {
            self.consume(k);
        }
    
        // 结束所有k线分析后，依然存在第三根K线，说明此时三根K线刚好构成顶底分型
        if self.third_k.is_some() {
            let k1 = self.first_k.take().unwrap();
            let k2 = self.second_k.take().unwrap();
            let k3 = self.third_k.take().unwrap();
    
            let parting = Parting {
                start_ts: k1.start_ts,
                end_ts: k3.end_ts,
                extremum_ts: k2.extremum_ts,
                extremum_price: if self.upward { k2.low } else { k2.high },
                n: k1.n + k2.n + k3.n,
                top: !self.upward,
            };
            self.body.push(parting);
            // 向左平移k2和k3
            self.first_k = Some(k2);
            self.second_k = Some(k3);
        }
    
        let mut tail = vec![];
        // 将剩余k线加入尾部，必定不会出现三根K线
        if let Some(fk) = self.first_k {
            tail.push(fk);
        }
        if let Some(sk) = self.second_k {
            tail.push(sk);
        }
        Ok(PartingSeq{ 
            body: self.body, 
            tail,
        })
    }

    /// 辅助函数，将单个K线转化为合并K线
    fn k_to_ck(k: &K) -> CK {
        CK {
            start_ts: k.ts,
            end_ts: k.ts,
            extremum_ts: k.ts,
            high: k.high,
            low: k.low,
            n: 1,
        }
    }

    
    /// 辅助函数，判断相邻K线是否符合包含关系，并在符合情况下返回包含后的合并K线
    fn inclusive_neighbor_k(k1: &CK, k2: &K, upward: bool) -> Option<CK> {
        let extremum_ts = if k1.high >= k2.high && k1.low <= k2.low {
            k1.extremum_ts
        } else if k2.high >= k1.high && k2.low <= k1.low {
            k2.ts
        } else {
            return None;
        };

        let start_ts = k1.start_ts;
        let end_ts = k2.ts;
        let n = k1.n + 1;

        let (high, low) = if upward {
            (
                if k1.high > k2.high { k1.high } else { k2.high },
                if k1.low > k2.low { k1.low } else { k2.low },
            )
        } else {
            (
                if k1.high < k2.high { k1.high } else { k2.high },
                if k1.low < k2.low { k1.low } else { k2.low },
            )
        };
        Some(CK {
            start_ts,
            end_ts,
            extremum_ts,
            high,
            low,
            n,
        })
    }
}

/// 将分型序列解析为笔序列
/// 
/// 步骤：
/// 1. 选择起始点。
/// 2. 选择下一个点。
///    若异型：邻接或交叉则忽略，不邻接则成笔
///    若同型：顶更高/底更低则修改当前笔，反之则忽略
pub fn pts_to_sks<T>(pts: &PartingSeq, tts: &T) -> Result<StrokeSeq>
where T: TradingTimestamps,
{
    StrokeShaper::new(pts, tts).run()
}

struct StrokeShaper<'p, 't, T> {
    pts: &'p PartingSeq,
    tts: &'t T,
    sks: Vec<Stroke>,
    tail: Vec<Parting>,
    start: Option<Parting>,
}

impl<'p, 't, T: TradingTimestamps> StrokeShaper<'p, 't, T> {
    fn new(pts: &'p PartingSeq, tts: &'t T) -> Self {
        StrokeShaper{
            pts,
            tts,
            sks: Vec::new(),
            tail: Vec::new(),
            start: None,
        }
    }

    fn run(mut self) -> Result<StrokeSeq> {
        if self.pts.body.is_empty() {
            return Ok(StrokeSeq{
                body: Vec::new(),
                tail: Some(self.pts.clone()),
            });
        }
        let mut pts_iter = self.pts.body.iter();
        let first = pts_iter.next().unwrap().clone();
        self.start = Some(first.clone());
        self.tail.push(first);
        while let Some(pt) = pts_iter.next() {
            self.consume(pt);
        }
        Ok(StrokeSeq{
            body: self.sks,
            tail: Some(PartingSeq{
                body: self.tail,
                tail: self.pts.tail.clone(),
            }),
        })
    }

    fn consume(&mut self, pt: &Parting) {
        self.tail.push(pt.clone());
        if pt.top != self.start().top {
            self.consume_diff_dir(pt);
        } else {
            self.consume_same_dir(pt);
        }
    }

    fn consume_diff_dir(&mut self, pt: &Parting) {
        if self.is_start_neighbor(pt) {
            // 这里不做变化
            // 可以保留的可能性是起点跳至pt点
            return;
        }
        // 顶比底低
        if (pt.top && pt.extremum_price <= self.start().extremum_price) || (self.start().top && self.start().extremum_price <= pt.extremum_price) {
            return;
        }
        // 成笔
        let new_sk = Stroke{
            start_pt: self.start.take().unwrap(),
            end_pt: pt.clone(),
        };
        self.start = Some(pt.clone());
        self.tail.clear();
        self.sks.push(new_sk);
    }

    fn consume_same_dir(&mut self, pt: &Parting) {
        if self.is_start_neighbor(pt) {
            return;
        }
        // 顶比起点低，底比起点高
        if (pt.top && pt.extremum_price < self.start().extremum_price) || (!pt.top && pt.extremum_price > self.start().extremum_price) {
            return;
        }
        
        if let Some(last_sk) = self.sks.last_mut() {
            // 有笔，需要修改笔终点
            last_sk.end_pt = pt.clone();
            
        }
        self.start.replace(pt.clone());
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

/// 将笔序列解析为线段序列
pub fn sks_to_sgs(sks: &StrokeSeq) -> Result<SegmentSeq> {
    unimplemented!()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::K;
    use chrono::NaiveDateTime;
    use tanglism_utils::LOCAL_TRADING_TS_1_MIN;

    #[test]
    fn test_shaper_no_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.25, 10.15),
            new_k("2020-02-01 10:04", 10.30, 10.20),
        ];
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        let r = ks_to_pts(&ks)?;
        assert_eq!(0, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:03"), r.tail[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04"), r.tail[1].start_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.10, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02"), r.body[0].extremum_ts);
        assert_eq!(10.20, r.body[0].extremum_price);
        assert_eq!(true, r.body[0].top);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting_inclusive() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        // panic!(json);
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:04"), r.body[0].end_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.20, 10.10),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(true, r.body[0].top);
        assert_eq!(new_ts("2020-02-01 10:02"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04"), r.body[1].end_ts);
        assert_eq!(false, r.body[1].top);
        assert_eq!(2, r.tail.len());
        Ok(())
    }

    #[test]
    fn test_shaper_two_indep_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00", 10.10, 10.00),
            new_k("2020-02-01 10:01", 10.15, 10.05),
            new_k("2020-02-01 10:02", 10.20, 10.10),
            new_k("2020-02-01 10:03", 10.15, 10.05),
            new_k("2020-02-01 10:04", 10.10, 10.00),
            new_k("2020-02-01 10:05", 10.05, 9.95),
            new_k("2020-02-01 10:06", 10.00, 9.90),
            new_k("2020-02-01 10:07", 10.05, 9.95),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:05"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:07"), r.body[1].end_ts);
        Ok(())
    }

    
    #[test]
    fn test_shaper_no_stroke() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:01", 10.10, true),
            new_pt1("2020-02-01 10:03", 9.50, false),
            new_pt1("2020-02-01 10:06", 9.80, true),
        ]);
        assert!(sks.body.is_empty());
        assert_eq!(4, sks.tail.unwrap().body.len());
        Ok(())
    }

    #[test]
    fn test_shaper_one_stroke_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.40, true),
            new_pt1("2020-02-01 10:13", 10.30, false),
        ]);
        assert_eq!(1, sks.body.len());
        assert_eq!(1, sks.tail.unwrap().body.len());
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
        assert_eq!(1, sks.body.len());
        assert_eq!(new_ts("2020-02-01 10:04"), sks.body[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks.body[0].end_pt.extremum_ts);
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
        assert_eq!(1, sks.body.len());
        assert_eq!(new_ts("2020-02-01 10:00"), sks.body[0].start_pt.extremum_ts);
        assert_eq!(new_ts("2020-02-01 10:10"), sks.body[0].end_pt.extremum_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_strokes_simple() -> Result<()> {
        let sks = pts_to_sks_1_min(vec![
            new_pt1("2020-02-01 10:00", 10.00, false),
            new_pt1("2020-02-01 10:10", 10.10, true),
            new_pt1("2020-02-01 10:20", 10.02, false),
        ]);
        assert_eq!(2, sks.body.len());
        Ok(())
    }

    fn pts_to_sks_1_min(pts: Vec<Parting>) -> StrokeSeq {
        pts_to_sks(&new_pts(pts), &*LOCAL_TRADING_TS_1_MIN).unwrap()
    }

    fn new_pts(pts: Vec<Parting>) -> PartingSeq {
        PartingSeq{
            body: pts,
            tail: vec![],
        }
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
        Parting{
            start_ts,
            extremum_ts,
            end_ts,
            extremum_price,
            n,
            top,
        }
    }

    fn new_k(ts: &str, high: f64, low: f64) -> K {
        K {
            ts: new_ts(ts),
            high,
            low,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
}
