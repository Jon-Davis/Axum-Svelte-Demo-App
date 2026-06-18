//! Session service: the row that backs a logged-in web user, plus the
//! background reaper that prunes expired rows. SQL lives in [`super::db`].

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::db;
use crate::error::Result;

/// The identity fields carried on a live session, as the middleware needs them.
#[derive(sqlx::FromRow)]
pub struct SessionIdentity {
    pub role: String,
    pub email: String,
    pub username: Option<String>,
}

/// Create a 7-day session for a just-authenticated user; returns its id.
pub async fn create(
    pool: &PgPool,
    user_sub: &str,
    email: &str,
    username: Option<&str>,
    role: &str,
) -> Result<String> {
    let session_id = Uuid::new_v4().to_string();
    let expires_at = OffsetDateTime::now_utc() + Duration::days(7);
    db::insert_session(pool, &session_id, user_sub, email, username, role, expires_at).await?;
    Ok(session_id)
}

/// Look up a live (unexpired) session by id.
pub async fn find(pool: &PgPool, id: &str) -> Result<Option<SessionIdentity>> {
    db::find_session(pool, id).await
}

/// Delete a session (logout).
pub async fn delete(pool: &PgPool, id: &str) -> Result<()> {
    db::delete_session(pool, id).await
}

/// Periodically delete expired sessions. Without this, the `sessions` table grows
/// one row per login forever — expired rows are filtered out of every query but
/// never reclaimed. Runs hourly; the `sessions_expires_at_idx` index keeps the
/// delete cheap.
pub fn spawn_reaper(pool: PgPool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match db::delete_expired_sessions(&pool).await {
                Ok(n) if n > 0 => tracing::info!("reaped {n} expired session(s)"),
                Ok(_) => {}
                Err(e) => tracing::error!("session reaper failed: {e}"),
            }
        }
    });
}
