use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use openidconnect::{AuthorizationCode, Nonce, PkceCodeVerifier, reqwest::async_http_client};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{secure_cookie, OidcFlowState};
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
    State(state): State<AppState>,
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

    // Recover OIDC flow state from encrypted cookie
    let oidc_cookie = jar
        .get("oidc_state")
        .ok_or_else(|| Error::BadRequest("missing OIDC state cookie".into()))?;
    let flow_state = serde_json::from_str::<OidcFlowState>(oidc_cookie.value())
        .map_err(|_| Error::BadRequest("invalid OIDC state cookie".into()))?;

    // Verify CSRF token
    if returned_state != flow_state.csrf_token {
        return Err(Error::BadRequest("CSRF token mismatch".into()));
    }

    // Exchange authorization code for tokens
    let token_response = state
        .oidc
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(flow_state.pkce_verifier))
        .request_async(async_http_client)
        .await
        .map_err(|e| Error::Auth(format!("token exchange failed: {e}")))?;

    // Verify ID token and extract claims
    let nonce = Nonce::new(flow_state.nonce);
    let verifier = state.oidc.id_token_verifier();
    let id_token = token_response
        .extra_fields()
        .id_token()
        .ok_or_else(|| Error::Auth("provider did not return an ID token".into()))?;
    let claims = id_token.claims(&verifier, &nonce).map_err(|e| {
        // An unverifiable token is the caller's problem, not ours → 401.
        tracing::warn!("ID token verification failed: {e}");
        Error::Unauthorized
    })?;

    let user_sub = claims.subject().to_string();
    let email = claims.email().map(|e| e.to_string()).unwrap_or_default();
    let username = claims.preferred_username().map(|u| u.to_string());

    // Upsert user record and retrieve their role.
    // ON CONFLICT only updates email, preserving any role changes made by an admin.
    let (role,) = sqlx::query_as::<_, (String,)>(
        "INSERT INTO users (sub, email) VALUES ($1, $2)
         ON CONFLICT (sub) DO UPDATE SET email = EXCLUDED.email
         RETURNING role",
    )
    .bind(&user_sub)
    .bind(&email)
    .fetch_one(&state.db)
    .await?;

    // Create session with the role baked in
    let session_id = Uuid::new_v4().to_string();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(7);

    sqlx::query(
        "INSERT INTO sessions (id, user_sub, email, username, role, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&session_id)
    .bind(&user_sub)
    .bind(&email)
    .bind(username.as_deref())
    .bind(&role)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    // Set session cookie and clear the temporary OIDC flow cookie
    let mut session_cookie = secure_cookie("session_id", session_id);
    session_cookie.set_max_age(time::Duration::days(7));

    let updated_jar = jar
        .remove(Cookie::from("oidc_state"))
        .add(session_cookie);

    Ok((updated_jar, Redirect::to("/")).into_response())
}
