use crate::{Result, Error, K, CK, Parting, Stroke, Segment, Center, Trend};
use serde_derive::*;

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
#[derive(Debug, Serialize, Deserialize)]
pub struct PartingSeq {
    pub pts: Vec<Parting>,
    pub tail: Vec<CK>,
}

/// 笔序列
/// 
/// 包含笔序列，以及未形成笔的尾部分型
pub struct StrokeSeq {
    pub sks: Vec<Stroke>,
    pub tail: Vec<Parting>,
}

pub struct SegmentSeq {
    pub determined: Vec<Segment>,
    pub undetermined: Vec<Stroke>,
    pub undetermined_top: bool,
}

pub trait KShaper {
    // 使用一组连续K线初始化
    fn init_ks(&mut self, ks: Vec<K>) -> Result<()>;
    // 获取当前K线
    fn ks(&self) -> &[K];
}

pub trait PartingShaper: KShaper {
    // 初始化分型序列
    fn init_pts(&mut self) -> Result<()>;
    // 获取当前的分型序列
    fn parting_seq(&self) -> Option<&PartingSeq>;
}

pub trait StrokeShaper: PartingShaper {
    // 初始化笔序列
    fn init_sks(&mut self) -> Result<()>;
    // 获取当前的笔序列
    fn stroke_seq(&self) -> Option<&StrokeSeq>;
}

pub trait SegmentShaper: StrokeShaper {
    // 初始化线段序列
    fn init_sgs(&mut self) -> Result<()>;
    // 获取当前的线段序列
    fn segment_seq(&self) -> Option<&SegmentSeq>;    
}

pub struct MemShaper {
    pub ks: Vec<K>,
    pub pts: Option<PartingSeq>,
    pub sks: Option<StrokeSeq>,
    pub sgs: Option<SegmentSeq>,
}

impl MemShaper {
    fn new() -> Self {
        MemShaper{
            ks: Vec::new(),
            pts: None,
            sks: None,
            sgs: None,
        }
    }
}

impl KShaper for MemShaper {
    fn init_ks(&mut self, ks: Vec<K>) -> Result<()> {
        if ks.is_empty() {
            return Err(Error("empty vec to init KShaper".to_owned()));
        }
        self.ks = ks;
        Ok(())
    }

    fn ks(&self) -> &[K] {
        &self.ks
    }
}

impl PartingShaper for MemShaper {
    fn init_pts(&mut self) -> Result<()> {
        if self.ks.is_empty() {
            return Err(Error("no k lines to init partings".to_owned()));
        }
        let mut pts = Vec::new();
        let mut first_k = None;
        let mut second_k = None;
        let mut third_k = None;
        let mut upward = true;
        
        for k in self.ks.iter() {
            // k1不存在
            if first_k.is_none() {
                first_k = Some(k_to_ck(k, 0));
                continue;
            }
            // k1存在
            let k1 = first_k.as_ref().unwrap();

            // k2不存在
            if second_k.is_none() {
                // 检查k1与k的包含关系
                match inclusive_neighbor_k(k1, k, upward) {
                    None => {
                        // 更新k2
                        second_k = Some(k_to_ck(k, k1.indep_k + 1));
                        upward = k.high > k1.high;
                        continue;
                    }
                    ck => {
                        // 合并k1与k
                        first_k = ck;
                        continue;
                    }
                    
                }
            }

            // k2存在
            let k2 = second_k.as_ref().unwrap();

            // k3不存在
            if third_k.is_none() {
                // 检查k2与k的包含关系
                let ck = inclusive_neighbor_k(k2, k, upward);
                if ck.is_some() {
                    // 更新k2
                    second_k = ck;
                    continue;
                }
                // 检查k1, k2与k是否形成顶/底分型
                if upward && k.low < k2.low {
                    // 形成顶分型，更新k2和k3
                    second_k.as_mut().unwrap().indep_k = 0;
                    third_k = Some(k_to_ck(k, 0));
                    upward = false;
                    continue;
                }

                if !upward && k.high > k2.high {
                    // 形成底分型，更新k2和k3
                    second_k.as_mut().unwrap().indep_k = 0;
                    third_k = Some(k_to_ck(k, 0));
                    upward = true;
                    continue;
                }

                // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变，独立K线数不变
                let indep_k = k2.indep_k + 1;
                first_k = second_k.take();
                second_k = Some(k_to_ck(k, indep_k));
                continue;
            }

            let k3 = third_k.as_ref().unwrap();

            // 检查k3与k的包含关系
            let ck = inclusive_neighbor_k(k3, k, upward);
            if ck.is_some() {
                // 更新k3
                third_k = ck;
                continue;
            }

            //不包含，需构建分型并记录
            let parting = Parting{
                start_ts: k1.start_ts.clone(),
                end_ts: k3.end_ts.clone(),
                extremum_ts: k2.extremum_ts.clone(),
                extremum_price: if upward { k2.low } else { k2.high },
                n: k1.n + k2.n + k3.n,
                top: !upward,
                indep_k: k1.indep_k,
            };
            pts.push(parting);
            // 将k3平移到k1, 将k平移到k2，调整方向
            second_k = Some(k_to_ck(k, 1));
            upward = k.high > k3.high;
            first_k = third_k.take();
            first_k.as_mut().unwrap().indep_k = 0;
            continue;
        }
        
        if third_k.is_some() {
            let k1 = first_k.take().unwrap();
            let k2 = second_k.take().unwrap();
            let k3 = third_k.take().unwrap();

            let parting = Parting{
                start_ts: k1.start_ts,
                end_ts: k3.end_ts,
                extremum_ts: k2.extremum_ts,
                extremum_price: if upward { k2.low } else { k2.high },
                n: k1.n + k2.n + k3.n,
                top: !upward,
                indep_k: k1.indep_k,
            };
            pts.push(parting);
        }
        
        let mut tail = vec![];

        if first_k.is_some() {
            tail.push(first_k.unwrap());
        }
        if second_k.is_some() {
            tail.push(second_k.unwrap());
        }
        self.pts = Some(PartingSeq{pts, tail});
        Ok(())
    }

