use axum::{http::header, response::IntoResponse};

const JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json"));

pub async fn get() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json")], JSON)
}
