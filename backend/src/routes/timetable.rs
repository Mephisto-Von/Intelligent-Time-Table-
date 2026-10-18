use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Duration;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::audit;
use crate::auth::AuthUser;
use crate::dates::{day_of, today};
use crate::error::{ApiError, ApiResult};
use crate::models::{day_name, EntryInput, EntryPatch, GenerateRequest};
use crate::solver::{
    self, ClassIn, FixedPlacement, Input as SolverInput, LessonIn, RoomIn, Slot, TeacherIn, Weights,
};
use crate::state::{id, now, AppState};

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

pub async fn timetable(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Value>> {
    let school = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    let slots = sqlx::query(
        "SELECT id, day, idx, start_time, end_time, label, is_break FROM time_slots WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(&user.school_id)
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

    let entries = if user.role == "student" {
        match &user.class_id {
            Some(cid) => sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE e.school_id = ? AND l.class_id = ? ORDER BY e.week, e.day, e.idx")))
                .bind(&user.school_id)
                .bind(cid)
                .fetch_all(&st.pool)
                .await?,
            None => vec![],
        }
    } else {
        sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE e.school_id = ? ORDER BY e.week, e.day, e.idx")))
            .bind(&user.school_id)
            .fetch_all(&st.pool)
            .await?
    };

    Ok(Json(json!({
        "slots": slots_json,
        "entries": entries.iter().map(entry_json).collect::<Vec<_>>(),
        "cycle_weeks": school.get::<i64, _>("cycle_weeks"),
    })))
}

// ---------------------------------------------------------------------------
// generation
// ---------------------------------------------------------------------------

pub async fn generate(
    State(st): State<AppState>,
    user: AuthUser,
    Json(req): Json<GenerateRequest>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let school_id = user.school_id.clone();

    let cycle: i64 = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&school_id)
        .fetch_one(&st.pool)
        .await?
        .get("cycle_weeks");
    let weeks = cycle.clamp(1, 2) as usize;

    let slot_rows = sqlx::query(
        "SELECT day, idx FROM time_slots WHERE school_id = ? AND is_break = 0 ORDER BY day, idx",
    )
    .bind(&school_id)
    .fetch_all(&st.pool)
    .await?;
    if slot_rows.is_empty() {
        return Err(ApiError::BadRequest(
            "define a bell schedule (time slots) before generating".into(),
        ));
    }
    let mut slot_pairs: Vec<(i64, i64)> = Vec::new();
    let mut slot_index: HashMap<(i64, i64), usize> = HashMap::new();
    for r in &slot_rows {
        let day: i64 = r.get("day");
        let idx: i64 = r.get("idx");
        slot_index.insert((day, idx), slot_pairs.len());
        slot_pairs.push((day, idx));
    }
    let slots: Vec<Slot> = slot_pairs.iter().map(|&(day, idx)| Slot { day, idx }).collect();

    let class_rows = sqlx::query("SELECT id, name, size FROM classes WHERE school_id = ? ORDER BY name")
        .bind(&school_id)
        .fetch_all(&st.pool)
        .await?;
    let mut class_index: HashMap<String, usize> = HashMap::new();
    let mut classes: Vec<ClassIn> = Vec::new();
    let mut class_names: Vec<String> = Vec::new();
    for r in &class_rows {
        let cid: String = r.get("id");
        class_index.insert(cid.clone(), classes.len());
        class_names.push(r.get::<String, _>("name"));
        classes.push(ClassIn { id: cid, size: r.get("size") });
    }

    let teacher_rows = sqlx::query("SELECT id, name, max_weekly, max_daily FROM teachers WHERE school_id = ? ORDER BY name")
        .bind(&school_id)
        .fetch_all(&st.pool)
        .await?;
    let unavail_rows = sqlx::query("SELECT teacher_id, day, idx FROM teacher_unavailability WHERE school_id = ?")
        .bind(&school_id)
        .fetch_all(&st.pool)
        .await?;
    let mut unavail_map: HashMap<String, Vec<(i64, i64)>> = HashMap::new();
    for r in &unavail_rows {
        unavail_map
            .entry(r.get::<String, _>("teacher_id"))
            .or_default()
            .push((r.get("day"), r.get("idx")));
    }
    let mut teacher_index: HashMap<String, usize> = HashMap::new();
    let mut teachers: Vec<TeacherIn> = Vec::new();
    let mut teacher_names: Vec<String> = Vec::new();
    for r in &teacher_rows {
        let tid: String = r.get("id");
        teacher_index.insert(tid.clone(), teachers.len());
        teacher_names.push(r.get::<String, _>("name"));
        teachers.push(TeacherIn {
            id: tid.clone(),
            max_weekly: Some(r.get::<i64, _>("max_weekly").max(0) as u32),
            max_daily: Some(r.get::<i64, _>("max_daily").max(0) as u32),
            unavailable: unavail_map.get(&tid).cloned().unwrap_or_default(),
        });
    }

    let room_rows = sqlx::query("SELECT id, name, capacity, room_type FROM rooms WHERE school_id = ? ORDER BY name")
        .bind(&school_id)
        .fetch_all(&st.pool)
        .await?;
    let mut room_index: HashMap<String, usize> = HashMap::new();
    let mut rooms: Vec<RoomIn> = Vec::new();
    let mut room_ids: Vec<String> = Vec::new();
    for r in &room_rows {
        let rid: String = r.get("id");
        room_index.insert(rid.clone(), rooms.len());
        room_ids.push(rid.clone());
        rooms.push(RoomIn {
            id: rid,
            capacity: r.get("capacity"),
            room_type: r.get::<String, _>("room_type"),
        });
    }

    let subject_rows = sqlx::query("SELECT id FROM subjects WHERE school_id = ? ORDER BY name")
        .bind(&school_id)
        .fetch_all(&st.pool)
        .await?;
    let mut subject_index: HashMap<String, usize> = HashMap::new();
    for (i, r) in subject_rows.iter().enumerate() {
        subject_index.insert(r.get::<String, _>("id"), i);
    }

    let lesson_rows = sqlx::query(
        "SELECT l.id, l.class_id, l.subject_id, l.teacher_id, l.room_id, l.required_room_type,
                l.periods_per_week, l.max_per_day, s.name AS subject_name, c.name AS class_name, t.name AS teacher_name
         FROM lessons l
         JOIN subjects s ON s.id = l.subject_id
         JOIN classes c ON c.id = l.class_id
         JOIN teachers t ON t.id = l.teacher_id
         WHERE l.school_id = ? ORDER BY c.name, s.name",
    )
    .bind(&school_id)
    .fetch_all(&st.pool)
    .await?;

    let locked_rows = sqlx::query(
        "SELECT lesson_id, week, day, idx, room_id FROM timetable_entries WHERE school_id = ? AND locked = 1",
    )
    .bind(&school_id)
    .fetch_all(&st.pool)
    .await?;
    let mut fixed_map: HashMap<String, Vec<FixedPlacement>> = HashMap::new();
    for r in &locked_rows {
        let lesson_id: String = r.get("lesson_id");
        let day: i64 = r.get("day");
        let idx: i64 = r.get("idx");
        let week: i64 = r.get("week");
        let room_id: Option<String> = r.get("room_id");
        if let Some(&slot) = slot_index.get(&(day, idx)) {
            let room = room_id.as_ref().and_then(|r| room_index.get(r).copied());
            fixed_map.entry(lesson_id).or_default().push(FixedPlacement {
                week: (week.max(0) as usize).min(weeks - 1),
                slot,
                room,
            });
        }
    }

    let mut lessons: Vec<LessonIn> = Vec::new();
    let mut lesson_meta: Vec<(String, String, String, String)> = Vec::new(); // id, class, subject, teacher
    let mut requested: u32 = 0;
    for r in &lesson_rows {
        let lid: String = r.get("id");
        let class_id: String = r.get("class_id");
        let subject_id: String = r.get("subject_id");
        let teacher_id: String = r.get("teacher_id");
        let room_id: Option<String> = r.get("room_id");
        let (Some(&class), Some(&teacher)) =
            (class_index.get(&class_id), teacher_index.get(&teacher_id))
        else {
            continue;
        };
        let periods = r.get::<i64, _>("periods_per_week").max(0) as u32;
        requested += periods;
        lesson_meta.push((
            lid.clone(),
            r.get::<String, _>("class_name"),
            r.get::<String, _>("subject_name"),
            r.get::<String, _>("teacher_name"),
        ));
        lessons.push(LessonIn {
            id: lid.clone(),
            class,
            teacher,
            subject: *subject_index.get(&subject_id).unwrap_or(&0),
            periods,
            max_per_day: r.get::<i64, _>("max_per_day").max(1) as u32,
            room: room_id.as_ref().and_then(|r| room_index.get(r).copied()),
            required_room_type: r.get::<Option<String>, _>("required_room_type"),
            fixed: fixed_map.remove(&lid).unwrap_or_default(),
        });
    }

    if lessons.is_empty() {
        return Err(ApiError::BadRequest("add lesson requirements before generating".into()));
    }

    let seed = req.seed.unwrap_or_else(rand::random::<u64>);
    let input = SolverInput {
        slots,
        weeks,
        lessons,
        teachers,
        classes,
        rooms,
        weights: req.weights.unwrap_or_else(Weights::default),
        seed,
        time_limit_ms: req.time_limit_ms.unwrap_or(2000).clamp(100, 20000),
        iterations: None,
    };

    let output = tokio::task::spawn_blocking(move || solver::run(&input))
        .await
        .map_err(|e| ApiError::Internal(format!("solver task failed: {e}")))?;

    let mut tx = st.pool.begin().await?;
    sqlx::query("DELETE FROM timetable_entries WHERE school_id = ? AND locked = 0")
        .bind(&school_id)
        .execute(&mut *tx)
        .await?;
    let ts = now();
    for p in &output.placed {
        let (day, idx) = slot_pairs[p.slot];
        let lesson_id = &lesson_meta[p.lesson].0;
        let room_id = p.room.map(|r| room_ids[r].clone());
        sqlx::query(
            "INSERT INTO timetable_entries (id, school_id, lesson_id, week, day, idx, room_id, locked, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(id())
        .bind(&school_id)
        .bind(lesson_id)
        .bind(p.week as i64)
        .bind(day)
        .bind(idx)
        .bind(room_id)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let unplaced: Vec<Value> = output
        .unplaced
        .iter()
        .map(|u| {
            let (lid, class_name, subject_name, teacher_name) = &lesson_meta[u.lesson];
            json!({
                "lesson_id": lid,
                "class_name": class_name,
                "subject_name": subject_name,
                "teacher_name": teacher_name,
                "count": u.count,
                "dominant": u.dominant,
                "reasons": u.reasons,
            })
        })
        .collect();

    audit::record(
        &st.pool,
        &user,
        "timetable.generate",
        "timetable",
        None,
        format!(
            "Generated timetable: {} placed, {} unplaced (seed {seed})",
            output.placed.len(),
            output.total_unplaced()
        ),
    )
    .await;

    Ok(Json(json!({
        "placed": output.placed.len(),
        "requested": requested,
        "unplaced_total": output.total_unplaced(),
        "unplaced": unplaced,
        "score": output.cost,
        "soft": output.soft,
        "iterations": output.iterations,
        "elapsed_ms": output.elapsed_ms,
        "seed": seed,
        "locked_kept": locked_rows.len(),
    })))
}

// ---------------------------------------------------------------------------
// manual entries
// ---------------------------------------------------------------------------

async fn check_entry_conflicts(
    pool: &SqlitePool,
    school_id: &str,
    lesson_id: &str,
    week: i64,
    day: i64,
    idx: i64,
    room_id: Option<&str>,
    exclude_entry: Option<&str>,
) -> ApiResult<Vec<Value>> {
    let mut conflicts: Vec<Value> = Vec::new();

    let lesson = sqlx::query(
        "SELECT l.class_id, l.teacher_id, l.room_id, l.required_room_type, l.max_per_day, c.size AS class_size, c.name AS class_name
         FROM lessons l JOIN classes c ON c.id = l.class_id WHERE l.id = ? AND l.school_id = ?",
    )
    .bind(lesson_id)
    .bind(school_id)
    .fetch_optional(pool)
    .await?;
    let Some(lesson) = lesson else {
        return Ok(vec![json!({ "kind": "lesson", "message": "lesson not found" })]);
    };
    let class_id: String = lesson.get("class_id");
    let teacher_id: String = lesson.get("teacher_id");
    let required_type: Option<String> = lesson.get("required_room_type");
    let max_per_day: i64 = lesson.get("max_per_day");
    let class_size: i64 = lesson.get("class_size");
    let class_name: String = lesson.get("class_name");

    let slot = sqlx::query("SELECT is_break FROM time_slots WHERE school_id = ? AND day = ? AND idx = ?")
        .bind(school_id)
        .bind(day)
        .bind(idx)
        .fetch_optional(pool)
        .await?;
    match slot {
        None => conflicts.push(json!({ "kind": "slot", "message": format!("no period {idx} on {} in the bell schedule", day_name(day)) })),
        Some(r) if r.get::<i64, _>("is_break") != 0 => {
            conflicts.push(json!({ "kind": "slot", "message": "that period is a break" }))
        }
        _ => {}
    }

    let unavailable: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM teacher_unavailability WHERE teacher_id = ? AND day = ? AND idx = ?",
    )
    .bind(&teacher_id)
    .bind(day)
    .bind(idx)
    .fetch_one(pool)
    .await?
    .get("n");
    if unavailable > 0 {
        conflicts.push(json!({ "kind": "teacher_unavailable", "message": "teacher is marked unavailable at that time" }));
    }

    let busy_sql = |col: &str| {
        format!(
            "SELECT s.name AS subject_name, c.name AS class_name FROM timetable_entries e
             JOIN lessons l2 ON l2.id = e.lesson_id
             JOIN subjects s ON s.id = l2.subject_id
             JOIN classes c ON c.id = l2.class_id
             WHERE e.school_id = ? AND e.week = ? AND e.day = ? AND e.idx = ? AND {col} = ?"
        )
    };
    let exclude_clause = "AND e.id != ?";

    let mut q = busy_sql("l2.class_id");
    if let Some(_) = exclude_entry {
        q.push_str(exclude_clause);
    }
    let mut query = sqlx::query(sqlx::AssertSqlSafe(q))
        .bind(school_id)
        .bind(week)
        .bind(day)
        .bind(idx)
        .bind(&class_id);
    if let Some(ex) = exclude_entry {
        query = query.bind(ex);
    }
    for r in query.fetch_all(pool).await? {
        conflicts.push(json!({
            "kind": "class_busy",
            "message": format!("{} already has {} at that time", r.get::<String, _>("class_name"), r.get::<String, _>("subject_name")),
        }));
    }

    let mut q = busy_sql("l2.teacher_id");
    if exclude_entry.is_some() {
        q.push_str(exclude_clause);
    }
    let mut query = sqlx::query(sqlx::AssertSqlSafe(q))
        .bind(school_id)
        .bind(week)
        .bind(day)
        .bind(idx)
        .bind(&teacher_id);
    if let Some(ex) = exclude_entry {
        query = query.bind(ex);
    }
    for r in query.fetch_all(pool).await? {
        conflicts.push(json!({
            "kind": "teacher_busy",
            "message": format!("teacher is already teaching {} to {} at that time", r.get::<String, _>("subject_name"), r.get::<String, _>("class_name")),
        }));
    }

    if let Some(room) = room_id {
        let room_row = sqlx::query("SELECT name, capacity, room_type FROM rooms WHERE id = ? AND school_id = ?")
            .bind(room)
            .bind(school_id)
            .fetch_optional(pool)
            .await?;
        match room_row {
            None => conflicts.push(json!({ "kind": "room", "message": "room not found" })),
            Some(r) => {
                let capacity: i64 = r.get("capacity");
                let room_type: String = r.get("room_type");
                let room_name: String = r.get("name");
                if capacity < class_size {
                    conflicts.push(json!({
                        "kind": "room_capacity",
                        "message": format!("{room_name} seats {capacity} but {class_name} has {class_size} students"),
                    }));
                }
                if let Some(t) = &required_type {
                    if &room_type != t {
                        conflicts.push(json!({
                            "kind": "room_type",
                            "message": format!("lesson needs a '{t}' room; {room_name} is '{room_type}'"),
                        }));
                    }
                }
                let mut q = String::from(
                    "SELECT COUNT(*) AS n FROM timetable_entries e WHERE e.school_id = ? AND e.week = ? AND e.day = ? AND e.idx = ? AND e.room_id = ?",
                );
                if exclude_entry.is_some() {
                    q.push_str(" AND e.id != ?");
                }
                let mut query = sqlx::query(sqlx::AssertSqlSafe(q))
                    .bind(school_id)
                    .bind(week)
                    .bind(day)
                    .bind(idx)
                    .bind(room);
                if let Some(ex) = exclude_entry {
                    query = query.bind(ex);
                }
                let n: i64 = query.fetch_one(pool).await?.get("n");
                if n > 0 {
                    conflicts.push(json!({ "kind": "room_busy", "message": format!("{room_name} is occupied at that time") }));
                }
            }
        }
    }

    let mut q = String::from(
        "SELECT COUNT(*) AS n FROM timetable_entries WHERE school_id = ? AND lesson_id = ? AND week = ? AND day = ?",
    );
    if exclude_entry.is_some() {
        q.push_str(" AND id != ?");
    }
    let mut query = sqlx::query(sqlx::AssertSqlSafe(q))
        .bind(school_id)
        .bind(lesson_id)
        .bind(week)
        .bind(day);
    if let Some(ex) = exclude_entry {
        query = query.bind(ex);
    }
    let per_day: i64 = query.fetch_one(pool).await?.get("n");
    if per_day >= max_per_day {
        conflicts.push(json!({
            "kind": "max_per_day",
            "message": format!("{class_name} already has {per_day} of this subject that day (max {max_per_day})"),
        }));
    }

    Ok(conflicts)
}

/// Chooses a room for an entry at a given slot, mirroring the solver's rules:
/// a fixed room is mandatory, a required room type must match and fit the class,
/// otherwise the smallest fitting free room is used (or none at all).
async fn pick_room_for_entry(
    pool: &SqlitePool,
    school_id: &str,
    lesson_id: &str,
    week: i64,
    day: i64,
    idx: i64,
    exclude_entry: Option<&str>,
) -> ApiResult<Option<String>> {
    let lesson = sqlx::query(
        "SELECT l.room_id, l.required_room_type, c.size AS class_size
         FROM lessons l JOIN classes c ON c.id = l.class_id
         WHERE l.id = ? AND l.school_id = ?",
    )
    .bind(lesson_id)
    .bind(school_id)
    .fetch_optional(pool)
    .await?;
    let Some(lesson) = lesson else {
        return Ok(None);
    };
    let fixed: Option<String> = lesson.get("room_id");
    if let Some(f) = fixed {
        return Ok(Some(f));
    }
    let required: Option<String> = lesson.get("required_room_type");
    let class_size: i64 = lesson.get("class_size");

    let rooms = sqlx::query("SELECT id, capacity, room_type FROM rooms WHERE school_id = ? ORDER BY capacity")
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    for r in &rooms {
        let rid: String = r.get("id");
        let capacity: i64 = r.get("capacity");
        let room_type: String = r.get("room_type");
        if capacity < class_size {
            continue;
        }
        if let Some(t) = &required {
            if &room_type != t {
                continue;
            }
        }
        let mut q = String::from(
            "SELECT COUNT(*) AS n FROM timetable_entries WHERE school_id = ? AND week = ? AND day = ? AND idx = ? AND room_id = ?",
        );
        if exclude_entry.is_some() {
            q.push_str(" AND id != ?");
        }
        let mut query = sqlx::query(sqlx::AssertSqlSafe(q))
            .bind(school_id)
            .bind(week)
            .bind(day)
            .bind(idx)
            .bind(&rid);
        if let Some(ex) = exclude_entry {
            query = query.bind(ex);
        }
        let n: i64 = query.fetch_one(pool).await?.get("n");
        if n == 0 {
            return Ok(Some(rid));
        }
    }

    if required.is_some() {
        Err(ApiError::Conflict(
            "no free room of the required type at that time".into(),
        ))
    } else {
        Ok(None)
    }
}

fn conflicts_error(conflicts: &[Value]) -> ApiError {
    let messages: Vec<String> = conflicts
        .iter()
        .filter_map(|c| c.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
        .collect();
    ApiError::Conflict(messages.join(" · "))
}

pub async fn create_entry(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<EntryInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    let week = input.week.unwrap_or(0).clamp(0, 1);
    let conflicts = check_entry_conflicts(
        &st.pool,
        &user.school_id,
        &input.lesson_id,
        week,
        input.day,
        input.idx,
        input.room_id.as_deref(),
        None,
    )
    .await?;
    if !conflicts.is_empty() {
        return Err(conflicts_error(&conflicts));
    }
    let eid = id();
    sqlx::query(
        "INSERT INTO timetable_entries (id, school_id, lesson_id, week, day, idx, room_id, locked, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&eid)
    .bind(&user.school_id)
    .bind(&input.lesson_id)
    .bind(week)
    .bind(input.day)
    .bind(input.idx)
    .bind(&input.room_id)
    .bind(if input.locked.unwrap_or(true) { 1 } else { 0 })
    .bind(now())
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "timetable.place",
        "entry",
        Some(&eid),
        format!("Placed a lesson at week {} day {} P{}", week + 1, input.day, input.idx),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": eid }))))
}

pub async fn update_entry(
    State(st): State<AppState>,
    user: AuthUser,
    Path(eid): Path<String>,
    Json(patch): Json<EntryPatch>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let row = sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE e.id = ? AND e.school_id = ?")))
        .bind(&eid)
        .bind(&user.school_id)
        .fetch_optional(&st.pool)
        .await?;
    let Some(row) = row else {
        return Err(ApiError::NotFound);
    };
    let lesson_id: String = row.get("lesson_id");
    let week = patch.week.unwrap_or(row.get::<i64, _>("week")).clamp(0, 1);
    let day = patch.day.unwrap_or(row.get::<i64, _>("day"));
    let idx = patch.idx.unwrap_or(row.get::<i64, _>("idx"));
    let slot_changed = week != row.get::<i64, _>("week")
        || day != row.get::<i64, _>("day")
        || idx != row.get::<i64, _>("idx");
    let room_id: Option<String> = match patch.room_id {
        Some(inner) => inner,
        None if slot_changed => {
            // Re-pick a room for the new slot instead of blindly keeping the old
            // one, so a hand-move rarely fails just because its room is taken.
            pick_room_for_entry(&st.pool, &user.school_id, &lesson_id, week, day, idx, Some(&eid))
                .await?
        }
        None => row.get::<Option<String>, _>("room_id"),
    };
    let locked = patch.locked.unwrap_or(row.get::<i64, _>("locked") != 0);

    let conflicts = check_entry_conflicts(
        &st.pool,
        &user.school_id,
        &lesson_id,
        week,
        day,
        idx,
        room_id.as_deref(),
        Some(&eid),
    )
    .await?;
    if !conflicts.is_empty() {
        return Err(conflicts_error(&conflicts));
    }
    sqlx::query(
        "UPDATE timetable_entries SET week = ?, day = ?, idx = ?, room_id = ?, locked = ? WHERE id = ? AND school_id = ?",
    )
    .bind(week)
    .bind(day)
    .bind(idx)
    .bind(&room_id)
    .bind(if locked { 1 } else { 0 })
    .bind(&eid)
    .bind(&user.school_id)
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "timetable.move",
        "entry",
        Some(&eid),
        format!("Moved a lesson to week {} day {} P{}{}", week + 1, day, idx, if locked { " (locked)" } else { "" }),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_entry(
    State(st): State<AppState>,
    user: AuthUser,
    Path(eid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    sqlx::query("DELETE FROM timetable_entries WHERE id = ? AND school_id = ?")
        .bind(&eid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    audit::record(
        &st.pool,
        &user,
        "timetable.remove",
        "entry",
        Some(&eid),
        "Removed a lesson from the timetable".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn clear_entries(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let n = sqlx::query("DELETE FROM timetable_entries WHERE school_id = ?")
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?
        .rows_affected();
    audit::record(
        &st.pool,
        &user,
        "timetable.clear",
        "timetable",
        None,
        format!("Cleared the timetable ({n} entries)"),
    )
    .await;
    Ok(Json(json!({ "ok": true, "cleared": n })))
}

// ---------------------------------------------------------------------------
// export
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub entity: Option<String>, // class | teacher | room
    pub id: Option<String>,
    pub week: Option<i64>,
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export_csv(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<ExportQuery>,
) -> ApiResult<impl IntoResponse> {
    let entity = q.entity.unwrap_or_else(|| "class".to_string());
    let week = q.week.unwrap_or(0).clamp(0, 1);
    let school = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    let cycle_weeks: i64 = school.get("cycle_weeks");

    let filter_col = match entity.as_str() {
        "teacher" => "l.teacher_id",
        "room" => "e.room_id",
        _ => "l.class_id",
    };
    let table = match entity.as_str() {
        "teacher" => "teachers",
        "room" => "rooms",
        _ => "classes",
    };
    // Fall back to the first entity when the caller omits the id, so a bare
    // /api/export/timetable.csv still produces a useful (non-empty) file.
    let id: String = match q.id.clone().filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE school_id = ? ORDER BY name LIMIT 1"
        )))
        .bind(&user.school_id)
        .fetch_optional(&st.pool)
        .await?
        .map(|r| r.get::<String, _>("id"))
        .unwrap_or_default(),
    };

    let entries = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{ENTRY_SELECT} WHERE e.school_id = ? AND {filter_col} = ? AND e.week = ? ORDER BY e.day, e.idx"
    )))
    .bind(&user.school_id)
    .bind(&id)
    .bind(week)
    .fetch_all(&st.pool)
    .await?;

    let name = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT name FROM {table} WHERE id = ?")))
        .bind(&id)
        .fetch_optional(&st.pool)
        .await?
        .map(|r| r.get::<String, _>("name"))
        .unwrap_or_else(|| "timetable".to_string());

    let slots = sqlx::query(
        "SELECT day, idx, start_time, end_time, is_break FROM time_slots WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(&user.school_id)
    .fetch_all(&st.pool)
    .await?;
    let days: Vec<i64> = {
        let mut d: Vec<i64> = slots.iter().map(|r| r.get::<i64, _>("day")).collect();
        d.sort_unstable();
        d.dedup();
        d
    };
    let max_idx: i64 = slots.iter().map(|r| r.get::<i64, _>("idx")).max().unwrap_or(0);

    let mut lines: Vec<String> = Vec::new();
    let mut header = vec!["Period".to_string(), "Time".to_string()];
    for d in &days {
        header.push(day_name(*d).to_string());
    }
    lines.push(header.join(","));

    let week_label = if cycle_weeks > 1 {
        if week == 0 { " (Week A)" } else { " (Week B)" }
    } else {
        ""
    };
    lines.push(csv_escape(&format!("{entity} timetable: {name}{week_label}")));

    for idx in 1..=max_idx {
        let time = slots
            .iter()
            .find(|r| r.get::<i64, _>("idx") == idx)
            .map(|r| format!("{} - {}", r.get::<String, _>("start_time"), r.get::<String, _>("end_time")))
            .unwrap_or_default();
        let is_break = slots
            .iter()
            .filter(|r| r.get::<i64, _>("idx") == idx)
            .all(|r| r.get::<i64, _>("is_break") != 0);
        let mut row = vec![idx.to_string(), csv_escape(&time)];
        for d in &days {
            let cell = entries
                .iter()
                .find(|e| e.get::<i64, _>("day") == *d && e.get::<i64, _>("idx") == idx)
                .map(|e| {
                    let room = e.get::<Option<String>, _>("room_name").unwrap_or_default();
                    let room_part = if room.is_empty() { String::new() } else { format!(" ({room})") };
                    format!(
                        "{} - {}{}",
                        e.get::<String, _>("subject_code"),
                        e.get::<String, _>("teacher_initials"),
                        room_part
                    )
                })
                .unwrap_or_else(|| if is_break { "BREAK".to_string() } else { String::new() });
            row.push(csv_escape(&cell));
        }
        lines.push(row.join(","));
    }

    let csv = lines.join("\n");
    let filename = format!(
        "timetable-{}-{}.csv",
        entity,
        name.to_lowercase().replace([' ', '/', '\\'], "-")
    );
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    ))
}

// ---------------------------------------------------------------------------
// ICS / PDF export
// ---------------------------------------------------------------------------

fn filter_column(entity: &str) -> &'static str {
    match entity {
        "teacher" => "l.teacher_id",
        "room" => "e.room_id",
        _ => "l.class_id",
    }
}

