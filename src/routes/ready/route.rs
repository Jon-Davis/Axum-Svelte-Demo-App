use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::AppState;
use crate::health;

#[derive(Serialize)]
struct Readiness {
    status: &'static str,
}

/// Readiness probe: the app can serve real traffic, which means its dependencies
/// are reachable. Returns 503 while the database is down so a load balancer pulls
/// this instance out of rotation without the process being restarted.
pub async fn get(State(state): State<&'static AppState>) -> impl IntoResponse {
    if health::db_ready(&state.db).await {
        (StatusCode::OK, Json(Readiness { status: "ready" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Readiness {
                status: "unavailable",
            }),
        )
    }
}
