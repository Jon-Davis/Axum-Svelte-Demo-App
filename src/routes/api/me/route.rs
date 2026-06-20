use axum::{response::IntoResponse, Extension, Json};
use serde::Serialize;
use typeshare::typeshare;

use crate::auth::Principal;

#[typeshare]
#[derive(Serialize)]
pub struct UserInfo {
    pub email: Option<String>,
    pub username: Option<String>,
    pub role: String,
}

pub async fn get(Extension(principal): Extension<Principal>) -> impl IntoResponse {
    Json(UserInfo {
        email: principal.email,
        username: principal.username,
        role: principal.role,
    })
    .into_response()
}
