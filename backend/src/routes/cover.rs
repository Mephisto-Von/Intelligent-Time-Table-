use std::collections::{HashMap, HashSet};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::dates::{day_of, parse_date, part_covers, today, week_index};
use crate::error::{ApiError, ApiResult};
use crate::models::{day_name, CoverAssignInput};
use crate::state::{id, now, AppState};

#[derive(Debug, Deserialize)]
pub struct CoverQuery {
    pub date: Option<String>,
}

struct Absence {
    teacher_id: String,
    name: String,
    initials: String,
    part: String,
    leave_id: String,
}

fn merge_absence(parts: &mut HashMap<String, String>, teacher_id: &str, part: &str) {
    let entry = parts.entry(teacher_id.to_string()).or_insert_with(|| part.to_string());
    if *entry != part {
        *entry = "full".to_string();
    }
}

async fn day_slots(
    st: &AppState,
    school_id: &str,
    day: i64,
) -> ApiResult<Vec<(i64, String, String, bool)>> {
    let rows = sqlx::query(
        "SELECT idx, start_time, end_time, is_break FROM time_slots WHERE school_id = ? AND day = ? ORDER BY idx",
    )
    .bind(school_id)
    .bind(day)
    .fetch_all(&st.pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get::<i64, _>("idx"),
                r.get::<String, _>("start_time"),
                r.get::<String, _>("end_time"),
                r.get::<i64, _>("is_break") != 0,
            )
        })
        .collect())
}

