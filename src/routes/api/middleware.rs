//! Authentication for the entire `/api` subtree. Picked up by `folder_router`
//! and applied to this folder's own routes plus every nested route. Sibling
//! subtrees (`/auth`, `/health`, `/ready`) live outside this folder and stay
//! public. Thin wrapper — the logic lives in [`crate::auth::require_api_auth`].

use axum::{Router, middleware::from_fn_with_state};

use crate::AppState;
use crate::auth::require_api_auth;

pub fn middleware(router: Router<AppState>, state: AppState) -> Router<AppState> {
    // `route_layer` (vs `layer`) skips the middleware on unmatched paths.
    router.route_layer(from_fn_with_state(state, require_api_auth))
}
