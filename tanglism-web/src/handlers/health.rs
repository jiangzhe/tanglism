use crate::errors::ApiError;
use crate::helpers::respond_json;
use actix_web::web::Json;
use serde_derive::*;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn get_health() -> Result<Json<HealthResponse>, ApiError> {
    respond_json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}
