use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use derive_more::Display;
use serde_derive::*;
use std::fmt;

/// the error type for web server
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}", self)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error(format!("IO error: {}", err))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Error {
        Error(format!("SQL error: {}", err))
    }
}

#[derive(Debug, Display, PartialEq)]
#[allow(dead_code)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    InternalServerError(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<String>,
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::BadRequest(err) => {
                HttpResponse::BadRequest().json::<ErrorResponse>(err.into())
            }
            ApiError::NotFound(msg) => HttpResponse::NotFound().json::<ErrorResponse>(msg.into()),
            _ => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl From<&String> for ErrorResponse {
    fn from(err: &String) -> Self {
        ErrorResponse {
            errors: vec![err.into()],
        }
    }
}

impl From<Vec<String>> for ErrorResponse {
    fn from(errors: Vec<String>) -> Self {
        ErrorResponse { errors }
    }
}
