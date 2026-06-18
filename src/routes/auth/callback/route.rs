use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use openidconnect::{AuthorizationCode, Nonce, PkceCodeVerifier, reqwest::async_http_client};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{secure_cookie, OidcFlowState};

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
) -> Response {
    // The provider reported an error (declined consent, invalid request, …).
    if let Some(err) = params.error {
        tracing::warn!(
            "OIDC provider returned error: {err}{}",
            params
                .error_description
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        );
        return Redirect::to("/auth/login").into_response();
    }

    // A successful redirect must carry both `code` and `state`.
    let (Some(code), Some(returned_state)) = (params.code, params.state) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    // Recover OIDC flow state from encrypted cookie
    let Some(oidc_cookie) = jar.get("oidc_state") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(flow_state) = serde_json::from_str::<OidcFlowState>(oidc_cookie.value()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    // Verify CSRF token
    if returned_state != flow_state.csrf_token {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Exchange authorization code for tokens
    let token_response = match state
        .oidc
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(flow_state.pkce_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("token exchange failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Verify ID token and extract claims
    let nonce = Nonce::new(flow_state.nonce);
    let verifier = state.oidc.id_token_verifier();
    let id_token = match token_response.extra_fields().id_token() {
        Some(t) => t,
        None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let claims = match id_token.claims(&verifier, &nonce) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("ID token verification failed: {e}");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let user_sub = claims.subject().to_string();
    let email = claims.email().map(|e| e.to_string()).unwrap_or_default();
    let username = claims.preferred_username().map(|u| u.to_string());

    // Upsert user record and retrieve their role.
    // ON CONFLICT only updates email, preserving any role changes made by an admin.
    let role_row = sqlx::query_as::<_, (String,)>(
        "INSERT INTO users (sub, email) VALUES ($1, $2)
         ON CONFLICT (sub) DO UPDATE SET email = EXCLUDED.email
         RETURNING role",
    )
    .bind(&user_sub)
    .bind(&email)
    .fetch_one(&state.db)
    .await;

    let role = match role_row {
        Ok((r,)) => r,
        Err(e) => {
            tracing::error!("user upsert failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Create session with the role baked in
    let session_id = Uuid::new_v4().to_string();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(7);

    let db_result = sqlx::query(
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
    .await;

    if let Err(e) = db_result {
        tracing::error!("failed to create session: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Set session cookie and clear the temporary OIDC flow cookie
    let mut session_cookie = secure_cookie("session_id", session_id);
    session_cookie.set_max_age(time::Duration::days(7));

    let updated_jar = jar
        .remove(Cookie::from("oidc_state"))
        .add(session_cookie);

    (updated_jar, Redirect::to("/")).into_response()
}
