use axum::{Router, http::StatusCode, Json};
use serde::Serialize;

#[derive(Serialize)]
struct NotFound {
    error: &'static str,
}

pub fn fallback<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.fallback(|| async {
        (StatusCode::NOT_FOUND, Json(NotFound { error: "not found" }))
    })
}
