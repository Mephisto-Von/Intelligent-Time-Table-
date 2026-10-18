//! Append-only activity log. Writes are best-effort: a failed audit insert is
//! logged but never fails the request that triggered it.

use sqlx::SqlitePool;

use crate::auth::AuthUser;
use crate::state::{id, now};

/// Records a user-initiated change.
pub async fn record(
    pool: &SqlitePool,
    user: &AuthUser,
    action: &str,
    entity: &str,
    entity_id: Option<&str>,
    summary: impl Into<String>,
) {
    let res = sqlx::query(
        "INSERT INTO audit_log (id, school_id, actor_id, actor_name, action, entity, entity_id, summary, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id())
    .bind(&user.school_id)
    .bind(&user.id)
    .bind(&user.name)
    .bind(action)
    .bind(entity)
    .bind(entity_id)
    .bind(summary.into())
    .bind(now())
    .execute(pool)
    .await;
    if let Err(e) = res {
        tracing::warn!("audit write failed: {e}");
    }
}
