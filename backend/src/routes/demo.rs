use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::{make_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::seed::{ensure_demo_account, DEMO_EMAIL, DEMO_PASSWORD};
use crate::state::AppState;

async fn token_for_demo_user(st: &AppState, user_id: &str) -> ApiResult<(String, Value, String)> {
    let row = sqlx::query(
        "SELECT account_id, school_id, email, name, role, teacher_id, class_id FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(&st.pool)
    .await?;
    let account_id: String = row.get("account_id");
    let school_id: String = row.get("school_id");
    let role: String = row.get("role");
    let token = make_token(st, user_id, &account_id, &school_id, &role)?;
    let user = json!({
        "id": user_id,
        "account_id": account_id,
        "school_id": school_id,
        "email": row.get::<String, _>("email"),
        "name": row.get::<String, _>("name"),
        "role": role,
        "teacher_id": row.get::<Option<String>, _>("teacher_id"),
        "class_id": row.get::<Option<String>, _>("class_id"),
    });
    Ok((token, user, school_id))
}

/// Public endpoint: create (once) and log straight into the demo school.
pub async fn demo_seed(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let (user_id, school_id, created) = ensure_demo_account(&st)
        .await
        .map_err(|e| ApiError::Internal(format!("demo seed failed: {e}")))?;
    let (token, user, _) = token_for_demo_user(&st, &user_id).await?;
    let email: String = sqlx::query("SELECT email FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_one(&st.pool)
        .await?
        .get("email");
    Ok(Json(json!({
        "token": token,
        "user": user,
        "school_id": school_id,
        "created": created,
        "demo_credentials": { "email": email, "password": DEMO_PASSWORD },
    })))
}

/// Demo admin only: wipe the demo school and rebuild it from scratch.
pub async fn demo_reset(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    let email: String = sqlx::query("SELECT email FROM users WHERE id = ?")
        .bind(&user.id)
        .fetch_one(&st.pool)
        .await?
        .get("email");
    if email != DEMO_EMAIL {
        return Err(ApiError::Forbidden);
    }
    crate::db::wipe_school_data(&st.pool, &user.school_id)
        .await
        .map_err(|e| ApiError::Internal(format!("wipe failed: {e}")))?;
    sqlx::query("DELETE FROM users WHERE account_id = ?")
        .bind(&user.account_id)
        .execute(&st.pool)
        .await?;
    sqlx::query("DELETE FROM schools WHERE account_id = ?")
        .bind(&user.account_id)
        .execute(&st.pool)
        .await?;
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(&user.account_id)
        .execute(&st.pool)
        .await?;

    let (user_id, _, _) = ensure_demo_account(&st)
        .await
        .map_err(|e| ApiError::Internal(format!("demo reseed failed: {e}")))?;
    let (token, user, _) = token_for_demo_user(&st, &user_id).await?;
    Ok(Json(json!({ "token": token, "user": user, "reset": true })))
}
