mod error;
mod shape;
mod trend;
mod segment;
mod parting;
mod stroke;

use chrono::NaiveDateTime;
pub use error::Error;
use serde_derive::*;
pub type Result<T> = std::result::Result<T, Error>;
