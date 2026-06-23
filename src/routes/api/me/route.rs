use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::Principal;

#[derive(Serialize, utoipa::ToSchema)]
pub struct UserInfo {
    pub email: Option<String>,
    pub username: Option<String>,
    pub role: String,
}

/// Return the authenticated caller's profile (email, username, role).
pub async fn get(Extension(principal): Extension<Principal>) -> Json<UserInfo> {
    Json(UserInfo {
        email: principal.email,
        username: principal.username,
        role: principal.role,
    })
}