/// Resolves the (id, display name) an export targets, defaulting to the first
/// entity of the requested kind when no id is given.
async fn resolve_target(
    pool: &SqlitePool,
    school_id: &str,
    entity: &str,
    id: Option<String>,
) -> ApiResult<(String, String)> {
    let table = match entity {
        "teacher" => "teachers",
        "room" => "rooms",
        _ => "classes",
    };
    let id: String = match id.filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE school_id = ? ORDER BY name LIMIT 1"
        )))
        .bind(school_id)
        .fetch_optional(pool)
        .await?
        .map(|r| r.get::<String, _>("id"))
        .unwrap_or_default(),
    };
    let name = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT name FROM {table} WHERE id = ?")))
        .bind(&id)
        .fetch_optional(pool)
        .await?
        .map(|r| r.get::<String, _>("name"))
        .unwrap_or_else(|| "timetable".to_string());
    Ok((id, name))
}

/// Period times keyed by (day, idx).
async fn slot_times(
    pool: &SqlitePool,
    school_id: &str,
) -> ApiResult<Vec<(i64, i64, String, String, bool)>> {
    let rows = sqlx::query(
        "SELECT day, idx, start_time, end_time, is_break FROM time_slots WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(school_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get::<i64, _>("day"),
                r.get::<i64, _>("idx"),
                r.get::<String, _>("start_time"),
                r.get::<String, _>("end_time"),
                r.get::<i64, _>("is_break") != 0,
            )
        })
        .collect())
}

