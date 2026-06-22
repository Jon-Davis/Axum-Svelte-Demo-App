use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::AppState;
use crate::auth::api_keys;
use crate::error::Result;

/// Delete an API key by ID (admin only).
pub async fn delete(State(state): State<&'static AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    api_keys::delete(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}
