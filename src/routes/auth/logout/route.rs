use axum::{extract::State, response::{IntoResponse, Redirect}};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};

use crate::AppState;

// POST, not GET: a state-changing action must not be triggerable by a cross-site
// `<img src="/auth/logout">` or link prefetch. Combined with the session cookie's
// `SameSite=Lax`, this closes logout CSRF.
pub async fn post(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    if let Some(session_cookie) = jar.get("session_id") {
        let session_id = session_cookie.value().to_string();
        let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(&session_id)
            .execute(&state.db)
            .await;
    }

    (jar.remove(Cookie::from("session_id")), Redirect::to("/"))
}
