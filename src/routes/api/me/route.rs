use axum::{response::IntoResponse, Extension, Json};
use serde::Serialize;

use crate::auth::Principal;

#[derive(Serialize)]
struct UserInfo {
    email: Option<String>,
    username: Option<String>,
    role: String,
}

// The `Principal` was already resolved by `require_login` (from the session
// cookie or a Bearer API key), so there is nothing to re-query here. Service
// accounts authenticate by API key and report a `role` with no email/username.
pub async fn get(Extension(principal): Extension<Principal>) -> impl IntoResponse {
    Json(UserInfo {
        email: principal.email,
        username: principal.username,
        role: principal.role,
    })
    .into_response()
}
