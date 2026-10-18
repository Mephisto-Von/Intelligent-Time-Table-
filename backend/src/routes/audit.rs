use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
    pub entity: Option<String>,
}

pub async fn list_audit(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let mut sql = String::from(
        "SELECT id, actor_id, actor_name, action, entity, entity_id, summary, created_at
         FROM audit_log WHERE school_id = ?",
    );
    let mut binds: Vec<String> = vec![user.school_id.clone()];
    if let Some(entity) = &q.entity {
        sql.push_str(" AND entity = ?");
        binds.push(entity.clone());
    }
    sql.push_str(" ORDER BY created_at DESC, rowid DESC LIMIT ?");

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for b in &binds {
        query = query.bind(b);
    }
    let rows = query.bind(limit).fetch_all(&st.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "actor_id": r.get::<Option<String>, _>("actor_id"),
                "actor_name": r.get::<String, _>("actor_name"),
                "action": r.get::<String, _>("action"),
                "entity": r.get::<String, _>("entity"),
                "entity_id": r.get::<Option<String>, _>("entity_id"),
                "summary": r.get::<String, _>("summary"),
                "created_at": r.get::<String, _>("created_at"),
            })
        })
        .collect();

    Ok(Json(json!({ "items": items })))
}
