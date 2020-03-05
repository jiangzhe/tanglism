use crate::errors::ApiError;
use actix_web::body::Body;
use actix_web::web::Json;
use actix_web::HttpResponse;
use serde::Serialize;

pub fn respond_json<T>(data: T) -> Result<Json<T>, ApiError>
where
    T: Serialize,
{
    Ok(Json(data))
}

pub fn respond_ok() -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().body(Body::Empty))
}
