//! OIDC handshake: building the provider authorization URL (login) and
//! exchanging the returned code for a verified user identity (callback).

use openidconnect::{
    AuthenticationFlow, AuthorizationCode, CsrfToken, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    Scope,
    core::{CoreClient, CoreResponseType},
    reqwest::async_http_client,
    url::Url,
};

use crate::error::{Error, Result};

/// Per-login state stashed in an encrypted cookie between `/auth/login` and
/// `/auth/callback`. Kept in one place so the serialized shape can't drift.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FlowState {
    pub csrf_token: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

/// User identity extracted from a verified ID token.
pub struct OidcUser {
    pub sub: String,
    pub email: String,
    pub username: Option<String>,
}

/// Begin a login: build the provider authorization URL and the flow state the
/// caller must persist (in a cookie) until the callback.
pub fn begin(client: &CoreClient) -> (Url, FlowState) {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let flow = FlowState {
        csrf_token: csrf_token.secret().clone(),
        nonce: nonce.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    };

    (auth_url, flow)
}

/// Complete a login: exchange the authorization code for tokens and verify the
/// ID token against the stored nonce, returning the user's identity.
pub async fn complete(client: &CoreClient, code: String, flow: &FlowState) -> Result<OidcUser> {
    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier.clone()))
        .request_async(async_http_client)
        .await
        .map_err(|e| Error::Auth(format!("token exchange failed: {e}")))?;

    let nonce = Nonce::new(flow.nonce.clone());
    let verifier = client.id_token_verifier();
    let id_token = token_response
        .extra_fields()
        .id_token()
        .ok_or_else(|| Error::Auth("provider did not return an ID token".into()))?;

    let claims = id_token.claims(&verifier, &nonce).map_err(|e| {
        // An unverifiable token is the caller's problem, not ours → 401.
        tracing::warn!("ID token verification failed: {e}");
        Error::Unauthorized
    })?;

    // Only trust an email the provider says it has verified. We key identity on
    // `sub`, but the stored email is matched on elsewhere (admin bootstrap is
    // `UPDATE users SET role='admin' WHERE email=…`), so an unverified address —
    // which some providers let a user set freely — must never be persisted.
    // Reject the login rather than store an untrustworthy email.
    let email = match (claims.email(), claims.email_verified()) {
        (Some(email), Some(true)) => email.to_string(),
        (Some(_), _) => {
            tracing::warn!("rejecting login: email present but not verified by provider");
            return Err(Error::Unauthorized);
        }
        (None, _) => {
            tracing::warn!("rejecting login: provider returned no email claim");
            return Err(Error::Unauthorized);
        }
    };

    Ok(OidcUser {
        sub: claims.subject().to_string(),
        email,
        username: claims.preferred_username().map(|u| u.to_string()),
    })
}
