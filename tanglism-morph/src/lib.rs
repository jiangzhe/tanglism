mod center;
mod error;
mod parting;
mod segment;
mod shape;
mod stroke;
mod subtrend;
mod trend;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
pub use center::*;
pub use parting::ks_to_pts;
pub use segment::sks_to_sgs;
pub use shape::*;
pub use stroke::*;
pub use subtrend::*;
pub use trend::*;
