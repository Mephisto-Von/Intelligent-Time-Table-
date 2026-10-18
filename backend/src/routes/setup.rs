use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::models::{
    BellScheduleInput, ClassInput, LessonInput, RoomInput, SubjectInput, TeacherInput,
};
use crate::state::{id, now, AppState};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn subject_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "name": r.get::<String, _>("name"),
        "code": r.get::<String, _>("code"),
        "color": r.get::<String, _>("color"),
    })
}

fn class_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "name": r.get::<String, _>("name"),
        "grade": r.get::<String, _>("grade"),
        "size": r.get::<i64, _>("size"),
    })
}

fn room_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "name": r.get::<String, _>("name"),
        "capacity": r.get::<i64, _>("capacity"),
        "room_type": r.get::<String, _>("room_type"),
    })
}

fn slot_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "day": r.get::<i64, _>("day"),
        "idx": r.get::<i64, _>("idx"),
        "start_time": r.get::<String, _>("start_time"),
        "end_time": r.get::<String, _>("end_time"),
        "label": r.get::<String, _>("label"),
        "is_break": r.get::<i64, _>("is_break") != 0,
    })
}

fn lesson_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String, _>("id"),
        "class_id": r.get::<String, _>("class_id"),
        "subject_id": r.get::<String, _>("subject_id"),
        "teacher_id": r.get::<String, _>("teacher_id"),
        "room_id": r.get::<Option<String>, _>("room_id"),
        "required_room_type": r.get::<Option<String>, _>("required_room_type"),
        "periods_per_week": r.get::<i64, _>("periods_per_week"),
        "max_per_day": r.get::<i64, _>("max_per_day"),
        "class_name": r.get::<String, _>("class_name"),
        "subject_name": r.get::<String, _>("subject_name"),
        "subject_code": r.get::<String, _>("subject_code"),
        "subject_color": r.get::<String, _>("subject_color"),
        "teacher_name": r.get::<String, _>("teacher_name"),
        "teacher_initials": r.get::<String, _>("teacher_initials"),
        "room_name": r.get::<Option<String>, _>("room_name"),
    })
}

fn derive_initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let mut initials = String::new();
    for p in parts.iter().take(3) {
        if let Some(c) = p.chars().next() {
            initials.push(c.to_ascii_uppercase());
        }
    }
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}

async fn fetch_subjects(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query("SELECT id, name, code, color FROM subjects WHERE school_id = ? ORDER BY name")
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(subject_json).collect())
}

async fn fetch_teachers(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT t.id, t.name, t.initials, t.email, t.max_weekly, t.max_daily,
                (SELECT COUNT(*) FROM lessons l WHERE l.teacher_id = t.id) AS lesson_count
         FROM teachers t WHERE t.school_id = ? ORDER BY t.name",
    )
    .bind(school_id)
    .fetch_all(pool)
    .await?;
    let subj_rows = sqlx::query("SELECT teacher_id, subject_id FROM teacher_subjects WHERE school_id = ?")
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    let mut subjects_map: HashMap<String, Vec<String>> = HashMap::new();
    for r in &subj_rows {
        subjects_map
            .entry(r.get::<String, _>("teacher_id"))
            .or_default()
            .push(r.get::<String, _>("subject_id"));
    }
    let unavail_rows = sqlx::query(
        "SELECT teacher_id, day, idx FROM teacher_unavailability WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(school_id)
    .fetch_all(pool)
    .await?;
    let mut unavail_map: HashMap<String, Vec<Value>> = HashMap::new();
    for r in &unavail_rows {
        unavail_map
            .entry(r.get::<String, _>("teacher_id"))
            .or_default()
            .push(json!({ "day": r.get::<i64, _>("day"), "idx": r.get::<i64, _>("idx") }));
    }

    Ok(rows
        .iter()
        .map(|r| {
            let tid: String = r.get("id");
            json!({
                "id": tid,
                "name": r.get::<String, _>("name"),
                "initials": r.get::<String, _>("initials"),
                "email": r.get::<Option<String>, _>("email"),
                "max_weekly": r.get::<i64, _>("max_weekly"),
                "max_daily": r.get::<i64, _>("max_daily"),
                "lesson_count": r.get::<i64, _>("lesson_count"),
                "subject_ids": subjects_map.get(&tid).cloned().unwrap_or_default(),
                "unavailable": unavail_map.get(&tid).cloned().unwrap_or_default(),
            })
        })
        .collect())
}

