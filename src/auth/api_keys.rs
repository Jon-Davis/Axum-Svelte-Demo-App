//! API-key service: the bearer-token authentication used by the middleware, plus
//! the list/create/delete operations behind the admin panel. The plaintext token
//! is only ever returned once (at creation); the table stores its SHA-256 hash.
//! SQL lives in [`super::db`].

use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::db;
use crate::error::{Error, Result};

/// A key as shown in the admin panel (never includes the secret).
#[derive(Serialize, sqlx::FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
}

/// The one-time result of creating a key: the id and the plaintext token.
pub struct CreatedKey {
    pub id: Uuid,
    pub token: String,
}

/// Authenticate a bearer token. Returns the key's role if it maps to a live key,
/// and records `last_used_at` in the background (best-effort, errors ignored).
pub async fn authenticate(pool: &PgPool, token: &str) -> Result<Option<String>> {
    let key_hash = hash(token);
    let role = db::find_api_key_role(pool, &key_hash).await?;

    if role.is_some() {
        let pool = pool.clone();
        tokio::spawn(async move {
            let _ = db::touch_api_key(&pool, &key_hash).await;
        });
    }

    Ok(role)
}

/// All keys, newest first.
pub async fn list(pool: &PgPool) -> Result<Vec<ApiKey>> {
    db::list_api_keys(pool).await
}

/// Generate and persist a new key, returning the plaintext token once.
pub async fn create(
    pool: &PgPool,
    name: &str,
    role: &str,
    expires_at: Option<OffsetDateTime>,
) -> Result<CreatedKey> {
    // Reject unknown roles up front — a typo like "Admin" would otherwise create a
    // key that silently fails every `is_admin()` check. The DB CHECK constraint is
    // the backstop; this returns a clean 400.
    if !super::VALID_ROLES.contains(&role) {
        return Err(Error::BadRequest(format!(
            "invalid role: must be one of {:?}",
            super::VALID_ROLES
        )));
    }

    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = format!("svrt_{}", hex::encode(raw));
    let key_hash = hash(&token);
    let id = Uuid::new_v4();

    db::insert_api_key(pool, id, name, role, &key_hash, expires_at).await?;

    Ok(CreatedKey { id, token })
}

/// Revoke a key by id.
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    db::delete_api_key(pool, id).await
}

fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
