use axum::{Json, extract::Query};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct HelloParams {
    /// Name to echo back in the greeting.
    pub name: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HelloResponse {
    pub message: String,
}

/// Return a greeting, optionally personalised with the `name` query parameter.
pub async fn get(Query(params): Query<HelloParams>) -> Json<HelloResponse> {
    let message = match params.name {
        Some(name) => format!("Hello, {name}!"),
        None => "Hello from Rust!".to_string(),
    };
    Json(HelloResponse { message })
}
