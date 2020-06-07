mod error;
mod parting;
mod segment;
mod shape;
mod stroke;
pub mod trend;
pub mod center;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
pub use parting::ks_to_pts;
pub use segment::sks_to_sgs;
pub use shape::*;
pub use stroke::*;
pub use trend::*;
pub use center::*;