use actix_web::web::Json;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn get_health() -> Result<