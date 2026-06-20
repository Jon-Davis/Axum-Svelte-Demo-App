use axum::{extract::State, response::{IntoResponse, Redirect}};
use axum_extra::extract::cookie::PrivateCookieJar;

use crate::AppState;
use crate::auth::{oidc, secure_cookie};

pub async fn get(State(state): State<&'static AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    let (auth_url, flow) = oidc::begin(&state.oidc);

    let mut cookie = secure_cookie(
        "oidc_state",
        serde_json::to_string(&flow).expect("FlowState serialization cannot fail"),
    );
    cookie.set_max_age(time::Duration::minutes(10));

    (jar.add(cookie), Redirect::to(auth_url.as_str()))
}
