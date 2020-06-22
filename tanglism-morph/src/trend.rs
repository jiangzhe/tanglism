//! 走势类型
//!
//! 缠论的基础概念
//!
//! 分为趋势和盘整
//! 趋势由至少2个没有价格区间重叠的中枢构成，趋势向上则为上涨，趋势向下则为下跌
//! 盘整由1个中枢构成
//!
//! 走势分解定理一：任何级别的任何走势，都可以分解成同级别盘整、下跌与上涨三种走势类型的连接。
//! 走势分解定理二：任何级别的任何走势类型，都至少由三段以上次级别走势类型构成。
//!
//! 目前的实现是直接使用次级别段作为次级别走势，而次级别笔作为次级别以下走势。

use crate::align_tick;
use crate::shape::{Center, CenterElement, SubTrend, SubTrendType, Trend, ValuePoint};
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct TrendConfig {
    pub level: i32,
}

pub fn unify_trends(centers: &[CenterElement]) -> Vec<Trend> {
    Standard::new().aggregate(centers)
}

trait TrendStrategy {
    fn aggregate(self, centers: &[CenterElement]) -> Vec<Trend>;
}

struct Standard {
    tmp: Vec<TemporaryTrend>,
}

impl TrendStrategy for Standard {
    fn aggregate(mut self, centers: &[CenterElement]) -> Vec<Trend> {
        for idx in 0..centers.len() {
            self.accmulate(centers, idx);
        }
        self.trends(centers)
    }
}

/// 中枢生成走势算法
impl Standard {
    fn new() -> Self {
        Standard { tmp: Vec::new() }
    }

