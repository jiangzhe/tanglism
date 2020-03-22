use crate::Result;
use crate::shape::{K, CK, Parting, PartingSeq};

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
        PartingShaper {
            ks,
            body: Vec::new(),
            first_k: None,
            second_k: None,
            third_k: None,
            upward: true,
        }
    }

    fn consume(&mut self, k: K) {
        // k1不存在
        if self.first_k.is_none() {
            self.first_k = Some(Self::k_to_ck(k));
            return;
        }
        // k1存在
        let k1 = self.first_k.unwrap();

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
        let k2 = self.second_k.unwrap();

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

        let k3 = self.third_k.unwrap();

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
        self.first_k = Some(k3);
        self.second_k = Some(Self::k_to_ck(k));
        self.third_k = None;
    }

    fn run(mut self) -> Result<PartingSeq> {
        for k in self.ks.iter() {
            self.consume(*k);
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
        Ok(PartingSeq {
            body: self.body,
            tail,
        })
    }

    /// 辅助函数，将单个K线转化为合并K线
    #[inline]
    fn k_to_ck(k: K) -> CK {
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
    fn inclusive_neighbor_k(k1: CK, k2: K, upward: bool) -> Option<CK> {
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


#[cfg(test)]
mod tests {
    use super::*;
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
