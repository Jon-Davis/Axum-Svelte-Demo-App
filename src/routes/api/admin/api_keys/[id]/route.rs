use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::AppState;
use crate::auth::api_keys;
use crate::error::Result;

// Admin-only: the `/api/admin` `middleware.rs` rejects non-admins before the
// request reaches this handler.
pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    api_keys::delete(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}
