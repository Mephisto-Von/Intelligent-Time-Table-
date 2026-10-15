use axum::extract::State;
use axum::Json;
use serde_json::json;
use sqlx::Row;

use crate::auth::{hash_password, make_token, verify_password, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::models::{LoginInput, SignupInput};
use crate::routes::setup::seed_default_bell_schedule;
use crate::state::{id, now, AppState};

pub async fn signup(
    State(st): State<AppState>,
    Json(input): Json<SignupInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::BadRequest("a valid email address is required".into()));
    }
    if input.password.len() < 6 {
        return Err(ApiError::BadRequest("password must be at least 6 characters".into()));
    }
    let existing = sqlx::query("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&st.pool)
        .await?;
    if existing.is_some() {
        return Err(ApiError::Conflict("an account with this email already exists".into()));
    }

    let account_id = id();
    let school_id = id();
    let user_id = id();
    let ts = now();
    let hash = hash_password(&input.password)?;
    let mut tx = st.pool.begin().await?;
    sqlx::query("INSERT INTO accounts (id, name, created_at) VALUES (?, ?, ?)")
        .bind(&account_id)
        .bind(input.account_name.trim())
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO schools (id, account_id, name, cycle_weeks, created_at) VALUES (?, ?, ?, 1, ?)")
        .bind(&school_id)
        .bind(&account_id)
        .bind(input.school_name.trim())
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, account_id, school_id, email, password_hash, name, role, created_at) VALUES (?, ?, ?, ?, ?, ?, 'admin', ?)",
    )
    .bind(&user_id)
    .bind(&account_id)
    .bind(&school_id)
    .bind(&email)
    .bind(&hash)
    .bind(input.name.trim())
    .bind(&ts)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    seed_default_bell_schedule(&st, &school_id).await?;

    let token = make_token(&st, &user_id, &account_id, &school_id, "admin")?;
    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user_id,
            "account_id": account_id,
            "school_id": school_id,
            "email": email,
            "name": input.name.trim(),
            "role": "admin",
            "teacher_id": null,
            "class_id": null,
        }
    })))
}

pub async fn login(
    State(st): State<AppState>,
    Json(input): Json<LoginInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let email = input.email.trim().to_lowercase();
    let row = sqlx::query(
        "SELECT id, account_id, school_id, email, password_hash, name, role, teacher_id, class_id FROM users WHERE email = ?",
    )
    .bind(&email)
    .fetch_optional(&st.pool)
    .await?;
    let r = match row {
        Some(r) => r,
        None => return Err(ApiError::Unauthorized),
    };
    let hash: String = r.get("password_hash");
    if !verify_password(&input.password, &hash) {
        return Err(ApiError::Unauthorized);
    }
    let user_id: String = r.get("id");
    let account_id: String = r.get("account_id");
    let school_id: String = r.get("school_id");
    let role: String = r.get("role");
    let token = make_token(&st, &user_id, &account_id, &school_id, &role)?;
    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user_id,
            "account_id": account_id,
            "school_id": school_id,
            "email": r.get::<String, _>("email"),
            "name": r.get::<String, _>("name"),
            "role": role,
            "teacher_id": r.get::<Option<String>, _>("teacher_id"),
            "class_id": r.get::<Option<String>, _>("class_id"),
        }
    })))
}

pub async fn me(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let school = sqlx::query("SELECT name, cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_optional(&st.pool)
        .await?;
    let (school_name, cycle_weeks) = match school {
        Some(r) => (r.get::<String, _>("name"), r.get::<i64, _>("cycle_weeks")),
        None => return Err(ApiError::Unauthorized),
    };
    Ok(Json(json!({
        "user": user.to_json(),
        "school": { "id": user.school_id, "name": school_name, "cycle_weeks": cycle_weeks },
    })))
}
