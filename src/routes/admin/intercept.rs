//! Server-side authorization guard for the `/admin` page subtree.
//!
//! `/admin` is a SvelteKit page (no `route.rs`), so it's served by the root
//! [`fallback`](../fallback.rs). That fallback's `require_page_auth` only checks
//! that a *session* exists — not the caller's role — so without this guard a
//! logged-in non-admin could load the admin shell and only discover they're
//! locked out client-side (a 403 from the API). This `intercept.rs` makes
//! `/admin` a boundary and gates it at the server: a non-admin is diverted to
//! `/forbidden` before the page is ever served.
//!
//! The macro always attaches an intercept with `.layer` (never `route_layer`),
//! so it runs over this subtree's inherited fallback too — i.e. the actual page
//! serve. It runs *above* `require_page_auth`, so we leave the no-session case to
//! that layer's login redirect and only act on the authenticated-but-unauthorized
//! case here.
//!
//! The session is resolved straight from the extracted `PrivateCookieJar`
//! (fully qualified so it resolves at the `#[folder_router]` site, which doesn't
//! import it); `State` carries the app state for the cookie key and DB.

use std::ops::ControlFlow;

use axum::{
    extract::{Request, State},
    response::{IntoResponse, Redirect, Response},
};

use crate::AppState;
use crate::auth::session_principal;

pub async fn intercept(
    jar: axum_extra::extract::cookie::PrivateCookieJar,
    State(state): State<&'static AppState>,
    req: Request,
) -> ControlFlow<Response, Request> {
    match session_principal(state, &jar).await {
        // Admin: let the request through to the page.
        Some(p) if p.is_admin() => ControlFlow::Continue(req),
        // Logged in but not an admin: divert to the insufficient-permissions page.
        Some(_) => ControlFlow::Break(Redirect::to("/forbidden").into_response()),
        // No session: defer to the fallback's `require_page_auth`, which redirects
        // to login.
        None => ControlFlow::Continue(req),
    }
}
