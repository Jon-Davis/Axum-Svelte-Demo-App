use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;

pub async fn delete(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !principal.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    match sqlx::query("DELETE FROM api_keys WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("api_keys delete failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
