use std::fmt;
use serde::de;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    Server(String),
    Client(String),
    Serde(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Reqwest(ref err) => write!(f, "Reqwest error: {}", err),
            Error::Server(ref s) => write!(f, "Server error: {}", s),
            Error::Client(ref s) => write!(f, "Client error: {}", s),
            Error::Serde(ref s) => write!(f, "Serde error: {}", s),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Reqwest(ref err) => err.description(),
            Error::Server(ref s) => s, 
            Error::Client(ref s) => s, 
            Error::Serde(ref s) => s,
        }
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            Error::Reqwest(ref err) => Some(err),
            Error::Server(..) => None,
            Error::Client(..) => None,
            Error::Serde(..) => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::Reqwest(err)
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self 
    where T: fmt::Display,
    {
        Error::Serde(format!("{}", msg))
    }
}