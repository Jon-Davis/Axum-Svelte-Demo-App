use axum::{response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

pub async fn get() -> impl IntoResponse {
    Json(HelloResponse {
        message: "Hello from Rust!".to_string(),
    })
}
