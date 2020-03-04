use actix_web::web::Json;
use crate::helpers::respond_json;
use serde_derive::*;
use crate::errors::ApiError;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn get_health() -> Result<Json<HealthResponse>, ApiError> {
    respond_json(HealthResponse{
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}
