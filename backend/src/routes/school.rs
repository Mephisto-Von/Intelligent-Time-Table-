use std::collections::{HashMap, HashSet};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::{hash_password, AuthUser};
use crate::dates::{day_of, part_covers, today, week_index};
use crate::error::{ApiError, ApiResult};
use crate::models::{SchoolInput, UserInput};
use crate::state::{id, now, AppState};

pub async fn get_school(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    let school = sqlx::query("SELECT id, name, cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    let mut counts = serde_json::Map::new();
    for (key, table) in [
        ("subjects", "subjects"),
        ("teachers", "teachers"),
        ("classes", "classes"),
        ("rooms", "rooms"),
        ("lessons", "lessons"),
        ("entries", "timetable_entries"),
    ] {
        let n: i64 = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) AS n FROM {table} WHERE school_id = ?"
        )))
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
        counts.insert(key.to_string(), json!(n));
    }
    Ok(Json(json!({
        "id": school.get::<String, _>("id"),
        "name": school.get::<String, _>("name"),
        "cycle_weeks": school.get::<i64, _>("cycle_weeks"),
        "counts": counts,
    })))
}

pub async fn update_school(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<SchoolInput>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("school name is required".into()));
    }
    let cycle = input.cycle_weeks.unwrap_or(1).clamp(1, 2);
    let current: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");
    let mut tx = st.pool.begin().await?;
    sqlx::query("UPDATE schools SET name = ?, cycle_weeks = ? WHERE id = ?")
        .bind(input.name.trim())
        .bind(cycle)
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?;
    let mut cleared = 0;
    if cycle != current {
        cleared = sqlx::query("DELETE FROM timetable_entries WHERE school_id = ?")
            .bind(&user.school_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true, "cleared_entries": cleared })))
}

// ---------------------------------------------------------------------------
// users
// ---------------------------------------------------------------------------

pub async fn list_users(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let rows = sqlx::query(
        "SELECT u.id, u.email, u.name, u.role, u.teacher_id, u.class_id, u.created_at,
                t.name AS teacher_name, c.name AS class_name
         FROM users u
         LEFT JOIN teachers t ON t.id = u.teacher_id
         LEFT JOIN classes c ON c.id = u.class_id
         WHERE u.account_id = ? AND u.school_id = ?
         ORDER BY u.role, u.name",
    )
    .bind(&user.account_id)
    .bind(&user.school_id)
    .fetch_all(&st.pool)
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "email": r.get::<String, _>("email"),
                "name": r.get::<String, _>("name"),
                "role": r.get::<String, _>("role"),
                "teacher_id": r.get::<Option<String>, _>("teacher_id"),
                "teacher_name": r.get::<Option<String>, _>("teacher_name"),
                "class_id": r.get::<Option<String>, _>("class_id"),
                "class_name": r.get::<Option<String>, _>("class_name"),
                "created_at": r.get::<String, _>("created_at"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

fn validate_role(role: &str) -> ApiResult<()> {
    if ["admin", "scheduler", "teacher", "student"].contains(&role) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("role must be admin, scheduler, teacher or student".into()))
    }
}

