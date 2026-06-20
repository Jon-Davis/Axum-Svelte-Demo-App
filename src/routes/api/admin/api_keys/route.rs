use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use typeshare::typeshare;
use uuid::Uuid;

use crate::AppState;
use crate::auth::api_keys;
use crate::error::Result;

#[typeshare]
#[derive(Deserialize)]
pub struct CreateRequest {
    pub name: String,
    pub role: String,
    pub expires_at: Option<OffsetDateTime>,
}

#[typeshare]
#[derive(Serialize)]
pub struct CreateResponse {
    pub id: Uuid,
    pub name: String,
    pub token: String,
}

// Admin-only: the `/api/admin` `middleware.rs` rejects non-admins before the
// request reaches this handler.
pub async fn get(State(state): State<&'static AppState>) -> Result<Json<Vec<api_keys::ApiKey>>> {
    Ok(Json(api_keys::list(&state.db).await?))
}

pub async fn post(
    State(state): State<&'static AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CreateResponse>)> {
    let created = api_keys::create(&state.db, &body.name, &body.role, body.expires_at).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateResponse {
            id: created.id,
            name: body.name,
            token: created.token,
        }),
    ))
}
