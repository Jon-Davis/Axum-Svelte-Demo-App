use axum::{extract::State, response::{IntoResponse, Redirect}};
use axum_extra::extract::cookie::PrivateCookieJar;
use openidconnect::{
    AuthenticationFlow, CsrfToken, Nonce, PkceCodeChallenge, Scope,
    core::CoreResponseType,
};

use crate::AppState;
use crate::auth::{secure_cookie, OidcFlowState};

pub async fn get(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = state
        .oidc
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

    let flow_state = OidcFlowState {
        csrf_token: csrf_token.secret().clone(),
        nonce: nonce.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    };

    let mut cookie = secure_cookie(
        "oidc_state",
        serde_json::to_string(&flow_state).expect("OidcFlowState serialization cannot fail"),
    );
    cookie.set_max_age(time::Duration::minutes(10));

    (jar.add(cookie), Redirect::to(auth_url.as_str()))
}