pub async fn create_user(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_admin()?;
    validate_role(&input.role)?;
    let email = input.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(ApiError::BadRequest("a valid email address is required".into()));
    }
    let password = input.password.clone().unwrap_or_default();
    if password.len() < 6 {
        return Err(ApiError::BadRequest("password must be at least 6 characters".into()));
    }
    let exists: i64 = sqlx::query("SELECT COUNT(*) AS n FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if exists > 0 {
        return Err(ApiError::Conflict("a user with this email already exists".into()));
    }
    let teacher_id = if input.role == "teacher" { input.teacher_id.clone() } else { None };
    let class_id = if input.role == "student" { input.class_id.clone() } else { None };
    let uid = id();
    sqlx::query(
        "INSERT INTO users (id, account_id, school_id, email, password_hash, name, role, teacher_id, class_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&uid)
    .bind(&user.account_id)
    .bind(&user.school_id)
    .bind(&email)
    .bind(hash_password(&password)?)
    .bind(input.name.trim())
    .bind(&input.role)
    .bind(&teacher_id)
    .bind(&class_id)
    .bind(now())
    .execute(&st.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": uid }))))
}

pub async fn update_user(
    State(st): State<AppState>,
    user: AuthUser,
    Path(uid): Path<String>,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    validate_role(&input.role)?;
    let teacher_id = if input.role == "teacher" { input.teacher_id.clone() } else { None };
    let class_id = if input.role == "student" { input.class_id.clone() } else { None };
    let n = sqlx::query(
        "UPDATE users SET name = ?, role = ?, teacher_id = ?, class_id = ? WHERE id = ? AND account_id = ?",
    )
    .bind(input.name.trim())
    .bind(&input.role)
    .bind(&teacher_id)
    .bind(&class_id)
    .bind(&uid)
    .bind(&user.account_id)
    .execute(&st.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    if let Some(password) = input.password.clone() {
        if !password.is_empty() {
            if password.len() < 6 {
                return Err(ApiError::BadRequest("password must be at least 6 characters".into()));
            }
            sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
                .bind(hash_password(&password)?)
                .bind(&uid)
                .execute(&st.pool)
                .await?;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_user(
    State(st): State<AppState>,
    user: AuthUser,
    Path(uid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    if uid == user.id {
        return Err(ApiError::BadRequest("you cannot delete your own account".into()));
    }
    let target = sqlx::query("SELECT role FROM users WHERE id = ? AND account_id = ?")
        .bind(&uid)
        .bind(&user.account_id)
        .fetch_optional(&st.pool)
        .await?;
    let Some(target) = target else { return Err(ApiError::NotFound) };
    if target.get::<String, _>("role") == "admin" {
        let admins: i64 = sqlx::query("SELECT COUNT(*) AS n FROM users WHERE account_id = ? AND role = 'admin'")
            .bind(&user.account_id)
            .fetch_one(&st.pool)
            .await?
            .get("n");
        if admins <= 1 {
            return Err(ApiError::BadRequest("the last admin cannot be deleted".into()));
        }
    }
    sqlx::query("DELETE FROM users WHERE id = ? AND account_id = ?")
        .bind(&uid)
        .bind(&user.account_id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// dashboard
// ---------------------------------------------------------------------------

pub async fn dashboard(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    let school_id = &user.school_id;
    let mut counts = serde_json::Map::new();
    for (key, table) in [
        ("subjects", "subjects"),
        ("teachers", "teachers"),
        ("classes", "classes"),
        ("rooms", "rooms"),
        ("lessons", "lessons"),
    ] {
        let n: i64 = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) AS n FROM {table} WHERE school_id = ?"
        )))
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
        counts.insert(key.to_string(), json!(n));
    }

    let requested: i64 = sqlx::query("SELECT COALESCE(SUM(periods_per_week), 0) AS n FROM lessons WHERE school_id = ?")
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    let placed: i64 = sqlx::query("SELECT COUNT(*) AS n FROM timetable_entries WHERE school_id = ?")
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    let pending_leave: i64 = sqlx::query("SELECT COUNT(*) AS n FROM leave_requests WHERE school_id = ? AND status = 'pending'")
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");

    let today_date = today();
    let dstr = today_date.to_string();
    let day = day_of(&today_date);
    let cycle: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");
    let week = week_index(&today_date, cycle);

    let slots = sqlx::query("SELECT idx, is_break FROM time_slots WHERE school_id = ? AND day = ? ORDER BY idx")
        .bind(school_id)
        .bind(day)
        .fetch_all(&st.pool)
        .await?;
    let teaching: Vec<i64> = slots
        .iter()
        .filter(|r| r.get::<i64, _>("is_break") == 0)
        .map(|r| r.get::<i64, _>("idx"))
        .collect();
    let am_max = teaching.get(teaching.len().saturating_sub(1) / 2).copied().unwrap_or(0);

    let mut absences: HashMap<String, String> = HashMap::new();
    let leave_rows = sqlx::query(
        "SELECT teacher_id, part FROM leave_requests WHERE school_id = ? AND status = 'approved' AND start_date <= ? AND end_date >= ?",
    )
    .bind(school_id)
    .bind(&dstr)
    .bind(&dstr)
    .fetch_all(&st.pool)
    .await?;
    for r in &leave_rows {
        let tid: String = r.get("teacher_id");
        let part: String = r.get("part");
        match absences.get(&tid) {
            Some(existing) if existing != &part => {
                absences.insert(tid, "full".to_string());
            }
            None => {
                absences.insert(tid, part);
            }
            _ => {}
        }
    }

    let mut today_sessions = 0i64;
    if !absences.is_empty() && !teaching.is_empty() {
        let entries = sqlx::query(
            "SELECT e.idx, e.lesson_id, l.teacher_id FROM timetable_entries e JOIN lessons l ON l.id = e.lesson_id
             WHERE e.school_id = ? AND e.week = ? AND e.day = ?",
        )
        .bind(school_id)
        .bind(week)
        .bind(day)
        .fetch_all(&st.pool)
        .await?;
        let sub_rows = sqlx::query(
            "SELECT lesson_id FROM substitutions WHERE school_id = ? AND date = ? AND status = 'assigned'",
        )
        .bind(school_id)
        .bind(&dstr)
        .fetch_all(&st.pool)
        .await?;
        let covered: HashSet<String> = sub_rows.iter().map(|r| r.get::<String, _>("lesson_id")).collect();
        for e in &entries {
            let tid: String = e.get("teacher_id");
            let Some(part) = absences.get(&tid) else { continue };
            let idx: i64 = e.get("idx");
            if part_covers(part, idx, am_max) && covered.contains(&e.get::<String, _>("lesson_id")) {
                today_sessions += 1;
            } else if part_covers(part, idx, am_max) {
                today_sessions += 1;
            }
        }
    }
    let covered_today: i64 = {
        let sub_rows: i64 = sqlx::query(
            "SELECT COUNT(*) AS n FROM substitutions WHERE school_id = ? AND date = ? AND status = 'assigned'",
        )
        .bind(school_id)
        .bind(&dstr)
        .fetch_one(&st.pool)
        .await?
        .get("n");
        sub_rows
    };

    let upcoming_rows = sqlx::query(
        "SELECT id, title, start_date, end_date, kind FROM calendar_events WHERE school_id = ? AND end_date >= ? ORDER BY start_date LIMIT 6",
    )
    .bind(school_id)
    .bind(&dstr)
    .fetch_all(&st.pool)
    .await?;
    let upcoming: Vec<Value> = upcoming_rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "title": r.get::<String, _>("title"),
                "start_date": r.get::<String, _>("start_date"),
                "end_date": r.get::<String, _>("end_date"),
                "kind": r.get::<String, _>("kind"),
            })
        })
        .collect();

    let gap_rows = sqlx::query(
        "SELECT l.id, l.periods_per_week, c.name AS class_name, s.name AS subject_name, COUNT(e.id) AS placed
         FROM lessons l
         JOIN classes c ON c.id = l.class_id
         JOIN subjects s ON s.id = l.subject_id
         LEFT JOIN timetable_entries e ON e.lesson_id = l.id
         WHERE l.school_id = ?
         GROUP BY l.id
         HAVING placed < l.periods_per_week
         ORDER BY (l.periods_per_week - placed) DESC
         LIMIT 5",
    )
    .bind(school_id)
    .fetch_all(&st.pool)
    .await?;
    let gaps: Vec<Value> = gap_rows
        .iter()
        .map(|r| {
            json!({
                "lesson_id": r.get::<String, _>("id"),
                "class_name": r.get::<String, _>("class_name"),
                "subject_name": r.get::<String, _>("subject_name"),
                "missing": r.get::<i64, _>("periods_per_week") - r.get::<i64, _>("placed"),
            })
        })
        .collect();

    Ok(Json(json!({
        "school_name": sqlx::query("SELECT name FROM schools WHERE id = ?")
            .bind(school_id)
            .fetch_one(&st.pool)
            .await?
            .get::<String, _>("name"),
        "counts": counts,
        "placed": placed,
        "requested": requested,
        "pending_leave": pending_leave,
        "today": {
            "date": dstr,
            "sessions_due": today_sessions,
            "covered": covered_today,
        },
        "upcoming_events": upcoming,
        "gaps": gaps,
    })))
}
