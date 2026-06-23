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

pub struct AppState {
    pub db: PgPool,
    pub oidc: CoreClient,
    pub cookie_key: Key,
}

// The router is parameterized over `&'static AppState` (the state is `Box::leak`ed
// once in `main`), so the `State` clone axum does per request is a pointer copy
// rather than a deep clone of the `CoreClient`/`Key`. `FromRef` is therefore
// implemented for the reference type.
impl FromRef<&'static AppState> for Key {
    fn from_ref(state: &&'static AppState) -> Self {
        state.cookie_key.clone()
    }
}

// The `openapi` flag makes the macro emit `ApiRouter::openapi()` (a
// `utoipa::openapi::OpenApi` built from the route tree) alongside the router. It
// names each handler's schema/param type by the tokens written in the handler
// signature, so every such type must be *nameable at this site*. Service-owned
// DTOs come straight from their module; DTOs declared inside a `route.rs` live
// under the router's generated module tree (`__folder_router__<struct>`), so we
// pull them in from there.
use auth::api_keys; // ApiKey, via `Vec<api_keys::ApiKey>` on GET /api/admin/api_keys
use __folder_router__apirouter::{
    api::admin::api_keys::route::{CreateRequest, CreateResponse},
    api::hello::route::{HelloParams, HelloResponse},
    api::me::route::UserInfo,
};

#[folder_router("./src/routes", &'static AppState, openapi)]
struct ApiRouter();

/// SvelteKit static-adapter output directory, relative to the workspace root.
/// Single source of truth: [`fallback`](routes::fallback) serves it at runtime,
/// and `server`'s `build.rs` passes it to the `svelte-rust` glue (the lib has no
/// build script of its own, so a `cargo:rustc-env` var couldn't reach this crate
/// — a plain `const` is the cross-crate-safe way to share it).
pub const BUILD_DIR: &str = "build";

/// Build the OpenAPI document for the whole route tree. Shared by the
/// `/api/docs/openapi.json` handler, the golden-file test (`openapi_golden`
/// below), and `server`'s `build.rs` (which links this crate as a
/// build-dependency to regenerate `openapi.json` at build time) so the served
/// spec and the committed `openapi.json` can never drift.
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    let mut doc = ApiRouter::openapi();
    doc.info = utoipa::openapi::InfoBuilder::new()
        .title("svelte-rust-test API")
        .version(env!("CARGO_PKG_VERSION"))
        .build();
    doc
}

/// Process entry point, called by the `server` binary's `main`. Handles the
/// `dump-openapi` and `migrate` subcommands, otherwise starts the HTTP server.
pub async fn run() {
    // `dump-openapi` writes the OpenAPI document to `openapi.json` and exits.
    // The build pipeline regenerates the spec from the compiled route tree — no
    // database or config required, so it must run before any of that setup. The
    // `openapi_golden` test guards drift in CI. (The same document is also
    // emitted at build time by `server/build.rs`.)
    if std::env::args().nth(1).as_deref() == Some("dump-openapi") {
        let json = openapi_document()
            .to_pretty_json()
            .expect("serialize OpenAPI document");
        std::fs::write("openapi.json", format!("{json}\n")).expect("write openapi.json");
        println!("wrote openapi.json");
        return;
    }

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

    // Leak the state once so every clone the router/handlers make is a cheap
    // pointer copy. It lives for the whole process, so there's nothing to free.
    let state: &'static AppState = Box::leak(Box::new(AppState {
        db,
        oidc,
        cookie_key: config.cookie_key,
    }));

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

#[cfg(test)]
mod openapi_golden {
    /// Drift guard for the committed OpenAPI spec.
    ///
    /// Compares the route tree's current spec against the committed
    /// `openapi.json` (the source of truth the frontend's `openapi-ts` client
    /// generation consumes) and fails if they differ — so a stale check-in can't pass
    /// CI. Regeneration is a separate, deterministic step (`server/build.rs` at
    /// build time, or `cargo run -p server -- dump-openapi` by hand). This test
    /// only reads, so `cargo build`/`check` stay codegen-free.
    #[test]
    fn openapi_json_is_current() {
        let generated = crate::openapi_document()
            .to_pretty_json()
            .expect("serialize OpenAPI document");

        let current = std::fs::read_to_string("openapi.json").unwrap_or_default();
        assert!(
            current.trim_end() == generated.trim_end(),
            "openapi.json is out of date — regenerate with `cargo run -p server -- dump-openapi` and commit."
        );
    }
}