pub async fn export_ics(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<ExportQuery>,
) -> ApiResult<impl IntoResponse> {
    let entity = q.entity.unwrap_or_else(|| "class".to_string());
    let week = q.week.unwrap_or(0).clamp(0, 1);
    let occurrences: u32 = 12;
    let school = sqlx::query("SELECT cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    let cycle_weeks: i64 = school.get("cycle_weeks");
    let (id, name) = resolve_target(&st.pool, &user.school_id, &entity, q.id).await?;

    let entries = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{ENTRY_SELECT} WHERE e.school_id = ? AND {} = ? AND e.week = ? ORDER BY e.day, e.idx",
        filter_column(&entity)
    )))
    .bind(&user.school_id)
    .bind(&id)
    .bind(week)
    .fetch_all(&st.pool)
    .await?;
    let times = slot_times(&st.pool, &user.school_id).await?;

    let t = today();
    let monday = t - Duration::days(day_of(&t) - 1);
    let interval = cycle_weeks.clamp(1, 2) as u32;
    let events: Vec<crate::ical::Event> = entries
        .iter()
        .map(|e| {
            let day: i64 = e.get("day");
            let idx: i64 = e.get("idx");
            let wk: i64 = e.get("week");
            let (_, _, start, end, _) = times
                .iter()
                .find(|(d, i, _, _, _)| *d == day && *i == idx)
                .cloned()
                .unwrap_or((day, idx, String::new(), String::new(), false));
            let date = monday + Duration::days(wk * 7 + (day - 1));
            let class_name: String = e.get("class_name");
            let subject: String = e.get("subject_name");
            let teacher: String = e.get("teacher_name");
            crate::ical::Event {
                uid: format!("{}-{}@timetablemaster", e.get::<String, _>("id"), wk),
                date,
                start,
                end,
                summary: format!("{subject} · {class_name}"),
                location: e.get::<Option<String>, _>("room_name"),
                description: Some(format!("Teacher: {teacher}")),
            }
        })
        .collect();

    let cal = crate::ical::render(
        &format!("{entity} timetable: {name}"),
        interval,
        occurrences,
        &events,
    );
    let filename = format!(
        "timetable-{}-{}.ics",
        entity,
        name.to_lowercase().replace([' ', '/', '\\'], "-")
    );
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        cal,
    ))
}

