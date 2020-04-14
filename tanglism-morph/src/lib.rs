mod error;
mod shape;
mod trend;
mod segment;
mod parting;
mod stroke;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
pub use shape::*;
pub use parting::ks_to_pts;
pub use stroke::{pts_to_sks, StrokeShaper, StrokeConfig};
pub use segment::sks_to_sgs;
