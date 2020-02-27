use crate::{Result, Error, K, CK, Parting, Stroke, Segment, Center, Trend};

/// 单标的确定周期的数据来源
pub trait Source {
    // 给定代码和时刻，获取该时刻前的不多于limit条数的K线数据
    fn data_before(&self, ts: &str, limit: u32) -> Vec<K>;

    // 给定代码和时刻，获取该时刻后的不多余limit条数的K线数据
    fn data_after(&self, ts: &str, limit: u32) -> Vec<K>;
}

/// 潜在分型
/// 
/// 在形态合成过程中，可能出现潜在分型，该分型符合分型定义，单并不一定可以跟相邻分型形成笔
pub struct PotentialParting {
    pub op: Option<Parting>,
    pub neighbor_too_close: bool,
}

/// 潜在分型序列
///
/// 包含潜在分型的序列，以及未能形成分型的尾部K线
/// 可通过输入最新K线，更新或延长已有序列
pub struct PotentialPartingSeq {
    pub pts: Vec<PotentialParting>,
    pub tail: Vec<K>,
}

/// 笔序列
/// 
/// 包含笔序列，以及未形成笔的尾部分型
pub struct StrokeSeq {
    pub sks: Vec<Stroke>,
    pub tail: Vec<PotentialParting>,
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
    fn parting_seq(&self) -> Option<&PotentialPartingSeq>;
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
    pub pts: Option<PotentialPartingSeq>,
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
        let mut pts = PotentialPartingSeq{pts: Vec::new(), tail: Vec::new()};
        let mut first_k = None;
        let mut second_k = None;
        let mut third_k = None;
        let mut upward = true;
        
        for k in self.ks.iter() {
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
                if upward && k.low < k2.low {
                    // 形成顶分型，更新k3
                    third_k = Some(k_to_ck(k));
                    upward = false;
                    continue;
                }

                if !upward && k.high > k2.high {
                    // 形成底分型，更新k3
                    third_k = Some(k_to_ck(k));
                    upward = true;
                    continue;
                }

                // 不形成顶/底分型时，将k1, k2, k平移一位，上升/下降方向不变
                first_k = second_k.take();
                second_k = Some(k_to_ck(k));
                continue;
            }

            // todo: 记录顶底分型中的独立K线
            // todo: 存储顶底分型


            

            
        }
        Ok(())
    }

    fn parting_seq(&self) -> Option<&PotentialPartingSeq> {
        self.pts.as_ref()
    }
}

fn k_to_ck(k: &K) -> CK {
    CK{start_ts: k.ts.clone(), end_ts: k.ts.clone(), extremum_ts: k.ts.clone(), high: k.high, low: k.low, n: 1}
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

    let (high, low) = if upward {
        (if k1.high > k2.high {k1.high} else {k2.high}, if k1.low > k2.low {k1.low} else {k2.low})
    } else {
        (if k1.high < k2.high {k1.high} else {k2.high}, if k1.low < k2.low {k1.low} else {k2.low})
    };
    Some(CK{start_ts, end_ts, extremum_ts, high, low, n})
}