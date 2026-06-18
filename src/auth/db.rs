//! All SQL for the `auth` module. Service modules (`sessions`, `api_keys`,
//! `users`) call into these functions; no `sqlx` query lives anywhere else in
//! the module. Keeping the queries together makes the schema surface easy to
//! audit and the service code easy to read.

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::api_keys::ApiKey;
use super::sessions::SessionIdentity;
use crate::error::Result;

// --- sessions ---

pub async fn insert_session(
    pool: &PgPool,
    id: &str,
    user_sub: &str,
    email: &str,
    username: Option<&str>,
    role: &str,
    expires_at: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO sessions (id, user_sub, email, username, role, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(user_sub)
    .bind(email)
    .bind(username)
    .bind(role)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_session(pool: &PgPool, id: &str) -> Result<Option<SessionIdentity>> {
    let row = sqlx::query_as::<_, SessionIdentity>(
        "SELECT role, email, username FROM sessions \
         WHERE id = $1 AND expires_at > NOW()",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_session(pool: &PgPool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete all expired sessions; returns how many rows were removed.
pub async fn delete_expired_sessions(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

// --- users ---

pub async fn upsert_user_returning_role(pool: &PgPool, sub: &str, email: &str) -> Result<String> {
    let (role,) = sqlx::query_as::<_, (String,)>(
        "INSERT INTO users (sub, email) VALUES ($1, $2)
         ON CONFLICT (sub) DO UPDATE SET email = EXCLUDED.email
         RETURNING role",
    )
    .bind(sub)
    .bind(email)
    .fetch_one(pool)
    .await?;
    Ok(role)
}

// --- api keys ---

pub async fn find_api_key_role(pool: &PgPool, key_hash: &str) -> Result<Option<String>> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT role FROM api_keys \
         WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(role,)| role))
}

pub async fn touch_api_key(pool: &PgPool, key_hash: &str) -> Result<()> {
    sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE key_hash = $1")
        .bind(key_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_api_keys(pool: &PgPool) -> Result<Vec<ApiKey>> {
    let keys = sqlx::query_as::<_, ApiKey>(
        "SELECT id, name, role, created_at, expires_at, last_used_at \
         FROM api_keys ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(keys)
}

pub async fn insert_api_key(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    role: &str,
    key_hash: &str,
    expires_at: Option<OffsetDateTime>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO api_keys (id, name, role, key_hash, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(name)
    .bind(role)
    .bind(key_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_api_key(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM api_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
