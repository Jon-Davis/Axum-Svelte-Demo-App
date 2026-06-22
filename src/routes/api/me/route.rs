use axum::{Extension, Json};
use serde::Serialize;
use serde_with::skip_serializing_none;
use typeshare::typeshare;

use crate::auth::Principal;

// `None` fields are omitted from the JSON entirely (rather than serialised as
// `null`), so the wire matches typeshare's `field?: T` (optional/undefined) types
// out of the box — no client-side null→undefined normalisation needed.
#[skip_serializing_none]
#[typeshare]
#[derive(Serialize, utoipa::ToSchema)]
pub struct UserInfo {
    pub email: Option<String>,
    pub username: Option<String>,
    pub role: String,
}

pub async fn get(Extension(principal): Extension<Principal>) -> Json<UserInfo> {
    Json(UserInfo {
        email: principal.email,
        username: principal.username,
        role: principal.role,
    })
}