pub async fn cover_day(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<CoverQuery>,
) -> ApiResult<Json<Value>> {
    if user.role == "student" {
        return Err(ApiError::Forbidden);
    }
    let date = match q.date {
        Some(d) => parse_date(&d)?,
        None => today(),
    };
    let dstr = date.to_string();
    let day = day_of(&date);
    let cycle: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");
    let week = week_index(&date, cycle);

    let slots = day_slots(&st, &user.school_id, day).await?;
    let teaching_idxs: Vec<i64> = slots.iter().filter(|s| !s.3).map(|s| s.0).collect();
    let is_school_day = !teaching_idxs.is_empty();
    let am_max = teaching_idxs.get(teaching_idxs.len().saturating_sub(1) / 2).copied().unwrap_or(0);

    let events = sqlx::query(
        "SELECT id, title, start_date, end_date, kind, notes FROM calendar_events WHERE school_id = ? AND start_date <= ? AND end_date >= ? ORDER BY start_date",
    )
    .bind(&user.school_id)
    .bind(&dstr)
    .bind(&dstr)
    .fetch_all(&st.pool)
    .await?;
    let is_holiday = events.iter().any(|e| e.get::<String, _>("kind") == "holiday");

    let leave_rows = sqlx::query(
        "SELECT l.id, l.teacher_id, l.part, t.name AS teacher_name, t.initials AS teacher_initials
         FROM leave_requests l JOIN teachers t ON t.id = l.teacher_id
         WHERE l.school_id = ? AND l.status = 'approved' AND l.start_date <= ? AND l.end_date >= ?",
    )
    .bind(&user.school_id)
    .bind(&dstr)
    .bind(&dstr)
    .fetch_all(&st.pool)
    .await?;

    let mut part_map: HashMap<String, String> = HashMap::new();
    let mut info: HashMap<String, (String, String, String)> = HashMap::new();
    for r in &leave_rows {
        let tid: String = r.get("teacher_id");
        merge_absence(&mut part_map, &tid, &r.get::<String, _>("part"));
        info.insert(
            tid.clone(),
            (
                r.get::<String, _>("teacher_name"),
                r.get::<String, _>("teacher_initials"),
                r.get::<String, _>("id"),
            ),
        );
    }
    let absences: Vec<Absence> = part_map
        .iter()
        .map(|(tid, part)| {
            let (name, initials, leave_id) = info.get(tid).cloned().unwrap_or_default();
            Absence { teacher_id: tid.clone(), name, initials, part: part.clone(), leave_id }
        })
        .collect();

    let events_json: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "id": e.get::<String, _>("id"),
                "title": e.get::<String, _>("title"),
                "kind": e.get::<String, _>("kind"),
                "start_date": e.get::<String, _>("start_date"),
                "end_date": e.get::<String, _>("end_date"),
            })
        })
        .collect();

    let absences_json: Vec<Value> = absences
        .iter()
        .map(|a| {
            json!({
                "teacher_id": a.teacher_id,
                "teacher_name": a.name,
                "teacher_initials": a.initials,
                "part": a.part,
                "leave_id": a.leave_id,
            })
        })
        .collect();

    let mut periods: Vec<Value> = Vec::new();
    if is_school_day && !is_holiday && !absences.is_empty() {
        let entries = sqlx::query(
            "SELECT e.idx, e.lesson_id, r.name AS room_name, l.teacher_id, l.subject_id,
                    s.name AS subject_name, s.code AS subject_code, s.color AS subject_color, c.name AS class_name
             FROM timetable_entries e
             JOIN lessons l ON l.id = e.lesson_id
             JOIN subjects s ON s.id = l.subject_id
             JOIN classes c ON c.id = l.class_id
             LEFT JOIN rooms r ON r.id = e.room_id
             WHERE e.school_id = ? AND e.week = ? AND e.day = ?
             ORDER BY e.idx",
        )
        .bind(&user.school_id)
        .bind(week)
        .bind(day)
        .fetch_all(&st.pool)
        .await?;

        let sub_rows = sqlx::query(
            "SELECT s.id, s.lesson_id, s.substitute_teacher_id, t.name AS substitute_name
             FROM substitutions s JOIN teachers t ON t.id = s.substitute_teacher_id
             WHERE s.school_id = ? AND s.date = ? AND s.status = 'assigned'",
        )
        .bind(&user.school_id)
        .bind(&dstr)
        .fetch_all(&st.pool)
        .await?;
        let mut sub_map: HashMap<String, (String, String)> = HashMap::new();
        for r in &sub_rows {
            sub_map.insert(
                r.get::<String, _>("lesson_id"),
                (r.get::<String, _>("id"), r.get::<String, _>("substitute_name")),
            );
        }

        // candidate inputs
        let teacher_rows = sqlx::query("SELECT id, name, initials FROM teachers WHERE school_id = ? ORDER BY name")
            .bind(&user.school_id)
            .fetch_all(&st.pool)
            .await?;
        let subj_rows = sqlx::query("SELECT teacher_id, subject_id FROM teacher_subjects WHERE school_id = ?")
            .bind(&user.school_id)
            .fetch_all(&st.pool)
            .await?;
        let mut teacher_subjects: HashMap<String, HashSet<String>> = HashMap::new();
        for r in &subj_rows {
            teacher_subjects
                .entry(r.get::<String, _>("teacher_id"))
                .or_default()
                .insert(r.get::<String, _>("subject_id"));
        }
        let unavail_rows =
            sqlx::query("SELECT teacher_id, idx FROM teacher_unavailability WHERE school_id = ? AND day = ?")
                .bind(&user.school_id)
                .bind(day)
                .fetch_all(&st.pool)
                .await?;
        let unavailable: HashSet<(String, i64)> = unavail_rows
            .iter()
            .map(|r| (r.get::<String, _>("teacher_id"), r.get::<i64, _>("idx")))
            .collect();
        let busy_rows = sqlx::query(
            "SELECT l.teacher_id, e.idx FROM timetable_entries e JOIN lessons l ON l.id = e.lesson_id
             WHERE e.school_id = ? AND e.week = ? AND e.day = ?",
        )
        .bind(&user.school_id)
        .bind(week)
        .bind(day)
        .fetch_all(&st.pool)
        .await?;
        let mut busy: HashSet<(String, i64)> = HashSet::new();
        let mut daily_load: HashMap<String, i64> = HashMap::new();
        for r in &busy_rows {
            let tid: String = r.get("teacher_id");
            let idx: i64 = r.get("idx");
            busy.insert((tid.clone(), idx));
            *daily_load.entry(tid).or_default() += 1;
        }

        let monday = date - chrono::Duration::days(day - 1);
        let sunday = monday + chrono::Duration::days(6);
        let cover_rows = sqlx::query(
            "SELECT substitute_teacher_id, COUNT(*) AS n FROM substitutions
             WHERE school_id = ? AND status = 'assigned' AND date >= ? AND date <= ?
             GROUP BY substitute_teacher_id",
        )
        .bind(&user.school_id)
        .bind(monday.to_string())
        .bind(sunday.to_string())
        .fetch_all(&st.pool)
        .await?;
        let mut cover_counts: HashMap<String, i64> = HashMap::new();
        for r in &cover_rows {
            cover_counts.insert(r.get::<String, _>("substitute_teacher_id"), r.get::<i64, _>("n"));
        }

        for e in &entries {
            let teacher_id: String = e.get("teacher_id");
            let idx: i64 = e.get("idx");
            let Some(absence) = absences.iter().find(|a| a.teacher_id == teacher_id) else {
                continue;
            };
            if !part_covers(&absence.part, idx, am_max) {
                continue;
            }
            let lesson_id: String = e.get("lesson_id");
            let subject_id: String = e.get("subject_id");

            let mut eligible: Vec<Value> = Vec::new();
            let mut blocked: Vec<Value> = Vec::new();
            for t in &teacher_rows {
                let tid: String = t.get("id");
                if tid == teacher_id {
                    continue;
                }
                let name: String = t.get("name");
                if let Some(part) = part_map.get(&tid) {
                    if part_covers(part, idx, am_max) {
                        blocked.push(json!({ "teacher_id": tid, "name": name, "reason": "on leave at this time" }));
                        continue;
                    }
                }
                if unavailable.contains(&(tid.clone(), idx)) {
                    blocked.push(json!({ "teacher_id": tid, "name": name, "reason": "marked unavailable" }));
                    continue;
                }
                if busy.contains(&(tid.clone(), idx)) {
                    blocked.push(json!({ "teacher_id": tid, "name": name, "reason": "teaching another class at this time" }));
                    continue;
                }
                let teaches_subject = teacher_subjects
                    .get(&tid)
                    .map(|s| s.contains(&subject_id))
                    .unwrap_or(false);
                let cover_count = cover_counts.get(&tid).copied().unwrap_or(0);
                let load = daily_load.get(&tid).copied().unwrap_or(0);
                let score = (if teaches_subject { 100.0 } else { 0.0 }) + 40.0
                    - cover_count as f64 * 10.0
                    - load as f64 * 2.0;
                eligible.push(json!({
                    "teacher_id": tid,
                    "name": name,
                    "initials": t.get::<String, _>("initials"),
                    "score": score,
                    "teaches_subject": teaches_subject,
                    "cover_count": cover_count,
                    "daily_load": load,
                    "reason": if teaches_subject { "teaches this subject" } else { "available, general cover" },
                }));
            }
            eligible.sort_by(|a, b| {
                let sa = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let sb = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
            eligible.truncate(10);

            let substitution = sub_map.get(&lesson_id).map(|(sid, name)| {
                json!({ "id": sid, "substitute_name": name })
            });

            periods.push(json!({
                "idx": idx,
                "day": day,
                "day_name": day_name(day),
                "lesson_id": lesson_id,
                "subject_id": subject_id,
                "subject_name": e.get::<String, _>("subject_name"),
                "subject_code": e.get::<String, _>("subject_code"),
                "subject_color": e.get::<String, _>("subject_color"),
                "class_name": e.get::<String, _>("class_name"),
                "room_name": e.get::<Option<String>, _>("room_name"),
                "absent_teacher_id": teacher_id,
                "absent_teacher_name": absence.name,
                "absent_part": absence.part,
                "substitution": substitution,
                "candidates": eligible,
                "blocked": blocked,
            }));
        }
    }

    Ok(Json(json!({
        "date": dstr,
        "day": day,
        "day_name": day_name(day),
        "week": week,
        "is_school_day": is_school_day,
        "is_holiday": is_holiday,
        "events": events_json,
        "absences": absences_json,
        "periods": periods,
    })))
}

pub async fn assign_cover(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<CoverAssignInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    let date = parse_date(&input.date)?;
    if day_of(&date) != input.day {
        return Err(ApiError::BadRequest("date and weekday do not match".into()));
    }
    let dstr = date.to_string();
    let cycle: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");
    let week = week_index(&date, cycle);

    let entry = sqlx::query(
        "SELECT l.teacher_id FROM timetable_entries e JOIN lessons l ON l.id = e.lesson_id
         WHERE e.school_id = ? AND e.lesson_id = ? AND e.week = ? AND e.day = ? AND e.idx = ?
         LIMIT 1",
    )
    .bind(&user.school_id)
    .bind(&input.lesson_id)
    .bind(week)
    .bind(input.day)
    .bind(input.idx)
    .fetch_optional(&st.pool)
    .await?;
    let Some(entry) = entry else {
        return Err(ApiError::BadRequest("that lesson is not on the timetable at this time".into()));
    };
    let original_teacher: String = entry.get("teacher_id");
    if original_teacher == input.substitute_teacher_id {
        return Err(ApiError::BadRequest("the absent teacher cannot cover their own lesson".into()));
    }

    let substitute = sqlx::query("SELECT id FROM teachers WHERE id = ? AND school_id = ?")
        .bind(&input.substitute_teacher_id)
        .bind(&user.school_id)
        .fetch_optional(&st.pool)
        .await?;
    if substitute.is_none() {
        return Err(ApiError::BadRequest("substitute teacher not found".into()));
    }

    let busy: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM timetable_entries e JOIN lessons l ON l.id = e.lesson_id
         WHERE e.school_id = ? AND l.teacher_id = ? AND e.week = ? AND e.day = ? AND e.idx = ?",
    )
    .bind(&user.school_id)
    .bind(&input.substitute_teacher_id)
    .bind(week)
    .bind(input.day)
    .bind(input.idx)
    .fetch_one(&st.pool)
    .await?
    .get("n");
    if busy > 0 {
        return Err(ApiError::Conflict("substitute is teaching another class at this time".into()));
    }

    let unavailable: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM teacher_unavailability WHERE teacher_id = ? AND day = ? AND idx = ?",
    )
    .bind(&input.substitute_teacher_id)
    .bind(input.day)
    .bind(input.idx)
    .fetch_one(&st.pool)
    .await?
    .get("n");
    if unavailable > 0 {
        return Err(ApiError::Conflict("substitute is marked unavailable at this time".into()));
    }

    let leave_rows = sqlx::query(
        "SELECT part FROM leave_requests WHERE teacher_id = ? AND school_id = ? AND status = 'approved' AND start_date <= ? AND end_date >= ?",
    )
    .bind(&input.substitute_teacher_id)
    .bind(&user.school_id)
    .bind(&dstr)
    .bind(&dstr)
    .fetch_all(&st.pool)
    .await?;
    let slots = day_slots(&st, &user.school_id, input.day).await?;
    let idxs: Vec<i64> = slots.iter().filter(|s| !s.3).map(|s| s.0).collect();
    let am_max = idxs.get(idxs.len().saturating_sub(1) / 2).copied().unwrap_or(0);
    for r in &leave_rows {
        let part: String = r.get("part");
        if part_covers(&part, input.idx, am_max) {
            return Err(ApiError::Conflict("substitute is on leave at that time".into()));
        }
    }

    let existing = sqlx::query("SELECT id FROM substitutions WHERE school_id = ? AND date = ? AND lesson_id = ?")
        .bind(&user.school_id)
        .bind(&dstr)
        .bind(&input.lesson_id)
        .fetch_optional(&st.pool)
        .await?;
    let ts = now();
    if let Some(e) = existing {
        let sid: String = e.get("id");
        sqlx::query(
            "UPDATE substitutions SET substitute_teacher_id = ?, original_teacher_id = ?, status = 'assigned', created_at = ? WHERE id = ?",
        )
        .bind(&input.substitute_teacher_id)
        .bind(&original_teacher)
        .bind(&ts)
        .bind(&sid)
        .execute(&st.pool)
        .await?;
        return Ok((StatusCode::OK, Json(json!({ "id": sid, "updated": true }))));
    }
    let sid = id();
    sqlx::query(
        "INSERT INTO substitutions (id, school_id, date, day, idx, lesson_id, original_teacher_id, substitute_teacher_id, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'assigned', ?)",
    )
    .bind(&sid)
    .bind(&user.school_id)
    .bind(&dstr)
    .bind(input.day)
    .bind(input.idx)
    .bind(&input.lesson_id)
    .bind(&original_teacher)
    .bind(&input.substitute_teacher_id)
    .bind(&ts)
    .execute(&st.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": sid }))))
}