async fn fetch_classes(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query("SELECT id, name, grade, size FROM classes WHERE school_id = ? ORDER BY name")
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(class_json).collect())
}

async fn fetch_rooms(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query("SELECT id, name, capacity, room_type FROM rooms WHERE school_id = ? ORDER BY name")
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(room_json).collect())
}

async fn fetch_slots(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, day, idx, start_time, end_time, label, is_break FROM time_slots WHERE school_id = ? ORDER BY day, idx",
    )
    .bind(school_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(slot_json).collect())
}

const LESSON_SELECT: &str = "SELECT l.id, l.class_id, l.subject_id, l.teacher_id, l.room_id, l.required_room_type,
        l.periods_per_week, l.max_per_day,
        c.name AS class_name, s.name AS subject_name, s.code AS subject_code, s.color AS subject_color,
        t.name AS teacher_name, t.initials AS teacher_initials, r.name AS room_name, r.room_type AS room_type
 FROM lessons l
 JOIN classes c ON c.id = l.class_id
 JOIN subjects s ON s.id = l.subject_id
 JOIN teachers t ON t.id = l.teacher_id
 LEFT JOIN rooms r ON r.id = l.room_id";

async fn fetch_lessons(pool: &SqlitePool, school_id: &str) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{LESSON_SELECT} WHERE l.school_id = ? ORDER BY c.name, s.name"
    )))
        .bind(school_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(lesson_json).collect())
}

