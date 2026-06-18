use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{api_keys, Principal};
use crate::error::{Error, Result};

pub async fn delete(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    if !principal.is_admin() {
        return Err(Error::Forbidden);
    }

    api_keys::delete(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}
