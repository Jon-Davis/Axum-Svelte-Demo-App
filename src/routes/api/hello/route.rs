use axum::{response::IntoResponse, Json};
use serde::Serialize;
use typeshare::typeshare;

#[typeshare]
#[derive(Serialize)]
pub struct HelloResponse {
    pub message: String,
}

pub async fn get() -> impl IntoResponse {
    Json(HelloResponse {
        message: "Hello from Rust!".to_string(),
    })
}
