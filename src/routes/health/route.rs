use axum::{Json, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

/// Liveness probe: the process is up and the HTTP stack is serving. Deliberately
/// does no I/O — it must stay green even while a dependency is down, so an
/// orchestrator restarts the process only when the process itself is wedged.
pub async fn get() -> impl IntoResponse {
    Json(Health { status: "ok" })
}