pub async fn seed_default_bell_schedule(st: &AppState, school_id: &str) -> ApiResult<()> {
    let exists: i64 = sqlx::query("SELECT COUNT(*) AS n FROM time_slots WHERE school_id = ?")
        .bind(school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if exists > 0 {
        return Ok(());
    }
    let ts = now();
    let periods = [
        ("08:30", "09:15", false),
        ("09:15", "10:00", false),
        ("10:00", "10:45", false),
        ("10:45", "11:30", true), // break
        ("11:30", "12:15", false),
        ("12:15", "13:00", false),
        ("13:00", "13:45", false),
        ("13:45", "14:30", false),
    ];
    let mut tx = st.pool.begin().await?;
    for day in 1..=5 {
        for (i, (start, end, is_break)) in periods.iter().enumerate() {
            sqlx::query(
                "INSERT INTO time_slots (id, school_id, day, idx, start_time, end_time, label, is_break) VALUES (?, ?, ?, ?, ?, ?, '', ?)",
            )
            .bind(id())
            .bind(school_id)
            .bind(day)
            .bind((i + 1) as i64)
            .bind(*start)
            .bind(*end)
            .bind(if *is_break { 1 } else { 0 })
            .execute(&mut *tx)
            .await?;
        }
    }
    let _ = ts;
    tx.commit().await?;
    Ok(())
}

pub async fn setup_all(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Value>> {
    let school = sqlx::query("SELECT name, cycle_weeks FROM schools WHERE id = ?")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?;
    Ok(Json(json!({
        "school": {
            "id": user.school_id,
            "name": school.get::<String, _>("name"),
            "cycle_weeks": school.get::<i64, _>("cycle_weeks"),
        },
        "subjects": fetch_subjects(&st.pool, &user.school_id).await?,
        "teachers": fetch_teachers(&st.pool, &user.school_id).await?,
        "classes": fetch_classes(&st.pool, &user.school_id).await?,
        "rooms": fetch_rooms(&st.pool, &user.school_id).await?,
        "slots": fetch_slots(&st.pool, &user.school_id).await?,
        "lessons": fetch_lessons(&st.pool, &user.school_id).await?,
    })))
}

// ---------------------------------------------------------------------------
// subjects
// ---------------------------------------------------------------------------

pub async fn create_subject(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<SubjectInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if input.name.trim().is_empty() || input.code.trim().is_empty() {
        return Err(ApiError::BadRequest("name and code are required".into()));
    }
    let sid = id();
    sqlx::query("INSERT INTO subjects (id, school_id, name, code, color, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&sid)
        .bind(&user.school_id)
        .bind(input.name.trim())
        .bind(input.code.trim())    .bind(input.color.unwrap_or_else(|| "#6366f1".into()))
    .bind(now())
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "subject.create",
        "subject",
        Some(&sid),
        format!("Added subject {}", input.name.trim()),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": sid }))))
}

pub async fn update_subject(
    State(st): State<AppState>,
    user: AuthUser,
    Path(sid): Path<String>,
    Json(input): Json<SubjectInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let n = sqlx::query("UPDATE subjects SET name = ?, code = ?, color = ? WHERE id = ? AND school_id = ?")
        .bind(input.name.trim())
        .bind(input.code.trim())
        .bind(input.color.unwrap_or_else(|| "#6366f1".into()))
        .bind(&sid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_subject(
    State(st): State<AppState>,
    user: AuthUser,
    Path(sid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let used: i64 = sqlx::query("SELECT COUNT(*) AS n FROM lessons WHERE subject_id = ?")
        .bind(&sid)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if used > 0 {
        return Err(ApiError::Conflict(format!("subject is used by {used} lesson(s)")));
    }
    sqlx::query("DELETE FROM teacher_subjects WHERE subject_id = ?").bind(&sid).execute(&st.pool).await?;
    sqlx::query("DELETE FROM subjects WHERE id = ? AND school_id = ?")
        .bind(&sid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    audit::record(
        &st.pool,
        &user,
        "subject.delete",
        "subject",
        Some(&sid),
        "Deleted a subject".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// teachers
// ---------------------------------------------------------------------------

pub async fn create_teacher(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<TeacherInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("teacher name is required".into()));
    }
    let tid = id();
    let max_weekly = input.max_weekly.unwrap_or(30).clamp(1, 80);
    let max_daily = input.max_daily.unwrap_or(6).clamp(1, 14);
    let initials = input.initials.clone().unwrap_or_else(|| derive_initials(&input.name));
    let ts = now();
    let mut tx = st.pool.begin().await?;
    sqlx::query(
        "INSERT INTO teachers (id, school_id, name, initials, email, max_weekly, max_daily, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&tid)
    .bind(&user.school_id)
    .bind(input.name.trim())
    .bind(&initials)
    .bind(&input.email)
    .bind(max_weekly)
    .bind(max_daily)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;
    write_teacher_links(&mut tx, &user.school_id, &tid, &input).await?;
    tx.commit().await?;
    audit::record(
        &st.pool,
        &user,
        "teacher.create",
        "teacher",
        Some(&tid),
        format!("Added teacher {}", input.name.trim()),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": tid }))))
}

async fn write_teacher_links(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    school_id: &str,
    teacher_id: &str,
    input: &TeacherInput,
) -> ApiResult<()> {
    for subject_id in &input.subject_ids {
        let exists: i64 = sqlx::query("SELECT COUNT(*) AS n FROM subjects WHERE id = ? AND school_id = ?")
            .bind(subject_id)
            .bind(school_id)
            .fetch_one(&mut **tx)
            .await?
            .get("n");
        if exists > 0 {
            sqlx::query("INSERT OR IGNORE INTO teacher_subjects (teacher_id, subject_id, school_id) VALUES (?, ?, ?)")
                .bind(teacher_id)
                .bind(subject_id)
                .bind(school_id)
                .execute(&mut **tx)
                .await?;
        }
    }
    for slot in &input.unavailable {
        if slot.day < 1 || slot.day > 7 || slot.idx < 1 || slot.idx > 20 {
            continue;
        }
        sqlx::query(
            "INSERT OR IGNORE INTO teacher_unavailability (id, teacher_id, school_id, day, idx) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(teacher_id)
        .bind(school_id)
        .bind(slot.day)
        .bind(slot.idx)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn update_teacher(
    State(st): State<AppState>,
    user: AuthUser,
    Path(tid): Path<String>,
    Json(input): Json<TeacherInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let max_weekly = input.max_weekly.unwrap_or(30).clamp(1, 80);
    let max_daily = input.max_daily.unwrap_or(6).clamp(1, 14);
    let initials = input.initials.clone().unwrap_or_else(|| derive_initials(&input.name));
    let mut tx = st.pool.begin().await?;
    let n = sqlx::query(
        "UPDATE teachers SET name = ?, initials = ?, email = ?, max_weekly = ?, max_daily = ? WHERE id = ? AND school_id = ?",
    )
    .bind(input.name.trim())
    .bind(&initials)
    .bind(&input.email)
    .bind(max_weekly)
    .bind(max_daily)
    .bind(&tid)
    .bind(&user.school_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    sqlx::query("DELETE FROM teacher_subjects WHERE teacher_id = ?").bind(&tid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM teacher_unavailability WHERE teacher_id = ?").bind(&tid).execute(&mut *tx).await?;
    write_teacher_links(&mut tx, &user.school_id, &tid, &input).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_teacher(
    State(st): State<AppState>,
    user: AuthUser,
    Path(tid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let used: i64 = sqlx::query("SELECT COUNT(*) AS n FROM lessons WHERE teacher_id = ?")
        .bind(&tid)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if used > 0 {
        return Err(ApiError::Conflict(format!("teacher has {used} lesson(s); remove those first")));
    }
    let mut tx = st.pool.begin().await?;
    sqlx::query("DELETE FROM teacher_subjects WHERE teacher_id = ?").bind(&tid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM teacher_unavailability WHERE teacher_id = ?").bind(&tid).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET teacher_id = NULL WHERE teacher_id = ?").bind(&tid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM teachers WHERE id = ? AND school_id = ?")
        .bind(&tid)
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::record(
        &st.pool,
        &user,
        "teacher.delete",
        "teacher",
        Some(&tid),
        "Deleted a teacher".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// classes
// ---------------------------------------------------------------------------

pub async fn create_class(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<ClassInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("class name is required".into()));
    }
    let cid = id();
    sqlx::query("INSERT INTO classes (id, school_id, name, grade, size, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&cid)
        .bind(&user.school_id)
        .bind(input.name.trim())
        .bind(input.grade.clone().unwrap_or_default())    .bind(input.size.unwrap_or(30).clamp(1, 500))
    .bind(now())
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "class.create",
        "class",
        Some(&cid),
        format!("Added class {}", input.name.trim()),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": cid }))))
}

pub async fn update_class(
    State(st): State<AppState>,
    user: AuthUser,
    Path(cid): Path<String>,
    Json(input): Json<ClassInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let n = sqlx::query("UPDATE classes SET name = ?, grade = ?, size = ? WHERE id = ? AND school_id = ?")
        .bind(input.name.trim())
        .bind(input.grade.clone().unwrap_or_default())
        .bind(input.size.unwrap_or(30).clamp(1, 500))
        .bind(&cid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_class(
    State(st): State<AppState>,
    user: AuthUser,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let used: i64 = sqlx::query("SELECT COUNT(*) AS n FROM lessons WHERE class_id = ?")
        .bind(&cid)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    if used > 0 {
        return Err(ApiError::Conflict(format!("class has {used} lesson(s); remove those first")));
    }
    let mut tx = st.pool.begin().await?;
    sqlx::query("UPDATE users SET class_id = NULL WHERE class_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM classes WHERE id = ? AND school_id = ?")
        .bind(&cid)
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::record(
        &st.pool,
        &user,
        "class.delete",
        "class",
        Some(&cid),
        "Deleted a class".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// rooms
// ---------------------------------------------------------------------------

pub async fn create_room(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<RoomInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("room name is required".into()));
    }
    let rid = id();
    sqlx::query("INSERT INTO rooms (id, school_id, name, capacity, room_type, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&rid)
        .bind(&user.school_id)
        .bind(input.name.trim())
        .bind(input.capacity.unwrap_or(30).clamp(1, 2000))    .bind(input.room_type.unwrap_or_else(|| "general".into()))
    .bind(now())
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "room.create",
        "room",
        Some(&rid),
        format!("Added room {}", input.name.trim()),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": rid }))))
}

pub async fn update_room(
    State(st): State<AppState>,
    user: AuthUser,
    Path(rid): Path<String>,
    Json(input): Json<RoomInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let n = sqlx::query("UPDATE rooms SET name = ?, capacity = ?, room_type = ? WHERE id = ? AND school_id = ?")
        .bind(input.name.trim())
        .bind(input.capacity.unwrap_or(30).clamp(1, 2000))
        .bind(input.room_type.unwrap_or_else(|| "general".into()))
        .bind(&rid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_room(
    State(st): State<AppState>,
    user: AuthUser,
    Path(rid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let used: i64 =    sqlx::query(
        "SELECT (SELECT COUNT(*) FROM lessons WHERE room_id = ? AND school_id = ?) + (SELECT COUNT(*) FROM timetable_entries WHERE room_id = ? AND school_id = ?) AS n",
    )
    .bind(&rid)
    .bind(&user.school_id)
    .bind(&rid)
    .bind(&user.school_id)
    .fetch_one(&st.pool)
    .await?
    .get("n");
    if used > 0 {
        return Err(ApiError::Conflict("room is referenced by lessons or timetable entries".into()));
    }
    sqlx::query("DELETE FROM rooms WHERE id = ? AND school_id = ?")
        .bind(&rid)
        .bind(&user.school_id)
        .execute(&st.pool)
        .await?;
    audit::record(
        &st.pool,
        &user,
        "room.delete",
        "room",
        Some(&rid),
        "Deleted a room".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// bell schedule
// ---------------------------------------------------------------------------

pub async fn replace_bell_schedule(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<BellScheduleInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    if input.slots.is_empty() {
        return Err(ApiError::BadRequest("at least one slot is required".into()));
    }
    if input.slots.len() > 7 * 20 {
        return Err(ApiError::BadRequest("too many slots".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for s in &input.slots {
        if s.day < 1 || s.day > 7 || s.idx < 1 || s.idx > 20 {
            return Err(ApiError::BadRequest("day must be 1-7 and period 1-20".into()));
        }
        if !seen.insert((s.day, s.idx)) {
            return Err(ApiError::BadRequest("duplicate day/period in bell schedule".into()));
        }
    }
    let mut tx = st.pool.begin().await?;
    sqlx::query("DELETE FROM time_slots WHERE school_id = ?")
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?;
    for s in &input.slots {
        sqlx::query(
            "INSERT INTO time_slots (id, school_id, day, idx, start_time, end_time, label, is_break) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(&user.school_id)
        .bind(s.day)
        .bind(s.idx)
        .bind(s.start_time.trim())
        .bind(s.end_time.trim())
        .bind(s.label.clone().unwrap_or_default())
        .bind(if s.is_break.unwrap_or(false) { 1 } else { 0 })
        .execute(&mut *tx)
        .await?;
    }
    let cleared = sqlx::query("DELETE FROM timetable_entries WHERE school_id = ?")
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    audit::record(
        &st.pool,
        &user,
        "bell.update",
        "bell_schedule",
        None,
        format!("Replaced the bell schedule ({} slots); cleared {cleared} timetable entries", input.slots.len()),
    )
    .await;
    Ok(Json(json!({ "ok": true, "slots": input.slots.len(), "cleared_entries": cleared })))
}

// ---------------------------------------------------------------------------
// lessons
// ---------------------------------------------------------------------------

pub async fn create_lesson(
    State(st): State<AppState>,
    user: AuthUser,
    Json(input): Json<LessonInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    user.require_staff()?;
    if input.periods_per_week < 1 || input.periods_per_week > 40 {
        return Err(ApiError::BadRequest("periods per week must be between 1 and 40".into()));
    }
    let days: i64 = sqlx::query("SELECT COUNT(DISTINCT day) AS n FROM time_slots WHERE school_id = ? AND is_break = 0")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    let days = days.max(1);
    let max_per_day = input
        .max_per_day
        .unwrap_or_else(|| (input.periods_per_week + days - 1) / days)
        .clamp(1, 10);
    for (table, value) in [
        ("classes", &input.class_id),
        ("subjects", &input.subject_id),
        ("teachers", &input.teacher_id),
    ] {
        let exists: i64 = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) AS n FROM {table} WHERE id = ? AND school_id = ?"
        )))
            .bind(value)
            .bind(&user.school_id)
            .fetch_one(&st.pool)
            .await?
            .get("n");
        if exists == 0 {
            return Err(ApiError::BadRequest(format!("{table} reference not found")));
        }
    }
    if let Some(room_id) = &input.room_id {
        let exists: i64 = sqlx::query("SELECT COUNT(*) AS n FROM rooms WHERE id = ? AND school_id = ?")
            .bind(room_id)
            .bind(&user.school_id)
            .fetch_one(&st.pool)
            .await?
            .get("n");
        if exists == 0 {
            return Err(ApiError::BadRequest("room not found".into()));
        }
    }
    let lid = id();
    sqlx::query(
        "INSERT INTO lessons (id, school_id, class_id, subject_id, teacher_id, room_id, required_room_type, periods_per_week, max_per_day, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&lid)
    .bind(&user.school_id)
    .bind(&input.class_id)
    .bind(&input.subject_id)
    .bind(&input.teacher_id)
    .bind(&input.room_id)
    .bind(&input.required_room_type)
    .bind(input.periods_per_week)
    .bind(max_per_day)
    .bind(now())
    .execute(&st.pool)
    .await?;
    audit::record(
        &st.pool,
        &user,
        "lesson.create",
        "lesson",
        Some(&lid),
        format!("Added a lesson ({} periods/week)", input.periods_per_week),
    )
    .await;
    Ok((StatusCode::CREATED, Json(json!({ "id": lid }))))
}

pub async fn update_lesson(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
    Json(input): Json<LessonInput>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    if input.periods_per_week < 1 || input.periods_per_week > 40 {
        return Err(ApiError::BadRequest("periods per week must be between 1 and 40".into()));
    }
    let days: i64 = sqlx::query("SELECT COUNT(DISTINCT day) AS n FROM time_slots WHERE school_id = ? AND is_break = 0")
        .bind(&user.school_id)
        .fetch_one(&st.pool)
        .await?
        .get("n");
    let days = days.max(1);
    let max_per_day = input
        .max_per_day
        .unwrap_or_else(|| (input.periods_per_week + days - 1) / days)
        .clamp(1, 10);
    let n = sqlx::query(
        "UPDATE lessons SET class_id = ?, subject_id = ?, teacher_id = ?, room_id = ?, required_room_type = ?, periods_per_week = ?, max_per_day = ? WHERE id = ? AND school_id = ?",
    )
    .bind(&input.class_id)
    .bind(&input.subject_id)
    .bind(&input.teacher_id)
    .bind(&input.room_id)
    .bind(&input.required_room_type)
    .bind(input.periods_per_week)
    .bind(max_per_day)
    .bind(&lid)
    .bind(&user.school_id)
    .execute(&st.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    audit::record(
        &st.pool,
        &user,
        "lesson.update",
        "lesson",
        Some(&lid),
        format!("Updated a lesson ({} periods/week)", input.periods_per_week),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_lesson(
    State(st): State<AppState>,
    user: AuthUser,
    Path(lid): Path<String>,
) -> ApiResult<Json<Value>> {
    user.require_staff()?;
    let mut tx = st.pool.begin().await?;
    sqlx::query("DELETE FROM timetable_entries WHERE lesson_id = ?").bind(&lid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM lessons WHERE id = ? AND school_id = ?")
        .bind(&lid)
        .bind(&user.school_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::record(
        &st.pool,
        &user,
        "lesson.delete",
        "lesson",
        Some(&lid),
        "Deleted a lesson".to_string(),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}
