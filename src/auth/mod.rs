//! Authentication and authorization. Submodules hold the persistence/service
//! logic the route handlers call into; this root keeps the cross-cutting pieces:
//! the request `Principal`, the secure-cookie builder, and the auth helpers that
//! guard the app. The logic here is consumed by the `intercept.rs` guards under
//! `src/routes` (`api`, `api/admin`, `admin` all call `principal_from_headers` /
//! read the `Principal`) and by the static fallback (`require_page_auth`).

pub mod api_keys;
pub mod oidc;
pub mod sessions;
pub mod users;

// Persistence layer for this module: every `sqlx` query lives here, the service
// modules above call into it. Private — callers go through the services.
mod db;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};

use crate::AppState;

/// The roles the application recognises. Anything outside this set is rejected
/// at the boundary (API-key creation) so a typo can never produce a key that
/// silently fails every `is_admin()` check.
pub const VALID_ROLES: [&str; 2] = ["user", "admin"];

/// `Authorization: Bearer …` typed-header extractor, aliased here so the `/api`
/// intercept can name it without the full `axum_extra` path. The `folder_router`
/// macro reproduces an intercept's parameter types at its invocation site, so the
/// alias is referenced there by crate-absolute path (`crate::auth::BearerHeader`),
/// which resolves regardless of what that site imports.
pub type BearerHeader =
    axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>;

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
///
/// Public so the `intercept.rs` guards can call it with the `PrivateCookieJar`
/// they extract (the `/api` and `/admin` intercepts) — no manual jar
/// reconstruction needed now that intercepts accept extractors.
pub async fn session_principal(state: &AppState, jar: &PrivateCookieJar) -> Option<Principal> {
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
