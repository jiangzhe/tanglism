//! 流式处理
//!
//! 缠论增量处理

use crate::Result;

/// 累加器
///
/// 累加器提供增量更新API
pub trait Accumulator<T> {
    type Delta;
    type State: Clone;

    fn accumulate(&mut self, item: &T) -> Result<Self::Delta>;

    fn state(&self) -> &Self::State;
}

/// 聚合器
///
/// 聚合器提供批量聚合API
/// 可支持多种I/O类型：结果型和日志型
pub trait Aggregator<I, O> {
    fn aggregate(self, input: I) -> Result<O>;
}

/// 复制器
///
/// 提供根据Delta进行复制状态的能力
pub trait Replicator {
    type Delta;
    type State: Clone;

    fn replicate(&mut self, delta: Self::Delta) -> Result<()>;

    fn state(&self) -> &Self::State;
}

#[derive(Debug, Clone)]
pub enum Delta<T> {
    None,
    Add(T),
    Update(T),
    Delete(T),
}

impl<T> Delta<T> {
    pub fn none(&self) -> bool {
        match self {
            Delta::None => true,
            _ => false,
        }
    }

    pub fn add(&self) -> Option<&T> {
        match self {
            Delta::Add(add) => Some(add),
            _ => None,
        }
    }

    pub fn update(&self) -> Option<&T> {
        match self {
            Delta::Update(update) => Some(update),
            _ => None,
        }
    }

    pub fn delete(&self) -> Option<&T> {
        match self {
            Delta::Delete(delete) => Some(delete),
            _ => None,
        }
    }
}
