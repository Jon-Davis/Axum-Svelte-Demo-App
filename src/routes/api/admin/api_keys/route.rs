use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::auth::api_keys;
use crate::error::Result;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateRequest {
    pub name: String,
    pub role: String,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CreateResponse {
    pub id: Uuid,
    pub name: String,
    pub token: String,
}

/// List all API keys (admin only).
pub async fn get(State(state): State<&'static AppState>) -> Result<Json<Vec<api_keys::ApiKey>>> {
    Ok(Json(api_keys::list(&state.db).await?))
}

/// Create a new API key (admin only).
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
