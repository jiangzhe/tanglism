use actix_web::error::ResponseError;
use actix_web::HttpResponse;
use derive_more::Display;
use serde_derive::*;
use std::fmt;

/// the error type for web server
#[derive(Debug)]
pub enum Error {
    Simple(ErrorKind),
    Custom(ErrorKind, String),
    Nested(Box<dyn std::error::Error + Send + Sync>),
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

    // construct nested error
    pub fn nested(err: Box<dyn std::error::Error + Send + Sync>) -> Error {
        Error::Nested(err)
    }

    // helper function to convert kind and error message into http response
    pub fn kind_to_response(kind: ErrorKind, err: &str) -> HttpResponse {
        match kind {
            ErrorKind::BadRequest => HttpResponse::BadRequest().json::<ErrorResponse>(err.into()),
            ErrorKind::NotFound => HttpResponse::NotFound().json::<ErrorResponse>(err.into()),
            ErrorKind::InternalServerError => {
                HttpResponse::InternalServerError().json::<ErrorResponse>(err.into())
            }
            ErrorKind::IO => HttpResponse::InternalServerError()
                .json::<ErrorResponse>((format!("IO Error: {}", err)).into()),
            ErrorKind::Diesel => HttpResponse::InternalServerError()
                .json::<ErrorResponse>(format!("Diesel Error: {}", err).into()),
            ErrorKind::Jqdata => HttpResponse::InternalServerError()
                .json::<ErrorResponse>(format!("Jqdata Error: {}", err).into()),
            ErrorKind::DbConn => HttpResponse::InternalServerError()
                .json::<ErrorResponse>(format!("DbConn Error: {}", err).into()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Simple(kind) => write!(fmt, "{}", kind),
            Error::Custom(kind, s) => write!(fmt, "{}: {}", kind, s),
            Error::Nested(err) => write!(fmt, "{}", err),
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

impl<E: std::fmt::Debug> From<actix_threadpool::BlockingError<E>> for Error {
    fn from(err: actix_threadpool::BlockingError<E>) -> Error {
        Error::custom(ErrorKind::InternalServerError, err.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<String>,
}

// implements ResponseError to allow converting error to response
impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        match self {
            Error::Simple(kind) => Self::kind_to_response(*kind, "no message"),
            Error::Custom(kind, err) => Self::kind_to_response(*kind, err),
            Error::Nested(err) => {
                Self::kind_to_response(ErrorKind::InternalServerError, &err.to_string())
            }
        }
    }
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
