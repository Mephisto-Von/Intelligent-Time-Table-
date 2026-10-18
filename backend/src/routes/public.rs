//! Shareable, read-only timetable links.
//!
//! Staff create a link for a class, teacher or room; the link carries an opaque
//! token that grants anonymous read access to just that timetable — useful for
//! embedding on a school website.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::{id, now, AppState};

#[derive(Debug, Deserialize)]
pub struct LinkInput {
    pub entity: String,
    pub entity_id: String,
    pub label: Option<String>,
}

fn table_for(entity: &str) -> Option<&'static str> {
    match entity {
        "class" => Some("classes"),
        "teacher" => Some("teachers"),
        "room" => Some("rooms"),
        _ => None,
    }
}

async fn entity_name(
    pool: &SqlitePool,
    school_id: &str,
    entity: &str,
    entity_id: &str,
) -> Option<String> {
    let table = table_for(entity)?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT name FROM {table} WHERE id = ? AND school_id = ?"
    )))
    .bind(entity_id)
    .bind(school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| r.get::<String, _>("name"))
}

fn link_json(
    row: &sqlx::sqlite::SqliteRow,
    entity_name: Option<String>,
) -> Value {
    let token: String = row.get("token");
    let id_: String = row.get("id");
    json!({
        "id": id_,
        "entity": row.get::<String, _>("entity"),
        "entity_id": row.get::<String, _>("entity_id"),
        "entity_name": entity_name,
        "label": row.get::<String, _>("label"),
        "token": token,
        "path": format!("/public/{token}"),
        "created_at": row.get::<String, _>("created_at"),
    })
}

pub async fn list_links(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let rows = sqlx::query(
        "SELECT id, entity, entity_id, token, label, created_at FROM public_links
         WHERE school_id = ? ORDER BY created_at DESC",
    )
    .bind(&user.school_id)
    .fetch_all(&st.pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for r in &rows {
        let entity: String = r.get("entity");
        let entity_id: String = r.get("entity_id");
        let name = entity_name(&st.pool, &user.school_id, &entity, &entity_id).await;
        items.push(link_json(r, name));
    }
    Ok(Json(json!({ "items": items })))
}

pub async fn create_link(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<LinkInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if table_for(&input.entity).is_none() {
        return Err(ApiError::BadRequest(
            "entity must be class, teacher or room".into(),
        ));
    }
    let name = entity_name(&st.pool, &user.school_id, &input.entity, &input.entity_id)
        .await
        .ok_or_else(|| ApiError::BadRequest("that timetable does not exist".into()))?;

    let existing = sqlx::query(
        "SELECT id, entity, entity_id, token, label, created_at FROM public_links
         WHERE school_id = ? AND entity = ? AND entity_id = ?",
    )
    .bind(&user.school_id)
    .bind(&input.entity)
    .bind(&input.entity_id)
    .fetch_optional(&st.pool)
    .await?;
    if let Some(row) = existing {
        return Ok((StatusCode::OK, Json(link_json(&row, Some(name)))));
    }

    let lid = id();
    let token = uuid::Uuid::new_v4().simple().to_string();
    let label = input.label.unwrap_or_default();
    sqlx::query(
        "INSERT INTO public_links (id, school_id, entity, entity_id, token, label, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&lid)
    .bind(&user.school_id)
    .bind(&input.entity)
    .bind(&input.entity_id)
    .bind(&token)
    .bind(&label)
    .bind(now())
    .execute(&st.pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, entity, entity_id, token, label, created_at FROM public_links WHERE id = ?",
    )
    .bind(&lid)
    .fetch_one(&st.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(link_json(&row, Some(name)))))
}

pub async fn delete_link(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    sqlx::query("DELETE FROM public_links WHERE id = ? AND school_id = ?")
        .bind(&lid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

fn entry_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "lesson_id": r.get::<String, _>("lesson_id"),
        "week": r.get::<i64, _>("week"),
        "day": r.get::<i64, _>("day"),
        "idx": r.get::<i64, _>("idx"),
        "room_id": r.get::<Option<String>, _>("room_id"),
        "locked": r.get::<i64, _>("locked") != 0,
        "subject_id": r.get::<String, _>("subject_id"),
        "subject_name": r.get::<String, _>("subject_name"),
        "subject_code": r.get::<String, _>("subject_code"),
        "subject_color": r.get::<String, _>("subject_color"),
        "teacher_id": r.get::<String, _>("teacher_id"),
        "teacher_name": r.get::<String, _>("teacher_name"),
        "teacher_initials": r.get::<String, _>("teacher_initials"),
        "class_id": r.get::<String, _>("class_id"),
        "class_name": r.get::<String, _>("class_name"),
        "room_name": r.get::<Option<String>, _>("room_name"),
    })
}

const ENTRY_SELECT: &str = "SELECT e.id, e.lesson_id, e.week, e.day, e.idx, e.room_id, e.locked,
        l.subject_id, l.teacher_id, l.class_id,
        s.name AS subject_name, s.code AS subject_code, s.color AS subject_color,
        t.name AS teacher_name, t.initials AS teacher_initials,
        c.name AS class_name, r.name AS room_name
 FROM timetable_entries e
 JOIN lessons l ON l.id = e.lesson_id
 JOIN subjects s ON s.id = l.subject_id
 JOIN teachers t ON t.id = l.teacher_id
 JOIN classes c ON c.id = l.class_id
 LEFT JOIN rooms r ON r.id = e.room_id";

/// Public, unauthenticated read of a shared timetable.
pub async fn public_timetable(
    State(st): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    let link = sqlx::query(
        "SELECT id, school_id, entity, entity_id, label FROM public_links WHERE token = ?",
    )
    .bind(&token)
    .fetch_optional(&st.pool)
    .await?
    .ok_or(ApiError::NotFound)?;

    let school_id: String = link.get("school_id");
    let entity: String = link.get("entity");
    let entity_id: String = link.get("entity_id");

    let school = sqlx::query("SELECT name, cycle_weeks FROM schools WHERE id = ?")
        .bind(&school_id)
        .fetch_one(&st.pool)
        .await?;

    let filter_col = match entity.as_str() {
        "teacher" => "l.teacher_id",
        "room" => "e.room_id",
        _ => "l.class_id",
    };
    let entries = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{ENTRY_SELECT} WHERE e.school_id = ? AND {filter_col} = ? ORDER BY e.week, e.day, e.idx"
    )))
    .bind(&school_id)
    .bind(&entity_id)
    .fetch_all(&st.pool)
    .await?;

    let slots = sqlx::query(
        "SELECT id, day, idx, start_time, end_time, label, is_break FROM time_slots
         WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(&school_id)
    .fetch_all(&st.pool)
    .await?;
    let slots_json: Vec<Value> = slots
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "day": r.get::<i64, _>("day"),
                "idx": r.get::<i64, _>("idx"),
                "start_time": r.get::<String, _>("start_time"),
                "end_time": r.get::<String, _>("end_time"),
                "label": r.get::<String, _>("label"),
                "is_break": r.get::<i64, _>("is_break") != 0,
            })
        })
        .collect();

    let name = entity_name(&st.pool, &school_id, &entity, &entity_id).await;

    Ok(Json(json!({
        "school_name": school.get::<String, _>("name"),
        "cycle_weeks": school.get::<i64, _>("cycle_weeks"),
        "entity": entity,
        "entity_id": entity_id,
        "entity_name": name,
        "label": link.get::<String, _>("label"),
        "slots": slots_json,
        "entries": entries.iter().map(entry_json).collect::<Vec<_>>(),
    })))
}
