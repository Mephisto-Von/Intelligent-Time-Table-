use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::dates::{date_range, day_of, parse_date, part_covers, week_index};
use crate::error::{ApiError, ApiResult};
use crate::models::{day_name, LeaveInput, DecideInput};
use crate::state::{id, now, AppState};

#[derive(Debug, Deserialize)]
pub struct LeaveQuery {
    pub status: Option<String>,
    pub teacher_id: Option<String>,
}

pub async fn list_leave(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<LeaveQuery>,
) -> ApiResult<Json<Value>> {
    if user.role == "student" {
        return Err(ApiError::Forbidden);
    }
    let mut sql = String::from(
        "SELECT l.id, l.teacher_id, t.name AS teacher_name, t.initials AS teacher_initials,
                l.start_date, l.end_date, l.part, l.reason, l.status, l.decided_by, l.decided_at, l.created_at
         FROM leave_requests l LEFT JOIN teachers t ON t.id = l.teacher_id
         WHERE l.school_id = ?",
    );
    let mut binds: Vec<String> = vec![user.school_id.clone()];
    if let Some(status) = &q.status {
        sql.push_str(" AND l.status = ?");
        binds.push(status.clone());
    }
    if user.role == "teacher" {
        sql.push_str(" AND l.teacher_id = ?");
        binds.push(user.teacher_id.clone().unwrap_or_default());
    } else if let Some(tid) = &q.teacher_id {
        sql.push_str(" AND l.teacher_id = ?");
        binds.push(tid.clone());
    }
    sql.push_str(" ORDER BY l.start_date DESC, l.created_at DESC LIMIT 500");

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for b in &binds {
        query = query.bind(b);
    }
    let rows = query.fetch_all(&st.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            let start = r.get::<String, _>("start_date");
            let end = r.get::<String, _>("end_date");
            let days = (parse_date(&end).ok(), parse_date(&start).ok());
            let n_days = match days {
                (Some(e), Some(s)) => (e - s).num_days() + 1,
                _ => 1,
            };
            json!({
                "id": r.get::<String, _>("id"),
                "teacher_id": r.get::<String, _>("teacher_id"),
                "teacher_name": r.get::<Option<String>, _>("teacher_name").unwrap_or_else(|| "(removed)".into()),
                "teacher_initials": r.get::<Option<String>, _>("teacher_initials").unwrap_or_default(),
                "start_date": start,
                "end_date": end,
                "days": n_days,
                "part": r.get::<String, _>("part"),
                "reason": r.get::<String, _>("reason"),
                "status": r.get::<String, _>("status"),
                "decided_at": r.get::<Option<String>, _>("decided_at"),
                "created_at": r.get::<String, _>("created_at"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

pub async fn create_leave(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<LeaveInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let teacher_id = if user.is_staff() {
        input.teacher_id.clone().or_else(|| user.teacher_id.clone())
    } else {
        user.teacher_id.clone()
    };
    let Some(teacher_id) = teacher_id else {
        return Err(ApiError::BadRequest("no teacher linked to this account".into()));
    };
    let exists: i64 = sqlx::query("SELECT COUNT(*) AS n FROM teachers WHERE id = ? AND school_id = ?")
        .bind(&teacher_id)
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if exists == 0 {
        return Err(ApiError::BadRequest("teacher not found".into()));
    }
    let start = parse_date(&input.start_date)?;
    let end = parse_date(&input.end_date)?;
    let _ = date_range(&start, &end, 90)?;
    let part = input.part.unwrap_or_else(|| "full".into());
    if !["full", "am", "pm"].contains(&part.as_str()) {
        return Err(ApiError::BadRequest("part must be full, am or pm".into()));
    }
    let lid = id();
    sqlx::query(
        "INSERT INTO leave_requests (id, school_id, teacher_id, start_date, end_date, part, reason, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)",
    )
    .bind(&lid)
    .bind(&user.school_id)
    .bind(&teacher_id)
    .bind(start.to_string())
    .bind(end.to_string())
    .bind(&part)
    .bind(input.reason.clone().unwrap_or_default())
    .bind(now())
    .execute(&st.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": lid }))))
}

pub async fn decide_leave(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
    Json(input): Json<DecideInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let status = if input.approve { "approved" } else { "rejected" };
    let n = sqlx::query(
        "UPDATE leave_requests SET status = ?, decided_by = ?, decided_at = ? WHERE id = ? AND school_id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(&user.id)
    .bind(now())
    .bind(&lid)
    .bind(&user.school_id)
    .execute(&st.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::BadRequest("request not found or already decided".into()));
    }
    Ok(Json(json!({ "ok": true, "status": status })))
}

pub async fn cancel_leave(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
) -> ApiResult<Json<Value>> {
    let row = sqlx::query("SELECT teacher_id, status FROM leave_requests WHERE id = ? AND school_id = ?")
        .bind(&lid)
        .bind(&user.school_id)
        .fetch_optional(&st.pool)
        .await?;
    let Some(row) = row else { return Err(ApiError::NotFound) };
    let owner: String = row.get("teacher_id");
    if !user.is_staff() && Some(&owner) != user.teacher_id.as_ref() {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("UPDATE leave_requests SET status = 'cancelled' WHERE id = ? AND school_id = ?")
        .bind(&lid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Periods affected if this leave goes ahead (or already did).
pub async fn leave_impact(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
) -> ApiResult<Json<Value>> {
    if user.role == "student" {
        return Err(ApiError::Forbidden);
    }
    let leave = sqlx::query(
        "SELECT teacher_id, start_date, end_date, part, status FROM leave_requests WHERE id = ? AND school_id = ?",
    )
    .bind(&lid)
    .bind(&user.school_id)
    .fetch_optional(&st.pool)
    .await?;
    let Some(leave) = leave else { return Err(ApiError::NotFound) };
    let teacher_id: String = leave.get("teacher_id");
    let part: String = leave.get("part");
    let start = parse_date(&leave.get::<String, _>("start_date"))?;
    let end = parse_date(&leave.get::<String, _>("end_date"))?;
    let dates = date_range(&start, &end, 90)?;

    let cycle: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");

    // school days + am cut-off per day
    let slot_rows = sqlx::query(
        "SELECT day, idx, start_time, end_time, label, is_break FROM time_slots WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(&user.school_id)
    .fetch_all(&st.pool)
    .await?;
    let mut day_idxs: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for r in &slot_rows {
        if r.get::<i64, _>("is_break") == 0 {
            day_idxs
                .entry(r.get::<i64, _>("day"))
                .or_default()
                .push(r.get::<i64, _>("idx"));
        }
    }

    let entries = sqlx::query(
        "SELECT e.idx, e.day, e.week, e.lesson_id, s.name AS subject_name, s.code AS subject_code,
                c.name AS class_name, r.name AS room_name
         FROM timetable_entries e
         JOIN lessons l ON l.id = e.lesson_id
         JOIN subjects s ON s.id = l.subject_id
         JOIN classes c ON c.id = l.class_id
         LEFT JOIN rooms r ON r.id = e.room_id
         WHERE e.school_id = ? AND l.teacher_id = ?",
    )
    .bind(&user.school_id)
    .bind(&teacher_id)
    .fetch_all(&st.pool)
    .await?;

    let holidays = sqlx::query("SELECT title, start_date, end_date FROM calendar_events WHERE school_id = ? AND kind = 'holiday'")
        .bind(&user.school_id)
        .fetch_all(&st.pool)
        .await?;

    let mut sessions: Vec<Value> = Vec::new();
    for date in dates {
        let day = day_of(&date);
        let week = week_index(&date, cycle);
        let dstr = date.to_string();
        let on_holiday = holidays.iter().any(|h| {
            let s: String = h.get("start_date");
            let e: String = h.get("end_date");
            dstr >= s && dstr <= e
        });
        let Some(idxs) = day_idxs.get(&day) else { continue };
        if on_holiday {
            continue;
        }
        let am_max = idxs[idxs.len().saturating_sub(1) / 2];
        for e in &entries {
            let idx: i64 = e.get("idx");
            if e.get::<i64, _>("day") != day || e.get::<i64, _>("week") != week {
                continue;
            }
            if !part_covers(&part, idx, am_max) {
                continue;
            }
            let sub = sqlx::query(
                "SELECT s.id, t.name AS substitute_name FROM substitutions s JOIN teachers t ON t.id = s.substitute_teacher_id
                 WHERE s.school_id = ? AND s.date = ? AND s.lesson_id = ? AND s.status = 'assigned'",
            )
            .bind(&user.school_id)
            .bind(&dstr)
            .bind(e.get::<String, _>("lesson_id"))
            .fetch_optional(&st.pool)
            .await?;
            sessions.push(json!({
                "date": dstr,
                "day": day,
                "day_name": day_name(day),
                "idx": idx,
                "lesson_id": e.get::<String, _>("lesson_id"),
                "subject_name": e.get::<String, _>("subject_name"),
                "subject_code": e.get::<String, _>("subject_code"),
                "class_name": e.get::<String, _>("class_name"),
                "room_name": e.get::<Option<String>, _>("room_name"),
                "substitution": sub.map(|s| json!({ "id": s.get::<String, _>("id"), "substitute_name": s.get::<String, _>("substitute_name") })),
            }));
        }
    }

    Ok(Json(json!({
        "leave_id": lid,
        "teacher_id": teacher_id,
        "part": part,
        "sessions": sessions,
    })))
}
