use axum::{extract::State, response::{IntoResponse, Redirect}};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};

use crate::AppState;
use crate::auth::sessions;

// POST, not GET: a state-changing action must not be triggerable by a cross-site
// `<img src="/auth/logout">` or link prefetch. Combined with the session cookie's
// `SameSite=Lax`, this closes logout CSRF.
pub async fn post(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    if let Some(session_cookie) = jar.get("session_id") {
        // Best-effort: a DB hiccup must not stop us clearing the cookie.
        let _ = sessions::delete(&state.db, session_cookie.value()).await;
    }

    (jar.remove(Cookie::from("session_id")), Redirect::to("/"))
}
