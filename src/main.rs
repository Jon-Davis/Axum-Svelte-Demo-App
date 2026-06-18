mod auth;

use axum::{Router, extract::FromRef};
use axum_extra::extract::cookie::Key;
use axum_folder_router::folder_router;
use openidconnect::{
    ClientId, ClientSecret, IssuerUrl, RedirectUrl,
    core::{CoreClient, CoreProviderMetadata},
    reqwest::async_http_client,
};
use sqlx::PgPool;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[derive(serde::Deserialize)]
struct Config {
    database_url: String,
    oidc_issuer: String,
    oidc_client_id: String,
    oidc_client_secret: String,
    oidc_redirect_uri: String,
    session_secret: String,
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub oidc: CoreClient,
    pub cookie_key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

#[folder_router("./src/routes", AppState)]
struct ApiRouter();

/// Periodically delete expired sessions. Without this, the `sessions` table grows
/// one row per login forever — expired rows are filtered out of every query but
/// never reclaimed. Runs hourly; the `sessions_expires_at_idx` index keeps the
/// delete cheap.
fn spawn_session_reaper(db: PgPool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
                .execute(&db)
                .await
            {
                Ok(r) if r.rows_affected() > 0 => {
                    tracing::info!("reaped {} expired session(s)", r.rows_affected());
                }
                Ok(_) => {}
                Err(e) => tracing::error!("session reaper failed: {e}"),
            }
        }
    });
}

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    let config: Config = envy::from_env().expect("missing required env vars");

    tracing_subscriber::fmt::init();

    let db = PgPool::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");

    spawn_session_reaper(db.clone());

    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(config.oidc_issuer).expect("invalid OIDC issuer URL"),
        async_http_client,
    )
    .await
    .expect("failed to discover OIDC provider — is Dex running?");

    let oidc = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(config.oidc_client_id),
        Some(ClientSecret::new(config.oidc_client_secret)),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.oidc_redirect_uri).expect("invalid redirect URI"),
    );

    let cookie_key = Key::from(config.session_secret.as_bytes());
    let state = AppState { db, oidc, cookie_key };

    let api = ApiRouter::into_router().with_state(state.clone());

    let app = Router::new()
        .merge(api)
        .fallback_service(
            ServeDir::new("build")
                .not_found_service(ServeFile::new("build/index.html")),
        )
        .layer(axum::middleware::from_fn_with_state(state, auth::require_login))
        .layer(TraceLayer::new_for_http());

    let host = config.host.as_deref().unwrap_or("127.0.0.1");
    let port = config.port.unwrap_or(3000);
    let addr = format!("{host}:{port}");
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
