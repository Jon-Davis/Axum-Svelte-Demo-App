//! Restricts the `/api/admin` subtree to admins. Runs after `require_api_auth`
//! (the outer `/api` layer), which has already resolved and attached the
//! `Principal`, so this is a stateless role check. Thin wrapper — the logic
//! lives in [`crate::auth::require_admin`].

use axum::{Router, middleware::from_fn};

use crate::auth::require_admin;

pub fn middleware<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.route_layer(from_fn(require_admin))
}