pub async fn export_pdf(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<ExportQuery>,
) -> ApiResult<impl IntoResponse> {
    let entity = q.entity.unwrap_or_else(|| "class".to_string());
    let week = q.week.unwrap_or(0).clamp(0, 1);
    let school = sqlx::query("SELECT name, cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    let cycle_weeks: i64 = school.get("cycle_weeks");
    let school_name: String = school.get("name");
    let (id, name) = resolve_target(&st.pool, &user.school_id, &entity, q.id).await?;

    let entries = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{ENTRY_SELECT} WHERE e.school_id = ? AND {} = ? AND e.week = ? ORDER BY e.day, e.idx",
        filter_column(&entity)
    )))
    .bind(&user.school_id)
    .bind(&id)
    .bind(week)
    .fetch_all(&st.pool)
    .await?;
    let slots = slot_times(&st.pool, &user.school_id).await?;

    let mut days: Vec<i64> = slots.iter().map(|s| s.0).collect();
    days.sort_unstable();
    days.dedup();
    let max_idx = slots.iter().map(|s| s.1).max().unwrap_or(0);

    let mut headers = vec!["Period".to_string(), "Time".to_string()];
    for d in &days {
        headers.push(day_name(*d).to_string());
    }
    let mut weights = vec![0.45, 0.7];
    weights.extend(std::iter::repeat_n(1.0, days.len()));

    let mut rows: Vec<Vec<String>> = Vec::new();
    for idx in 1..=max_idx {
        let time = slots
            .iter()
            .find(|s| s.1 == idx)
            .map(|s| format!("{} - {}", s.2, s.3))
            .unwrap_or_default();
        let is_break = slots
            .iter()
            .filter(|s| s.1 == idx)
            .all(|s| s.4);
        let mut row = vec![format!("P{idx}"), time];
        for d in &days {
            let cell = entries
                .iter()
                .find(|e| e.get::<i64, _>("day") == *d && e.get::<i64, _>("idx") == idx)
                .map(|e| {
                    let room = e.get::<Option<String>, _>("room_name").unwrap_or_default();
                    let room_part = if room.is_empty() {
                        String::new()
                    } else {
                        format!(" ({room})")
                    };
                    format!(
                        "{} - {}{}",
                        e.get::<String, _>("subject_code"),
                        e.get::<String, _>("teacher_initials"),
                        room_part
                    )
                })
                .unwrap_or_else(|| {
                    if is_break {
                        "BREAK".to_string()
                    } else {
                        String::new()
                    }
                });
            row.push(cell);
        }
        rows.push(row);
    }

    let week_label = if cycle_weeks > 1 {
        if week == 0 { " — Week A" } else { " — Week B" }
    } else {
        ""
    };
    let meta = format!("{school_name} · {entity} timetable: {name}{week_label}");
    let bytes = crate::pdf::render(&crate::pdf::Table {
        title: format!("Timetable — {name}"),
        meta,
        headers,
        rows,
        weights,
    });
    let filename = format!(
        "timetable-{}-{}.pdf",
        entity,
        name.to_lowercase().replace([' ', '/', '\\'], "-")
    );
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        Bytes::from(bytes),
    ))
}
