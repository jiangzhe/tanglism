use derive_more::Display;

#[derive(Debug, Display, PartialEq)]
#[allow(dead_code)]
pub enum ApiError {
    BadRequest(String),
    InternalServerError(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<String>,
}