    fn parting_seq(&self) -> Option<&PartingSeq> {
        self.pts.as_ref()
    }
}

fn k_to_ck(k: &K, indep_k: i32) -> CK {
    CK{start_ts: k.ts.clone(), end_ts: k.ts.clone(), extremum_ts: k.ts.clone(), 
        high: k.high, low: k.low, n: 1, indep_k}
}

fn inclusive_neighbor_k(k1: &CK, k2: &K, upward: bool) -> Option<CK> {
    let extremum_ts = if k1.high >= k2.high && k1.low <= k2.low {
        k1.extremum_ts.clone()
    } else if k2.high >= k1.high && k2.low <= k1.low {
        k2.ts.clone()
    } else {
        return None;
    };

    let start_ts = k1.start_ts.clone();
    let end_ts = k2.ts.clone();
    let n = k1.n + 1;
    let indep_k = k1.indep_k;

    let (high, low) = if upward {
        (if k1.high > k2.high {k1.high} else {k2.high}, if k1.low > k2.low {k1.low} else {k2.low})
    } else {
        (if k1.high < k2.high {k1.high} else {k2.high}, if k1.low < k2.low {k1.low} else {k2.low})
    };
    Some(CK{start_ts, end_ts, extremum_ts, high, low, n, indep_k})
}





#[cfg(test)]
mod tests {
    use super::*;
    use crate::K;
    #[test]
    fn test_shaper_no_parting() -> Result<()> {
        let shaper = init_shaper(vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.25, 10.15),
            new_k("2020-02-01 10:04:00", 10.30, 10.20),
        ])?;
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        let r = shaper.parting_seq().unwrap();
        assert_eq!(0, r.pts.len());
        assert_eq!(2, r.tail.len());
        assert_eq!("2020-02-01 10:03:00", &r.tail[0].start_ts);
        assert_eq!("2020-02-01 10:04:00", &r.tail[1].start_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting() -> Result<()> {
        let shaper = init_shaper(vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.10, 10.00),
        ])?;
        let r = shaper.parting_seq().unwrap();
        assert_eq!(1, r.pts.len());
        assert_eq!(2, r.tail.len());
        assert_eq!("2020-02-01 10:01:00", &r.pts[0].start_ts);
        assert_eq!("2020-02-01 10:03:00", &r.pts[0].end_ts);
        assert_eq!("2020-02-01 10:02:00", &r.pts[0].extremum_ts);
        assert_eq!(10.20, r.pts[0].extremum_price);
        assert_eq!(true, r.pts[0].top);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting_inclusive() -> Result<()> {
        let shaper = init_shaper(vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.20, 10.00),
        ])?;
        let r = shaper.parting_seq().unwrap();
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        // panic!(json);
        assert_eq!(1, r.pts.len());
        assert_eq!(0, r.tail.len());
        assert_eq!("2020-02-01 10:04:00", &r.pts[0].end_ts);
        Ok(())
    }

    fn new_k(ts: &str, high: f64, low: f64) -> K {
        K {ts: ts.to_owned(), high, low }
    }

    fn init_shaper(input: Vec<K>) -> Result<MemShaper> {
        let mut shaper = MemShaper::new();
        shaper.init_ks(input)?;
        shaper.init_pts()?;
        Ok(shaper)
    }

}