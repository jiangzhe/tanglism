pub mod error;
pub mod cli;

pub use cli::JqdataClient;
pub use error::Error;

use std::fmt;
use std::str::FromStr;
use serde::{Serializer, Serialize, Deserializer, Deserialize};
use serde::de::{self, Visitor};

pub enum Unit {
    U1m,
    U5m,
    U15m,
    U30m,
    U60m,
    U120m,
    U1d,
    U1w,
    // 1M is not included for simplicity
    // U1M,
}

impl Unit {
    fn to_seconds(&self) -> u64 {
        match *self {
            Unit::U1m => 60,
            Unit::U5m => 300,
            Unit::U15m => 900,
            Unit::U30m => 1800,
            Unit::U60m => 3600,
            Unit::U120m => 7200,
            Unit::U1d => 86400,
            Unit::U1w => 86400 * 7,
        }
    }

    fn to_millis(&self) -> u64 {
        self.to_seconds() * 1000
    }
}

impl Serialize for Unit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> 
    where
        S: Serializer, 
    {
        match *self {
            Unit::U1m => serializer.serialize_str("1m"),
            Unit::U5m => serializer.serialize_str("5m"),
            Unit::U15m => serializer.serialize_str("15m"),
            Unit::U30m => serializer.serialize_str("30m"),
            Unit::U60m => serializer.serialize_str("60m"),
            Unit::U120m => serializer.serialize_str("120m"),
            Unit::U1d => serializer.serialize_str("1d"),
            Unit::U1w => serializer.serialize_str("1w"),
        }
    }
}

struct UnitVisitor;

impl<'de> Visitor<'de> for UnitVisitor {
    type Value = Unit;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string of any: 1m, 5m, 15m, 30m, 60m, 120m, 1d, 1w")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match s {
            "1m" => Ok(Unit::U1m),
            "5m" => Ok(Unit::U5m),
            "15m" => Ok(Unit::U15m),
            "30m" => Ok(Unit::U30m),
            "60m" => Ok(Unit::U60m),
            "120m" => Ok(Unit::U120m),
            "1d" => Ok(Unit::U1d),
            "1w" => Ok(Unit::U1w),
            // "1M" => Ok(Unit::U1M),
            _ => Err(E::custom(format!("invalid unit: {}", s))),
        }
    }
}

impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D>(deserializer: D) -> Result<Unit, D::Error>
    where
        D: Deserializer<'de>
    {
        deserializer.deserialize_str(UnitVisitor)
    }
}

/// enable parse string to unit
impl FromStr for Unit {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1m" => Ok(Unit::U1m),
            "5m" => Ok(Unit::U5m),
            "15m" => Ok(Unit::U15m),
            "30m" => Ok(Unit::U30m),
            "60m" => Ok(Unit::U60m),
            "120m" => Ok(Unit::U120m),
            "1d" => Ok(Unit::U1d),
            "1w" => Ok(Unit::U1w),
            // "1M" => Ok(Unit::U1M),
            _ => Err(Error::Client(format!("invalid unit: {}", s))),
        }
    }
}
