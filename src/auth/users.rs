//! Persistent user records. Upserted on every OIDC login so email stays current
//! while an admin-assigned role is preserved. SQL lives in [`super::db`].

use sqlx::PgPool;

use super::db;
use crate::error::Result;

/// Upsert the user keyed by their OIDC `sub` and return their current role.
/// The upsert only refreshes the email, so a role an admin set earlier is kept.
pub async fn upsert_and_get_role(pool: &PgPool, sub: &str, email: &str) -> Result<String> {
    db::upsert_user_returning_role(pool, sub, email).await
}
