use axum::{Json, extract::Query};
use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct HelloParams {
    /// Name to echo back in the greeting.
    pub name: Option<String>,
}

#[typeshare]
#[derive(Serialize, utoipa::ToSchema)]
pub struct HelloResponse {
    pub message: String,
}

pub async fn get(Query(params): Query<HelloParams>) -> Json<HelloResponse> {
    let message = match params.name {
        Some(name) => format!("Hello, {name}!"),
        None => "Hello from Rust!".to_string(),
    };
    Json(HelloResponse { message })
}
