use crate::{Error, Parting, Result, Segment, Stroke, CK, K};
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PartingSeq {
    pub body: Vec<Parting>,
    pub tail: Vec<CK>,
}

impl PartingSeq {
    fn new() -> Self {
        PartingSeq{
            body: Vec::new(),
            tail: Vec::new(),
        }
    }
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

impl StrokeSeq {
    fn new() -> Self {
        StrokeSeq{
            body: Vec::new(),
            tail: None,
        }
    }
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

/// 将K线图解析为分型序列
pub fn ks_to_pts(ks: &[K]) -> Result<PartingSeq> {
    let mut body = Vec::new();
    let mut first_k = None;
    let mut second_k = None;
    let mut third_k = None;
    let mut upward = true;

    for k in ks.iter() {
        // k1不存在
        if first_k.is_none() {
            first_k = Some(k_to_ck(k));
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
                    second_k = Some(k_to_ck(k));
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
            if (upward && k.low < k2.low) || (!upward && k.high > k2.high) {
                // 形成顶/底分型，更新k2和k3，并将走势颠倒
                third_k = Some(k_to_ck(k));
                upward = !upward;
                continue;
            }

            // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变
            first_k = second_k.take();
            second_k = Some(k_to_ck(k));
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
        let parting = Parting {
            start_ts: k1.start_ts,
            end_ts: k3.end_ts,
            extremum_ts: k2.extremum_ts,
            extremum_price: if upward { k2.low } else { k2.high },
            n: k1.n + k2.n + k3.n,
            top: !upward,
        };
        body.push(parting);

        // 当k2, k3, k形成顶底分型时，左移1位
        if (upward && k.low < k3.low) || (!upward && k.high > k3.high) {
            first_k = second_k.take();
            second_k = third_k.take();
            third_k = Some(k_to_ck(k));
            upward = !upward;
            continue;
        }

        // 不形成分型时，将k3, k向左移两位
        upward = k.high > k3.high;
        first_k = Some(k3.clone());
        second_k = Some(k_to_ck(k));
        third_k = None;
    }

    // 结束所有k线分析后，依然存在第三根K线，说明此时三根K线刚好构成顶底分型
    if third_k.is_some() {
        let k1 = first_k.take().unwrap();
        let k2 = second_k.take().unwrap();
        let k3 = third_k.take().unwrap();

        let parting = Parting {
            start_ts: k1.start_ts,
            end_ts: k3.end_ts,
            extremum_ts: k2.extremum_ts,
            extremum_price: if upward { k2.low } else { k2.high },
            n: k1.n + k2.n + k3.n,
            top: !upward,
        };
        body.push(parting);
        // 向左平移k2和k3
        first_k = Some(k2);
        second_k = Some(k3);
    }

    let mut tail = vec![];
    // 将剩余k线加入尾部，必定不会出现三根K线
    if let Some(fk) = first_k {
        tail.push(fk);
    }
    if let Some(sk) = second_k {
        tail.push(sk);
    }
    Ok(PartingSeq { body, tail })
}

/// 将分型序列解析为笔序列
/// 
/// 步骤：
/// 1. 选择起始点。
/// 2. 选择下一个点。
///    若异型：邻接或交叉则忽略，不邻接则成笔
///    若同型：顶更高/底更低则修改当前笔，反之则忽略
pub fn pts_to_sks(pts: &PartingSeq, unit: String) -> Result<StrokeSeq> {
    if pts.body.is_empty() {
        return Ok(StrokeSeq{
            body: Vec::new(),
            tail: Some(pts.clone()),
        });
    }
    // todo
    // let mut pts_iter = pts.body.iter();
    // // 笔序列
    // let mut sks = Vec::new();
    // // 暂存忽略点序列
    // let mut ignored = Vec::new();
    // // 起点
    // let mut start = pts_iter.next().unwrap().clone();
    // while let Some(pt) = pts_iter.next() {
    //     if pt.top != start.top {  // 异型
    //         // todo
    //     } else {  // 同型

    //     }
    // }

    unimplemented!()
}

/// 将笔序列解析为线段序列
pub fn sks_to_sgs(sks: &StrokeSeq) -> Result<SegmentSeq> {
    unimplemented!()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::K;
    use chrono::NaiveDateTime;
    #[test]
    fn test_shaper_no_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.25, 10.15),
            new_k("2020-02-01 10:04:00", 10.30, 10.20),
        ];
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        let r = ks_to_pts(&ks)?;
        assert_eq!(0, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:03:00"), r.tail[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04:00"), r.tail[1].start_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.10, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:01:00"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03:00"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:02:00"), r.body[0].extremum_ts);
        assert_eq!(10.20, r.body[0].extremum_price);
        assert_eq!(true, r.body[0].top);
        Ok(())
    }

    #[test]
    fn test_shaper_one_parting_inclusive() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.20, 10.00),
        ];
        let r = ks_to_pts(&ks)?;
        // let json = serde_json::to_string_pretty(&shaper.parting_seq())?;
        // panic!(json);
        assert_eq!(1, r.body.len());
        assert_eq!(2, r.tail.len());
        assert_eq!(new_ts("2020-02-01 10:04:00"), r.body[0].end_ts);
        Ok(())
    }

    #[test]
    fn test_shaper_two_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.20, 10.10),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01:00"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03:00"), r.body[0].end_ts);
        assert_eq!(true, r.body[0].top);
        assert_eq!(new_ts("2020-02-01 10:02:00"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:04:00"), r.body[1].end_ts);
        assert_eq!(false, r.body[1].top);
        assert_eq!(2, r.tail.len());
        Ok(())
    }

    #[test]
    fn test_shaper_two_indep_partings() -> Result<()> {
        let ks = vec![
            new_k("2020-02-01 10:00:00", 10.10, 10.00),
            new_k("2020-02-01 10:01:00", 10.15, 10.05),
            new_k("2020-02-01 10:02:00", 10.20, 10.10),
            new_k("2020-02-01 10:03:00", 10.15, 10.05),
            new_k("2020-02-01 10:04:00", 10.10, 10.00),
            new_k("2020-02-01 10:05:00", 10.05, 9.95),
            new_k("2020-02-01 10:06:00", 10.00, 9.90),
            new_k("2020-02-01 10:07:00", 10.05, 9.95),
        ];
        let r = ks_to_pts(&ks)?;
        assert_eq!(2, r.body.len());
        assert_eq!(new_ts("2020-02-01 10:01:00"), r.body[0].start_ts);
        assert_eq!(new_ts("2020-02-01 10:03:00"), r.body[0].end_ts);
        assert_eq!(new_ts("2020-02-01 10:05:00"), r.body[1].start_ts);
        assert_eq!(new_ts("2020-02-01 10:07:00"), r.body[1].end_ts);
        Ok(())
    }

    fn new_k(ts: &str, high: f64, low: f64) -> K {
        K {
            ts: new_ts(ts),
            high,
            low,
        }
    }

    fn new_ts(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }
}
