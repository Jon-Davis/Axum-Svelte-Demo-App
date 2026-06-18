//! Authentication and authorization. Submodules hold the persistence/service
//! logic the route handlers call into; this root keeps the cross-cutting pieces:
//! the request `Principal`, the secure-cookie builder, and the `require_login`
//! middleware that every request passes through.

pub mod api_keys;
pub mod oidc;
pub mod sessions;
pub mod users;

// Persistence layer for this module: every `sqlx` query lives here, the service
// modules above call into it. Private — callers go through the services.
mod db;

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};

use crate::AppState;

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

pub async fn require_login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Auth flow and SvelteKit's bundled assets are always public
    if path.starts_with("/auth/") || path.starts_with("/_app/") {
        return next.run(request).await;
    }

    let is_api = path.starts_with("/api/");

    // For API routes check the Authorization header first.
    // If a Bearer token is present we validate it and make a decision immediately —
    // we don't fall through to the session check, so a bad/unknown token always gets
    // a 401 (a DB error is treated the same way, never a fall-through to a session).
    if is_api {
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
    }

    // Session cookie check — fetches the identity and role baked in at login time.
    // A DB error is swallowed (treated as "no session") so an outage degrades to a
    // login redirect rather than a 500 on every page.
    let principal = match jar.get("session_id") {
        None => None,
        Some(cookie) => sessions::find(&state.db, cookie.value())
            .await
            .ok()
            .flatten()
            .map(|s| Principal {
                role: s.role,
                email: Some(s.email),
                username: s.username,
            }),
    };

    match principal {
        Some(p) => {
            request.extensions_mut().insert(p);
            next.run(request).await
        }
        None if is_api => StatusCode::UNAUTHORIZED.into_response(),
        None => Redirect::to("/auth/login").into_response(),
    }
}
