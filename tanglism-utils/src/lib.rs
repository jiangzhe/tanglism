mod error;
pub mod trading_date;
pub mod trading_timestamp;

#[macro_use]
extern crate lazy_static;

// pub use datetime::*;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use trading_date::LOCAL_TRADING_DATE_BITMAP;
pub use trading_timestamp::{LOCAL_TRADING_TS_1_MIN, LOCAL_TRADING_TS_5_MIN, LOCAL_TRADING_TS_30_MIN};
