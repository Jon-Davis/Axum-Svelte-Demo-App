//! Application configuration: read every setting from the environment, validate
//! it, and convert each value to the concrete type the rest of the app uses.
//! `main` receives a ready-to-use `Config` or a clear error — no env parsing and
//! no `expect`s on env values live anywhere else.

use std::str::FromStr;

use axum_extra::extract::cookie::Key;
use openidconnect::{ClientId, ClientSecret, IssuerUrl, RedirectUrl, url};
use serde::Deserialize;
use sqlx::postgres::PgConnectOptions;

/// `Key::from` panics on a secret shorter than 64 bytes; we check for it here so
/// a misconfiguration fails with a clear message instead of an opaque panic.
const MIN_SESSION_SECRET_LEN: usize = 64;

/// Raw strings as they arrive from the environment, before validation. Only
/// [`Config::from_env`] constructs this; everything else uses the typed [`Config`].
#[derive(Deserialize)]
struct RawConfig {
    database_url: String,
    oidc_issuer: String,
    oidc_client_id: String,
    oidc_client_secret: String,
    oidc_redirect_uri: String,
    session_secret: String,
    host: Option<String>,
    port: Option<u16>,
}

/// Validated, fully-typed configuration the application runs on. Every field is
/// the concrete type its consumer wants, so wiring in `main` is just moves.
pub struct Config {
    pub database: PgConnectOptions,
    pub oidc_issuer: IssuerUrl,
    pub oidc_client_id: ClientId,
    pub oidc_client_secret: ClientSecret,
    pub oidc_redirect_uri: RedirectUrl,
    pub cookie_key: Key,
    pub host: String,
    pub port: u16,
}

/// Everything that can go wrong turning the environment into a [`Config`].
/// Each variant names the offending variable so the operator can fix it.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing or malformed environment variable(s): {0}")]
    Env(#[from] envy::Error),

    #[error("DATABASE_URL is not a valid Postgres connection string: {0}")]
    DatabaseUrl(sqlx::Error),

    #[error("OIDC_ISSUER is not a valid URL: {0}")]
    OidcIssuer(url::ParseError),

    #[error("OIDC_REDIRECT_URI is not a valid URL: {0}")]
    OidcRedirectUri(url::ParseError),

    #[error("SESSION_SECRET must be at least {MIN_SESSION_SECRET_LEN} bytes (got {0})")]
    SessionSecretTooShort(usize),
}

impl Config {
    /// Load and validate configuration from the process environment. In debug
    /// builds a local `.env` file is read first (via `dotenvy`); in release the
    /// environment must be set directly.
    pub fn from_env() -> Result<Self, ConfigError> {
        #[cfg(debug_assertions)]
        dotenvy::dotenv().ok();

        let raw: RawConfig = envy::from_env()?;

        // Parsing the connection string here surfaces a bad URL at startup rather
        // than as a cryptic failure on the first connect.
        let database =
            PgConnectOptions::from_str(&raw.database_url).map_err(ConfigError::DatabaseUrl)?;

        // The OIDC URL newtypes validate the URL as part of construction.
        let oidc_issuer = IssuerUrl::new(raw.oidc_issuer).map_err(ConfigError::OidcIssuer)?;
        let oidc_redirect_uri =
            RedirectUrl::new(raw.oidc_redirect_uri).map_err(ConfigError::OidcRedirectUri)?;

        if raw.session_secret.len() < MIN_SESSION_SECRET_LEN {
            return Err(ConfigError::SessionSecretTooShort(raw.session_secret.len()));
        }
        let cookie_key = Key::from(raw.session_secret.as_bytes());

        Ok(Config {
            database,
            oidc_issuer,
            oidc_client_id: ClientId::new(raw.oidc_client_id),
            oidc_client_secret: ClientSecret::new(raw.oidc_client_secret),
            oidc_redirect_uri,
            cookie_key,
            host: raw.host.unwrap_or_else(|| "127.0.0.1".to_string()),
            port: raw.port.unwrap_or(3000),
        })
    }
}