    fn accmulate(&mut self, centers: &[CenterElement], idx: usize) {
        let ce = &centers[idx];
        if self.tmp.is_empty() {
            let (centers, last_center) = if let Some(c) = ce.center() {
                (1, Some(Box::new(c.clone())))
            } else {
                (0, None)
            };
            let start = ce.start().clone();
            self.push_pending(TemporaryPending {
                end_idx: 0,
                centers,
                last_center,
                start,
                upward: None,
                level: ce.level(),
            });
            return;
        }
        match self.last().unwrap() {
            TemporaryTrend::Pending(p) => {
                // 未完成的走势
                match &centers[idx] {
                    CenterElement::Center(c) => {
                        if p.centers == 0 {
                            // 走势没有中枢，合并进入走势
                            let c = c.clone();
                            self.update_pending(move |p| {
                                p.end_idx = idx;
                                p.centers += 1;
                                p.last_center.replace(Box::new(c));
                            });
                        } else if let Some(upward) = p.upward {
                            // 走势存在中枢，且方向固定
                            let last_center = p
                                .last_center
                                .as_ref()
                                .cloned()
                                .expect("last center in pending trend");
                            if (upward && c.shared_low.value > last_center.shared_high.value)
                                || (!upward && c.shared_high.value < last_center.shared_low.value)
                            {
                                // 向上同向或向下同向
                                let new_center = c.clone();
                                self.update_pending(move |p| {
                                    p.end_idx = idx;
                                    p.centers += 1;
                                    p.last_center.replace(Box::new(new_center));
                                });
                            } else if upward {
                                // 向上走势终结
                                // 寻找最高点作为向上走势结束点
                                let end = if last_center.high.value > c.start.value {
                                    last_center.high
                                } else {
                                    c.start.clone()
                                };
                                let new_start = end.clone();
                                // 结束前走势
                                self.complete_pending(move |p| TemporaryCompleted {
                                    start: p.start,
                                    end,
                                    centers: p.centers,
                                    level: std::cmp::max(p.level, c.level),
                                });
                                // 开始新走势
                                self.push_pending(TemporaryPending {
                                    end_idx: idx,
                                    start: new_start,
                                    centers: 1,
                                    last_center: Some(Box::new(c.clone())),
                                    upward: Some(false),
                                    level: c.level,
                                });
                            } else {
                                // 向下走势终结
                                let end = if last_center.low.value < c.start.value {
                                    last_center.low
                                } else {
                                    c.start.clone()
                                };
                                let new_start = end.clone();
                                // 结束前走势
                                self.complete_pending(move |p| TemporaryCompleted {
                                    start: p.start,
                                    end,
                                    centers: p.centers,
                                    level: std::cmp::max(p.level, c.level),
                                });
                                // 开始新走势
                                self.push_pending(TemporaryPending {
                                    end_idx: idx,
                                    start: new_start,
                                    centers: 1,
                                    last_center: Some(Box::new(c.clone())),
                                    upward: Some(true),
                                    level: c.level,
                                });
                            }
                        } else {
                            // 走势存在中枢，方向不固定
                            let last_center = p
                                .last_center
                                .as_ref()
                                .cloned()
                                .expect("last center in pending trend");
                            if c.shared_low.value > last_center.shared_high.value {
                                // 向上走势
                                let new_center = c.clone();
                                self.update_pending(move |p| {
                                    p.end_idx = idx;
                                    p.centers += 1;
                                    p.last_center.replace(Box::new(new_center));
                                    p.upward.replace(true);
                                });
                            } else if c.shared_high.value < last_center.shared_low.value {
                                // 向下走势
                                let new_center = c.clone();
                                self.update_pending(move |p| {
                                    p.end_idx = idx;
                                    p.centers += 1;
                                    p.last_center.replace(Box::new(new_center));
                                    p.upward.replace(false);
                                });
                            } else {
                                // 盘整，同级别切分为不同走势
                                let end = c.start.clone();
                                self.complete_pending(move |p| TemporaryCompleted {
                                    start: p.start,
                                    end,
                                    centers: p.centers,
                                    level: std::cmp::max(p.level, c.level),
                                });
                                self.push_pending(TemporaryPending {
                                    end_idx: idx,
                                    start: c.start.clone(),
                                    centers: 1,
                                    last_center: Some(Box::new(c.clone())),
                                    upward: None,
                                    level: c.level,
                                });
                            }
                        }
                    }
                    _ => {
                        self.update_pending(|p| {
                            p.end_idx = idx;
                        });
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn last(&self) -> Option<&TemporaryTrend> {
        self.tmp.last()
    }

    #[allow(dead_code)]
    fn remove_lastn(&mut self, n: usize) {
        for _ in 0..n {
            self.tmp.pop().unwrap();
        }
    }

    fn push_pending(&mut self, pending: TemporaryPending) {
        self.tmp.push(TemporaryTrend::Pending(pending));
    }

    fn update_pending<F>(&mut self, f: F)
    where
        F: FnOnce(&mut TemporaryPending),
    {
        if let Some(TemporaryTrend::Pending(pending)) = self.tmp.last_mut() {
            f(pending);
        }
    }

    fn complete_pending<F>(&mut self, f: F)
    where
        F: FnOnce(TemporaryPending) -> TemporaryCompleted,
    {
        if let Some(TemporaryTrend::Pending(pending)) = self.tmp.pop() {
            self.tmp.push(TemporaryTrend::Completed(f(pending)));
        }
    }

    fn trends(self, _centers: &[CenterElement]) -> Vec<Trend> {
        self.tmp
            .into_iter()
            .filter_map(|t| match t {
                TemporaryTrend::Pending(_) => None,
                TemporaryTrend::Completed(cp) => Some(Trend {
                    start: cp.start,
                    end: cp.end,
                    centers: cp.centers,
                    level: cp.level,
                }),
            })
            .collect()
    }
}

enum TemporaryTrend {
    Pending(TemporaryPending),
    Completed(TemporaryCompleted),
}

struct TemporaryPending {
    end_idx: usize,
    centers: usize,
    // 最后一个中枢
    last_center: Option<Box<Center>>,
    // 起始点，可能与start_idx不一致，是因为前一走势的极值点作为后一走势起点
    start: ValuePoint,
    // 方向固定向上或向下，None表示方向未固定
    upward: Option<bool>,
    level: i32,
}

struct TemporaryCompleted {
    // start_idx: usize,
    // end_idx: usize,
    start: ValuePoint,
    end: ValuePoint,
    centers: usize,
    level: i32,
}

pub fn trend_as_subtrend(trend: &Trend, tick: &str) -> Result<SubTrend> {
    Ok(SubTrend {
        start: ValuePoint {
            ts: align_tick(tick, trend.start.ts)?,
            value: trend.start.value.clone(),
        },
        end: ValuePoint {
            ts: align_tick(tick, trend.end.ts)?,
            value: trend.end.value.clone(),
        },
        level: trend.level + 1,
        typ: SubTrendType::Normal,
    })
}
