//! Health checks: a liveness probe (the process is up and serving) and a
//! readiness probe (dependencies — currently Postgres — are reachable). The
//! route handlers in `src/routes/health/` stay thin and call into here.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::timeout;

/// How long the readiness probe waits for the database before giving up. Bounds
/// the probe so a hung or unreachable DB returns 503 promptly instead of leaving
/// the request (and the orchestrator polling it) hanging.
const DB_PING_TIMEOUT: Duration = Duration::from_secs(2);

/// Whether the database answers a trivial query within [`DB_PING_TIMEOUT`]. The
/// readiness probe reports 503 while this is false so a load balancer stops
/// sending traffic until the DB is reachable again. A timeout counts as not
/// ready, same as an error.
pub async fn db_ready(pool: &PgPool) -> bool {
    matches!(
        timeout(DB_PING_TIMEOUT, sqlx::query("SELECT 1").execute(pool)).await,
        Ok(Ok(_))
    )
}
