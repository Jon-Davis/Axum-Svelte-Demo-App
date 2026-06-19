//! Root fallback: serves the SvelteKit app for everything that didn't match a
//! `route.rs`. Picked up by `folder_router` and applied as the router's
//! fallback. State-aware so it can carry the session guard. Thin wrapper — the
//! auth logic lives in [`crate::auth::require_page_auth`].

use axum::{Router, middleware::from_fn_with_state};
use tower_http::services::{ServeDir, ServeFile};

use crate::AppState;
use crate::auth::require_page_auth;

pub fn fallback(router: Router<AppState>, state: AppState) -> Router<AppState> {
    // Wrapping the static service in its own Router keeps `require_page_auth`
    // scoped to the fallback (not layered over the whole tree) and normalises
    // the `ServeDir` body type so it's accepted as a `fallback_service`.
    let static_files = Router::new()
        .fallback_service(
            ServeDir::new("build").not_found_service(ServeFile::new("build/index.html")),
        )
        .layer(from_fn_with_state(state, require_page_auth));

    router.fallback_service(static_files)
}
