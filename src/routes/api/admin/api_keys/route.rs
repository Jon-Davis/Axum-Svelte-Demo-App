use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{api_keys, Principal};
use crate::error::{Error, Result};

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

pub async fn get(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
) -> Result<Json<Vec<api_keys::ApiKey>>> {
    if !principal.is_admin() {
        return Err(Error::Forbidden);
    }

    Ok(Json(api_keys::list(&state.db).await?))
}

pub async fn post(
    Extension(principal): Extension<Principal>,
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CreateResponse>)> {
    if !principal.is_admin() {
        return Err(Error::Forbidden);
    }

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
