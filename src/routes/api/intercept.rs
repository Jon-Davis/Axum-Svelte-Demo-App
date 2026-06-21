//! Authentication for the entire `/api` subtree. Accepts a Bearer API key
//! (service accounts) or a session cookie (a browser calling its own API); on
//! success it attaches the resolved `Principal` to the request — which a nested
//! `intercept` (`api/admin`) and the handlers read via `Extension<Principal>` —
//! and lets it through. Anything else is a 401: API callers get a status code,
//! never an HTML redirect.
//!
//! The "inspect the request, attach a principal, else divert" shape is exactly
//! what an intercept expresses, so the logic lives here directly. The session
//! cookie is read from the extracted `PrivateCookieJar` (fully qualified so it
//! resolves at the `#[folder_router]` site); `State` carries the app state.
//!
//! Behaviour note: an intercept is always `.layer`ed, so this also runs on
//! *unmatched* `/api/*` paths — an unknown `/api/...` returns 401 rather than
//! falling through to the SvelteKit fallback. The public siblings `/auth`,
//! `/health`, `/ready` live outside this folder and stay unauthenticated.

use std::ops::ControlFlow;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::{TypedHeader, headers::Authorization};

use crate::AppState;
use crate::auth::{Principal, api_keys, session_principal};

pub async fn intercept(
    // `crate::auth::BearerHeader` (a crate-absolute alias) so the type resolves
    // where the macro reproduces it — the `#[folder_router]` site.
    bearer: Option<crate::auth::BearerHeader>,
    jar: axum_extra::extract::cookie::PrivateCookieJar,
    State(state): State<&'static AppState>,
    mut req: Request,
) -> ControlFlow<Response, Request> {
    // Check the Authorization header first. If a Bearer token is present we
    // validate it and decide immediately — we don't fall through to the session
    // check, so a bad/unknown token always gets a 401 (a DB error is treated the
    // same way, never a fall-through to a session). A missing or non-Bearer
    // header extracts as `None`, so those fall through to the session check.
    if let Some(TypedHeader(Authorization(bearer))) = bearer {
        return match api_keys::authenticate(&state.db, bearer.token()).await {
            Ok(Some(role)) => {
                // Service accounts have no human identity.
                req.extensions_mut().insert(Principal {
                    role,
                    email: None,
                    username: None,
                });
                ControlFlow::Continue(req)
            }
            _ => ControlFlow::Break(StatusCode::UNAUTHORIZED.into_response()),
        };
    }

    match session_principal(state, &jar).await {
        Some(p) => {
            req.extensions_mut().insert(p);
            ControlFlow::Continue(req)
        }
        None => ControlFlow::Break(StatusCode::UNAUTHORIZED.into_response()),
    }
}
