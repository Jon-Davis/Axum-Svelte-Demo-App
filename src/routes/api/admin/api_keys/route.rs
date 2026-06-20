use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::auth::api_keys;
use crate::error::Result;

#[derive(Deserialize)]
pub struct CreateRequest {
    name: String,
    role: String,
    expires_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
pub struct CreateResponse {
    id: Uuid,
    name: String,
    token: String,
}

// Admin-only: the `/api/admin` `middleware.rs` rejects non-admins before the
// request reaches this handler.
pub async fn get(State(state): State<AppState>) -> Result<Json<Vec<api_keys::ApiKey>>> {
    Ok(Json(api_keys::list(&state.db).await?))
}

pub async fn post(
    State(state): State<AppState>,
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
