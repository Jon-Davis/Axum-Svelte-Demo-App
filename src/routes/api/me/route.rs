use axum::{response::IntoResponse, Extension, Json};
use serde::Serialize;

use crate::auth::Principal;

#[derive(Serialize)]
struct UserInfo {
    email: Option<String>,
    username: Option<String>,
    role: String,
}

pub async fn get(Extension(principal): Extension<Principal>) -> impl IntoResponse {
    Json(UserInfo {
        email: principal.email,
        username: principal.username,
        role: principal.role,
    })
    .into_response()
}