pub async fn cancel_cover(
    State(st): State<AppState>,
    user: AuthUser,
    Path(sid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    sqlx::query("UPDATE substitutions SET status = 'cancelled' WHERE id = ? AND school_id = ?")
        .bind(&sid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

pub async fn cover_history(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Value>> {
    if user.role == "student" {
        return Err(ApiError::Forbidden);
    }
    let from = q.from.unwrap_or_else(|| "2000-01-01".to_string());
    let to = q.to.unwrap_or_else(|| "2100-01-01".to_string());
    let rows = sqlx::query(
        "SELECT s.id, s.date, s.day, s.idx, s.status,
                ot.name AS original_name, st2.name AS substitute_name,
                sub.name AS subject_name, sub.code AS subject_code, c.name AS class_name
         FROM substitutions s
         LEFT JOIN teachers ot ON ot.id = s.original_teacher_id
         LEFT JOIN teachers st2 ON st2.id = s.substitute_teacher_id
         LEFT JOIN lessons l ON l.id = s.lesson_id
         LEFT JOIN subjects sub ON sub.id = l.subject_id
         LEFT JOIN classes c ON c.id = l.class_id
         WHERE s.school_id = ? AND s.date >= ? AND s.date <= ?
         ORDER BY s.date DESC, s.idx DESC LIMIT 500",
    )
    .bind(&user.school_id)
    .bind(&from)
    .bind(&to)
    .fetch_all(&st.pool)
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "date": r.get::<String, _>("date"),
                "day": r.get::<i64, _>("day"),
                "idx": r.get::<i64, _>("idx"),
                "status": r.get::<String, _>("status"),
                "original_name": r.get::<Option<String>, _>("original_name").unwrap_or_else(|| "(removed)".into()),
                "substitute_name": r.get::<Option<String>, _>("substitute_name").unwrap_or_else(|| "(removed)".into()),
                "subject_name": r.get::<Option<String>, _>("subject_name"),
                "subject_code": r.get::<Option<String>, _>("subject_code"),
                "class_name": r.get::<Option<String>, _>("class_name"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}
