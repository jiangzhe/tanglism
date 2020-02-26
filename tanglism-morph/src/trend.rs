use crate::shape::{Shaper, Appender};
use crate::{Result, Error};

pub trait Analyzer {
    fn init<S: Shaper>(&mut self, shaper: S) -> Result<()>;


}