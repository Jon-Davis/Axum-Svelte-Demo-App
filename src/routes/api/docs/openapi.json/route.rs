use axum::{http::header, response::IntoResponse};
use std::sync::OnceLock;

static JSON: OnceLock<String> = OnceLock::new();

pub async fn get() -> impl IntoResponse {
    let body = JSON.get_or_init(|| {
        crate::openapi_document()
            .to_pretty_json()
            .expect("serialize OpenAPI document")
    });
    ([(header::CONTENT_TYPE, "application/json")], body.as_str())
}
