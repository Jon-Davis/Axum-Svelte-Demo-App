use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};

use crate::AppState;
use crate::auth::{oidc, secure_cookie, sessions, users};
use crate::error::{Error, Result};

#[derive(serde::Deserialize)]
pub struct CallbackParams {
    // All optional: the provider may redirect back with an `error` (e.g. the user
    // declined consent) and no `code`/`state` at all. We handle that explicitly
    // instead of failing extraction with an opaque 400.
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn get(
    State(state): State<&'static AppState>,
    jar: PrivateCookieJar,
    Query(params): Query<CallbackParams>,
) -> Result<Response> {
    // The provider reported an error (declined consent, invalid request, …).
    // Not our failure — send the user back to retry login.
    if let Some(err) = params.error {
        tracing::warn!(
            "OIDC provider returned error: {err}{}",
            params
                .error_description
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        );
        return Ok(Redirect::to("/auth/login").into_response());
    }

    // A successful redirect must carry both `code` and `state`.
    let (Some(code), Some(returned_state)) = (params.code, params.state) else {
        return Err(Error::BadRequest("missing code or state".into()));
    };

    // Recover OIDC flow state from the encrypted cookie and verify the CSRF token.
    let oidc_cookie = jar
        .get("oidc_state")
        .ok_or_else(|| Error::BadRequest("missing OIDC state cookie".into()))?;
    let flow = serde_json::from_str::<oidc::FlowState>(oidc_cookie.value())
        .map_err(|_| Error::BadRequest("invalid OIDC state cookie".into()))?;
    if returned_state != flow.csrf_token {
        return Err(Error::BadRequest("CSRF token mismatch".into()));
    }

    // Exchange the code, verify the ID token, then upsert the user and open a session.
    let user = oidc::complete(&state.oidc, code, &flow).await?;
    let role = users::upsert_and_get_role(&state.db, &user.sub, &user.email).await?;
    let session_id =
        sessions::create(&state.db, &user.sub, &user.email, user.username.as_deref(), &role).await?;

    // Set the session cookie and clear the temporary OIDC flow cookie.
    let mut session_cookie = secure_cookie("session_id", session_id);
    session_cookie.set_max_age(time::Duration::days(7));

    let updated_jar = jar.remove(Cookie::from("oidc_state")).add(session_cookie);

    Ok((updated_jar, Redirect::to("/")).into_response())
}
