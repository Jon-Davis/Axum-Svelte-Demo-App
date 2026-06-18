use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Principal, VALID_ROLES};

#[derive(Serialize, sqlx::FromRow)]
struct ApiKeyRow {
    id: Uuid,
    name: String,
    role: String,
    created_at: OffsetDateTime,
    expires_at: Option<OffsetDateTime>,
    last_used_at: Option<OffsetDateTime>,
}

#[derive(Deserialize)]
pub struct CreateRequest {
    name: String,
    role: String,
    expires_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct CreateResponse {
    id: Uuid,
    name: String,
    token: String,
}

pub async fn get(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !principal.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    match sqlx::query_as::<_, ApiKeyRow>(
        "SELECT id, name, role, created_at, expires_at, last_used_at \
         FROM api_keys ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(keys) => Json(keys).into_response(),
        Err(e) => {
            tracing::error!("api_keys list failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> impl IntoResponse {
    if !principal.is_admin() {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Reject unknown roles up front — a typo like "Admin" would otherwise create a
    // key that silently fails every `is_admin()` check. The DB CHECK constraint is
    // the backstop; this gives the caller a clean 400 instead of a 500.
    if !VALID_ROLES.contains(&body.role.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            format!("invalid role: must be one of {VALID_ROLES:?}"),
        )
            .into_response();
    }

    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = format!("svrt_{}", hex::encode(raw));
    let key_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let id = Uuid::new_v4();

    match sqlx::query(
        "INSERT INTO api_keys (id, name, role, key_hash, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(&body.name)
    .bind(&body.role)
    .bind(&key_hash)
    .bind(body.expires_at)
    .execute(&state.db)
    .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(CreateResponse { id, name: body.name, token }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("api_keys create failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
