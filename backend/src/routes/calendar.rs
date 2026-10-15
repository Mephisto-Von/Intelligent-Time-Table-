use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::dates::parse_date;
use crate::error::{ApiError, ApiResult};
use crate::models::EventInput;
use crate::state::{id, now, AppState};

#[derive(Debug, Deserialize)]
pub struct CalendarQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

fn event_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "title": r.get::<String, _>("title"),
        "start_date": r.get::<String, _>("start_date"),
        "end_date": r.get::<String, _>("end_date"),
        "kind": r.get::<String, _>("kind"),
        "notes": r.get::<String, _>("notes"),
    })
}

pub async fn list_events(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<CalendarQuery>,
) -> ApiResult<Json<Value>> {
    let from = q.from.unwrap_or_else(|| "0000-01-01".into());
    let to = q.to.unwrap_or_else(|| "9999-12-31".into());
    let rows = sqlx::query(
        "SELECT id, title, start_date, end_date, kind, notes FROM calendar_events
         WHERE school_id = ? AND start_date <= ? AND end_date >= ? ORDER BY start_date",
    )
    .bind(&user.school_id)
    .bind(&to)
    .bind(&from)
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(json!({ "items": rows.iter().map(event_json).collect::<Vec<_>>() })))
}

fn validate_event(input: &EventInput) -> ApiResult<(String, String, String)> {
    if input.title.trim().is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }
    let start = parse_date(&input.start_date)?;
    let end = parse_date(&input.end_date)?;
    if end < start {
        return Err(ApiError::BadRequest("end date must be on or after start date".into()));
    }
    let kind = input.kind.clone().unwrap_or_else(|| "event".into());
    if !["holiday", "exam", "event", "activity"].contains(&kind.as_str()) {
        return Err(ApiError::BadRequest("kind must be holiday, exam, event or activity".into()));
    }
    Ok((start.to_string(), end.to_string(), kind))
}

pub async fn create_event(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<EventInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    let (start, end, kind) = validate_event(&input)?;
    let eid = id();
    sqlx::query(
        "INSERT INTO calendar_events (id, school_id, title, start_date, end_date, kind, notes, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&eid)
    .bind(&user.school_id)
    .bind(input.title.trim())
    .bind(start)
    .bind(end)
    .bind(kind)
    .bind(input.notes.clone().unwrap_or_default())
    .bind(now())
    .execute(&st.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": eid }))))
}

pub async fn update_event(
    State(st): State<AppState>,
    user: AuthUser,
    Path(eid): Path<String>,
    Json(input): Json<EventInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let (start, end, kind) = validate_event(&input)?;
    let n = sqlx::query(
        "UPDATE calendar_events SET title = ?, start_date = ?, end_date = ?, kind = ?, notes = ? WHERE id = ? AND school_id = ?",
    )
    .bind(input.title.trim())
    .bind(start)
    .bind(end)
    .bind(kind)
    .bind(input.notes.clone().unwrap_or_default())
    .bind(&eid)
    .bind(&user.school_id)
    .execute(&st.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_event(
    State(st): State<AppState>,
    user: AuthUser,
    Path(eid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    sqlx::query("DELETE FROM calendar_events WHERE id = ? AND school_id = ?")
        .bind(&eid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
