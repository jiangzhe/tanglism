// use actix_web::error::ResponseError;
// use actix_web::HttpResponse;
use derive_more::Display;
use serde_derive::*;
use std::fmt;

/// the error type for web server
#[derive(Debug, Clone)]
pub enum Error {
    Simple(ErrorKind),
    Custom(ErrorKind, String),
}

impl Error {
    // construct simple error
    pub fn simple(kind: ErrorKind) -> Error {
        Error::Simple(kind)
    }

    // construct custom error with description
    pub fn custom(kind: ErrorKind, err: String) -> Error {
        Error::Custom(kind, err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Simple(kind) => write!(fmt, "{}", kind),
            Error::Custom(kind, s) => write!(fmt, "{}: {}", kind, s),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Display, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    BadRequest,
    NotFound,
    InternalServerError,
    IO,
    Diesel,
    Jqdata,
    DbConn,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::custom(ErrorKind::IO, err.to_string())
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Error {
        Error::custom(ErrorKind::Diesel, err.to_string())
    }
}

impl From<jqdata::Error> for Error {
    fn from(err: jqdata::Error) -> Error {
        Error::custom(ErrorKind::Jqdata, err.to_string())
    }
}

impl From<tanglism_utils::Error> for Error {
    fn from(err: tanglism_utils::Error) -> Error {
        Error::custom(ErrorKind::InternalServerError, err.to_string())
    }
}

impl From<r2d2::Error> for Error {
    fn from(err: r2d2::Error) -> Error {
        Error::custom(ErrorKind::DbConn, err.to_string())
    }
}

impl From<tanglism_morph::Error> for Error {
    fn from(err: tanglism_morph::Error) -> Error {
        Error::custom(ErrorKind::InternalServerError, err.to_string())
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Error {
        Error::custom(ErrorKind::InternalServerError, err.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<String>,
}

impl From<&str> for ErrorResponse {
    fn from(err: &str) -> Self {
        ErrorResponse {
            errors: vec![err.into()],
        }
    }
}

impl From<String> for ErrorResponse {
    fn from(err: String) -> Self {
        ErrorResponse { errors: vec![err] }
    }
}

impl From<Vec<String>> for ErrorResponse {
    fn from(errors: Vec<String>) -> Self {
        ErrorResponse { errors }
    }
}

impl warp::reject::Reject for Error {}
