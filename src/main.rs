mod auth;
mod config;
mod error;
mod health;

use std::net::SocketAddr;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use axum_folder_router::folder_router;
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata},
    reqwest::async_http_client,
};
use sqlx::PgPool;

use config::Config;

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

#[tokio::main]
async fn main() {
    // Load config before tracing so a `.env` `RUST_LOG` is in place when the
    // subscriber reads the environment.
    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    tracing_subscriber::fmt::init();

    let db = PgPool::connect_with(config.database)
        .await
        .expect("failed to connect to database");

    // Migrations are a manual, opt-in step: `svelte-rust-test migrate` applies
    // them and exits. Normal startup never touches the schema, so a deploy can't
    // silently migrate the database.
    if std::env::args().nth(1).as_deref() == Some("migrate") {
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("failed to run migrations");
        tracing::info!("migrations applied");
        return;
    }

    auth::sessions::spawn_reaper(db.clone());

    let provider_metadata =
        CoreProviderMetadata::discover_async(config.oidc_issuer, async_http_client)
            .await
            .expect("failed to discover OIDC provider — is Dex running?");

    let oidc = CoreClient::from_provider_metadata(
        provider_metadata,
        config.oidc_client_id,
        Some(config.oidc_client_secret),
    )
    .set_redirect_uri(config.oidc_redirect_uri);

    let state = AppState {
        db,
        oidc,
        cookie_key: config.cookie_key,
    };

    // The whole router is assembled from the `src/routes` tree: global layers
    // (root `middleware.rs`), `/auth` rate limiting, `/api` auth, `/api/admin`
    // authorization, and the static-app fallback (with its session guard) are
    // all wired on by `middleware.rs`/`fallback.rs` files.
    // `into_router_with_state` threads the state into each of them.
    let app = ApiRouter::into_router_with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    // `ConnectInfo` makes the peer address available to the `/auth` rate
    // limiter's IP key extractor when no proxy headers are present.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();

    tracing::info!("server stopped");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl+C, shutting down"),
        _ = sigterm => tracing::info!("received SIGTERM, shutting down"),
    }
}
