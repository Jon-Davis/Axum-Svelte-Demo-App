//! Authentication and authorization. Submodules hold the persistence/service
//! logic the route handlers call into; this root keeps the cross-cutting pieces:
//! the request `Principal`, the secure-cookie builder, and the middleware
//! functions that guard the app. The middleware here is the *logic*; it's wired
//! onto the route tree by the thin `middleware.rs` wrappers under `src/routes`
//! (`require_api_auth`, `require_admin`) and onto the static fallback in `main`
//! (`require_page_auth`).

pub mod api_keys;
pub mod oidc;
pub mod sessions;
pub mod users;

// Persistence layer for this module: every `sqlx` query lives here, the service
// modules above call into it. Private — callers go through the services.
mod db;

use axum::{
    Extension,
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};

use crate::AppState;
use crate::error::Error;

/// The roles the application recognises. Anything outside this set is rejected
/// at the boundary (API-key creation) so a typo can never produce a key that
/// silently fails every `is_admin()` check.
pub const VALID_ROLES: [&str; 2] = ["user", "admin"];

/// Attached to every authenticated request by `require_login`.
/// Handlers extract it with `Extension<Principal>` to check roles and identity.
/// `email`/`username` are populated for session (web) users and `None` for
/// API-key (service-account) callers, which have no human identity.
#[derive(Clone)]
pub struct Principal {
    pub role: String,
    pub email: Option<String>,
    pub username: Option<String>,
}

impl Principal {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// Builds a cookie with the security attributes every cookie in this app needs:
/// `HttpOnly` (no JS access), `Path=/`, and `SameSite=Lax` (blocks the cookie
/// from riding along on cross-site POST/DELETE, closing CSRF on the cookie-authed
/// API and on logout). `Secure` is set in release builds so it is never sent over
/// plain HTTP in production, but left off in debug so local http://localhost works.
/// The caller sets `Max-Age` (it differs per cookie).
pub fn secure_cookie<'a>(name: &'a str, value: String) -> Cookie<'a> {
    let mut cookie = Cookie::new(name, value);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(!cfg!(debug_assertions));
    cookie
}

/// Resolve the caller from their session cookie, or `None` if there's no valid
/// session. A DB error is swallowed (treated as "no session") so an outage
/// degrades to a login redirect / 401 rather than a 500 on every request.
async fn session_principal(state: &AppState, jar: &PrivateCookieJar) -> Option<Principal> {
    let cookie = jar.get("session_id")?;
    sessions::find(&state.db, cookie.value())
        .await
        .ok()
        .flatten()
        .map(|s| Principal {
            role: s.role,
            email: Some(s.email),
            username: s.username,
        })
}

/// Auth for the entire `/api` subtree. Accepts a Bearer API key (service
/// accounts) or a session cookie (a browser calling its own API); anything else
/// is a 401. Never redirects — API callers get a status code, not HTML.
///
/// This only runs on matched `/api` routes (wired with `route_layer`), so the
/// public siblings `/auth`, `/health` and `/ready` — which live outside this
/// folder — need no carve-out here.
pub async fn require_api_auth(
    State(state): State<&'static AppState>,
    jar: PrivateCookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    // Check the Authorization header first. If a Bearer token is present we
    // validate it and decide immediately — we don't fall through to the session
    // check, so a bad/unknown token always gets a 401 (a DB error is treated the
    // same way, never a fall-through to a session).
    if let Some(bearer) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        return match api_keys::authenticate(&state.db, bearer).await {
            Ok(Some(role)) => {
                // Service accounts have no human identity.
                request.extensions_mut().insert(Principal {
                    role,
                    email: None,
                    username: None,
                });
                next.run(request).await
            }
            _ => StatusCode::UNAUTHORIZED.into_response(),
        };
    }

    match session_principal(&state, &jar).await {
        Some(p) => {
            request.extensions_mut().insert(p);
            next.run(request).await
        }
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Auth for the SvelteKit pages served by the static fallback. Session-cookie
/// only (no Bearer); an unauthenticated request is redirected to login rather
/// than refused. Static files don't read the `Principal`, so this only gates
/// access — it doesn't bother attaching it.
pub async fn require_page_auth(
    State(state): State<&'static AppState>,
    jar: PrivateCookieJar,
    request: Request,
    next: Next,
) -> Response {
    // SvelteKit's bundled assets must load even for an unauthenticated browser
    // (e.g. while a redirect to login is in flight), so they stay public.
    if request.uri().path().starts_with("/_app/") {
        return next.run(request).await;
    }

    if session_principal(&state, &jar).await.is_some() {
        next.run(request).await
    } else {
        Redirect::to("/auth/login").into_response()
    }
}

/// Authorization for the `/api/admin` subtree. The `Principal` is already in the
/// request extensions — `require_api_auth` is the outer `/api` layer and runs
/// first — so this only has to check the role.
pub async fn require_admin(
    Extension(principal): Extension<Principal>,
    request: Request,
    next: Next,
) -> Response {
    if !principal.is_admin() {
        return Error::Forbidden.into_response();
    }
    next.run(request).await
}
