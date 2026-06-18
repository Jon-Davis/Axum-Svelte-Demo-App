use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use sha2::{Digest, Sha256};

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

/// OIDC flow state stashed in an encrypted cookie between `/auth/login` and
/// `/auth/callback`. Shared here so the serialized shape can't drift between
/// the two handlers that read and write it.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct OidcFlowState {
    pub csrf_token: String,
    pub nonce: String,
    pub pkce_verifier: String,
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

    // For API routes check the Authorization header first.
    // If a Bearer token is present we validate it and make a decision immediately —
    // we don't fall through to the session check, so a bad/unknown token always gets a 401.
    if path.starts_with("/api/") {
        if let Some(bearer) = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
        {
            let key_hash = hex::encode(Sha256::digest(bearer.as_bytes()));
            let row = sqlx::query_as::<_, (String,)>(
                "SELECT role FROM api_keys \
                 WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > NOW())",
            )
            .bind(&key_hash)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            return match row {
                Some((role,)) => {
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            "UPDATE api_keys SET last_used_at = NOW() WHERE key_hash = $1",
                        )
                        .bind(&key_hash)
                        .execute(&db)
                        .await;
                    });
                    // Service accounts have no human identity.
                    request.extensions_mut().insert(Principal {
                        role,
                        email: None,
                        username: None,
                    });
                    next.run(request).await
                }
                None => StatusCode::UNAUTHORIZED.into_response(),
            };
        }
    }

    // Session cookie check — fetches the identity and role baked in at login time
    let principal = match jar.get("session_id") {
        None => None,
        Some(cookie) => {
            sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT role, email, username FROM sessions \
                 WHERE id = $1 AND expires_at > NOW()",
            )
            .bind(cookie.value())
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|(role, email, username)| Principal {
                role,
                email: Some(email),
                username,
            })
        }
    };

    match principal {
        Some(p) => {
            request.extensions_mut().insert(p);
            next.run(request).await
        }
        None if path.starts_with("/api/") => StatusCode::UNAUTHORIZED.into_response(),
        None => Redirect::to("/auth/login").into_response(),
    }
}
