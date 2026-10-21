//! Builds a realistic demo school and runs the timetable solver over it.

use serde_json::json;
use sqlx::Row;

use crate::auth::hash_password;
use crate::dates::today;
use crate::solver::{self, ClassIn, LessonIn, RoomIn, Slot, TeacherIn, Weights};
use crate::state::{id, now, AppState};

pub const DEMO_EMAIL: &str = "admin@demo.school";
pub const DEMO_PASSWORD: &str = "demo1234";
pub const DEMO_ACCOUNT_NAME: &str = "Demo Academy";

struct SubjectSeed {
    name: &'static str,
    code: &'static str,
    color: &'static str,
}

struct TeacherSeed {
    name: &'static str,
    initials: &'static str,
    subjects: &'static [usize],
    unavailable: &'static [(i64, i64)],
    max_weekly: i64,
    max_daily: i64,
}

/// (class index, subject index, teacher index, periods, max per day, required room type)
struct LessonSeed(usize, usize, usize, i64, i64, Option<&'static str>);

const SUBJECTS: &[SubjectSeed] = &[
    SubjectSeed { name: "Mathematics", code: "MAT", color: "#4f46e5" },
    SubjectSeed { name: "English", code: "ENG", color: "#0891b2" },
    SubjectSeed { name: "Physics", code: "PHY", color: "#7c3aed" },
    SubjectSeed { name: "Chemistry", code: "CHE", color: "#059669" },
    SubjectSeed { name: "Biology", code: "BIO", color: "#16a34a" },
    SubjectSeed { name: "History", code: "HIS", color: "#b45309" },
    SubjectSeed { name: "Geography", code: "GEO", color: "#0d9488" },
    SubjectSeed { name: "Computer Science", code: "CSC", color: "#2563eb" },
    SubjectSeed { name: "Art", code: "ART", color: "#db2777" },
    SubjectSeed { name: "Physical Education", code: "PE", color: "#ea580c" },
];

const TEACHERS: &[TeacherSeed] = &[
    TeacherSeed { name: "Sarah Mitchell", initials: "SM", subjects: &[0], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "David Chen", initials: "DC", subjects: &[0], unavailable: &[(1, 1), (1, 2)], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Emma Hughes", initials: "EH", subjects: &[1], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "James Okafor", initials: "JO", subjects: &[1], unavailable: &[(5, 6), (5, 7)], max_weekly: 28, max_daily: 7 },
    TeacherSeed { name: "Priya Sharma", initials: "PS", subjects: &[2], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Tom Becker", initials: "TB", subjects: &[3], unavailable: &[(3, 1)], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Lena Fischer", initials: "LF", subjects: &[4], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Marco Rossi", initials: "MR", subjects: &[5], unavailable: &[], max_weekly: 26, max_daily: 6 },
    TeacherSeed { name: "Aisha Bello", initials: "AB", subjects: &[6], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Kevin Park", initials: "KP", subjects: &[7], unavailable: &[], max_weekly: 26, max_daily: 6 },
    TeacherSeed { name: "Nina Novak", initials: "NN", subjects: &[8], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Omar Haddad", initials: "OH", subjects: &[9], unavailable: &[], max_weekly: 30, max_daily: 7 },
    TeacherSeed { name: "Rachel Green", initials: "RG", subjects: &[1, 5], unavailable: &[], max_weekly: 28, max_daily: 7 },
    TeacherSeed { name: "Daniel Kim", initials: "DK", subjects: &[0, 7], unavailable: &[], max_weekly: 30, max_daily: 7 },
];

fn lesson_seeds() -> Vec<LessonSeed> {
    let mut out = Vec::new();
    // teachers: 0 SM, 1 DC (math) | 2 EH, 12 RG (english) | 4 PS (phys) | 5 TB (chem)
    // 6 LF (bio) | 7 MR, 12 RG (history) | 8 AB (geo) | 9 KP, 13 DK (cs) | 10 NN (art) | 11 OH (pe)
    for class in 0..6usize {
        let math = if class < 3 { 0 } else { 1 };
        let english = if class < 4 { 2 } else { 12 };
        let history = if class < 2 { 12 } else { 7 };
        let cs = if class < 4 { 9 } else { 13 };
        out.push(LessonSeed(class, 0, math, 5, 1, None));
        out.push(LessonSeed(class, 1, english, 4, 1, None));
        out.push(LessonSeed(class, 2, 4, 3, 1, Some("lab")));
        out.push(LessonSeed(class, 3, 5, 3, 1, None));
        out.push(LessonSeed(class, 4, 6, 2, 1, None));
        out.push(LessonSeed(class, 5, history, 2, 1, None));
        out.push(LessonSeed(class, 6, 8, 2, 1, None));
        out.push(LessonSeed(class, 7, cs, 2, 1, None));
        out.push(LessonSeed(class, 8, 10, 2, 1, Some("art")));
        out.push(LessonSeed(class, 9, 11, 2, 1, Some("gym")));
    }
    out
}

const CLASSES: &[(&str, &str, i64)] = &[
    ("Grade 7A", "7", 26),
    ("Grade 7B", "7", 28),
    ("Grade 8A", "8", 30),
    ("Grade 8B", "8", 24),
    ("Grade 9A", "9", 28),
    ("Grade 9B", "9", 26),
];

const ROOMS: &[(&str, &str, i64)] = &[
    ("Room 101", "general", 30),
    ("Room 102", "general", 30),
    ("Room 103", "general", 28),
    ("Room 104", "general", 26),
    ("Physics Lab", "lab", 30),
    ("Art Studio", "art", 30),
    ("Sports Hall", "gym", 60),
];

pub struct SeedSummary {
    pub school_name: String,
    pub placed: usize,
    pub requested: usize,
    pub teachers: usize,
    pub classes: usize,
    pub rooms: usize,
    pub lessons: usize,
}

/// Seeds a fresh demo account + school. Assumes the account does not exist yet.
pub async fn seed_demo(st: &AppState, account_id: &str, school_id: &str) -> anyhow::Result<SeedSummary> {
    let ts = now();
    let hash = hash_password(DEMO_PASSWORD)?;

    let mut tx = st.pool.begin().await?;
    sqlx::query("INSERT INTO schools (id, account_id, name, cycle_weeks, created_at) VALUES (?, ?, ?, 1, ?)")
        .bind(school_id)
        .bind(account_id)
        .bind("Demo High School")
        .bind(&ts)
        .execute(&mut *tx)
        .await?;

    // users
    let mut users: Vec<(String, String, String, &str, Option<String>, Option<String>)> = vec![
        (DEMO_EMAIL.to_string(), "Sam Rivera".to_string(), hash.clone(), "admin", None, None),
        ("scheduler@demo.school".to_string(), "Alex Morgan".to_string(), hash.clone(), "scheduler", None, None),
    ];
    let mut teacher_email_names = vec![
        ("david.chen", 1usize, "David Chen"),
        ("emma.hughes", 2, "Emma Hughes"),
        ("priya.sharma", 4, "Priya Sharma"),
        ("tom.becker", 5, "Tom Becker"),
        ("omar.haddad", 11, "Omar Haddad"),
    ];
    teacher_email_names.sort_by_key(|(_, i, _)| *i);
    for (slug, _, name) in &teacher_email_names {
        users.push((
            format!("{slug}@demo.school"),
            name.to_string(),
            hash.clone(),
            "teacher",
            None, // teacher link filled after teachers are inserted
            None,
        ));
    }
    users.push(("student@demo.school".to_string(), "Maya Patel".to_string(), hash.clone(), "student", None, None));
    users.push(("student2@demo.school".to_string(), "Leo Santos".to_string(), hash.clone(), "student", None, None));

    // subjects
    let subject_ids: Vec<String> = SUBJECTS.iter().map(|_| id()).collect();
    for (i, s) in SUBJECTS.iter().enumerate() {
        sqlx::query("INSERT INTO subjects (id, school_id, name, code, color, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&subject_ids[i])
            .bind(school_id)
            .bind(s.name)
            .bind(s.code)
            .bind(s.color)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
    }

    // bell schedule: 5 days x 8 periods, period 4 is lunch
    let bells = [
        ("08:30", "09:15", "", false),
        ("09:15", "10:00", "", false),
        ("10:00", "10:45", "", false),
        ("10:45", "11:30", "Lunch", true),
        ("11:30", "12:15", "", false),
        ("12:15", "13:00", "", false),
        ("13:00", "13:45", "", false),
        ("13:45", "14:30", "", false),
    ];
    for day in 1..=5i64 {
        for (i, (start, end, label, is_break)) in bells.iter().enumerate() {
            sqlx::query(
                "INSERT INTO time_slots (id, school_id, day, idx, start_time, end_time, label, is_break) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id())
            .bind(school_id)
            .bind(day)
            .bind((i + 1) as i64)
            .bind(*start)
            .bind(*end)
            .bind(*label)
            .bind(if *is_break { 1 } else { 0 })
            .execute(&mut *tx)
            .await?;
        }
    }

    // rooms
    let room_ids: Vec<String> = ROOMS.iter().map(|_| id()).collect();
    for (i, (name, room_type, capacity)) in ROOMS.iter().enumerate() {
        sqlx::query("INSERT INTO rooms (id, school_id, name, capacity, room_type, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&room_ids[i])
            .bind(school_id)
            .bind(name)
            .bind(capacity)
            .bind(room_type)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
    }

    // classes
    let class_ids: Vec<String> = CLASSES.iter().map(|_| id()).collect();
    for (i, (name, grade, size)) in CLASSES.iter().enumerate() {
        sqlx::query("INSERT INTO classes (id, school_id, name, grade, size, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&class_ids[i])
            .bind(school_id)
            .bind(name)
            .bind(grade)
            .bind(size)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
    }

    // teachers
    let teacher_ids: Vec<String> = TEACHERS.iter().map(|_| id()).collect();
    for (i, t) in TEACHERS.iter().enumerate() {
        sqlx::query(
            "INSERT INTO teachers (id, school_id, name, initials, email, max_weekly, max_daily, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&teacher_ids[i])
        .bind(school_id)
        .bind(t.name)
        .bind(t.initials)
        .bind(format!("{}@demo.school", t.name.to_lowercase().replace(' ', ".")))
        .bind(t.max_weekly)
        .bind(t.max_daily)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        for subj in t.subjects {
            sqlx::query("INSERT OR IGNORE INTO teacher_subjects (teacher_id, subject_id, school_id) VALUES (?, ?, ?)")
                .bind(&teacher_ids[i])
                .bind(&subject_ids[*subj])
                .bind(school_id)
                .execute(&mut *tx)
                .await?;
        }
        for (day, idx) in t.unavailable {
            sqlx::query(
                "INSERT OR IGNORE INTO teacher_unavailability (id, teacher_id, school_id, day, idx) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id())
            .bind(&teacher_ids[i])
            .bind(school_id)
            .bind(*day)
            .bind(*idx)
            .execute(&mut *tx)
            .await?;
        }
    }

    // users (with teacher/class links)
    for (email, name, hash, role, _, _) in &users {
        let teacher_link = if role == &"teacher" {
            let slug = email.split('@').next().unwrap_or("");
            teacher_email_names
                .iter()
                .find(|(s, _, _)| s == &slug)
                .map(|(_, ti, _)| teacher_ids[*ti].clone())
        } else {
            None
        };
        let class_link = match email.as_str() {
            "student@demo.school" => Some(class_ids[0].clone()),
            "student2@demo.school" => Some(class_ids[5].clone()),
            _ => None,
        };
        sqlx::query(
            "INSERT INTO users (id, account_id, school_id, email, password_hash, name, role, teacher_id, class_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(account_id)
        .bind(school_id)
        .bind(email)
        .bind(hash)
        .bind(name)
        .bind(role)
        .bind(&teacher_link)
        .bind(&class_link)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    // lessons
    let seeds = lesson_seeds();
    let lesson_ids: Vec<String> = seeds.iter().map(|_| id()).collect();
    for (i, LessonSeed(class, subject, teacher, periods, max_per_day, room_type)) in seeds.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lessons (id, school_id, class_id, subject_id, teacher_id, room_id, required_room_type, periods_per_week, max_per_day, created_at) VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(&lesson_ids[i])
        .bind(school_id)
        .bind(&class_ids[*class])
        .bind(&subject_ids[*subject])
        .bind(&teacher_ids[*teacher])
        .bind(*room_type)
        .bind(*periods)
        .bind(*max_per_day)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    // calendar events
    let t = today();
    let fmt = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
    let events = [
        ("Sports Day", t + chrono::Duration::days(10), t + chrono::Duration::days(10), "activity", "Whole-school sports day"),
        ("Staff Training Day", t + chrono::Duration::days(14), t + chrono::Duration::days(14), "holiday", "No lessons"),
        ("Founders' Day", t + chrono::Duration::days(21), t + chrono::Duration::days(21), "holiday", "School closed"),
        ("Midterm exams", t + chrono::Duration::days(42), t + chrono::Duration::days(46), "exam", "Exam week"),
    ];
    for (title, start, end, kind, notes) in events {
        sqlx::query("INSERT INTO calendar_events (id, school_id, title, start_date, end_date, kind, notes, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(id())
            .bind(school_id)
            .bind(title)
            .bind(fmt(start))
            .bind(fmt(end))
            .bind(kind)
            .bind(notes)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
    }

    // leave requests
    let leave_seeds = [
        (0usize, t, t + chrono::Duration::days(1), "full", "Family matters", "approved"),
        (5, t, t, "pm", "Medical appointment", "approved"),
        (4, t, t + chrono::Duration::days(1), "full", "Conference", "pending"),
    ];
    for (ti, start, end, part, reason, status) in leave_seeds {
        sqlx::query(
            "INSERT INTO leave_requests (id, school_id, teacher_id, start_date, end_date, part, reason, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(school_id)
        .bind(&teacher_ids[ti])
        .bind(fmt(start))
        .bind(fmt(end))
        .bind(part)
        .bind(reason)
        .bind(status)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // ---- run the solver over the freshly seeded data ----------------------
    let mut slots: Vec<Slot> = Vec::new();
    for day in 1..=5i64 {
        for idx in 1..=8i64 {
            if idx != 4 {
                slots.push(Slot { day, idx });
            }
        }
    }

    let solver_lessons: Vec<LessonIn> = seeds
        .iter()
        .enumerate()
        .map(|(i, LessonSeed(class, subject, teacher, periods, max_per_day, room_type))| LessonIn {
            id: lesson_ids[i].clone(),
            class: *class,
            teacher: *teacher,
            subject: *subject,
            periods: *periods as u32,
            max_per_day: *max_per_day as u32,
            room: None,
            required_room_type: room_type.map(|s| s.to_string()),
            fixed: vec![],
        })
        .collect();

    let solver_teachers: Vec<TeacherIn> = TEACHERS
        .iter()
        .map(|t| TeacherIn {
            id: t.name.to_string(),
            max_weekly: Some(t.max_weekly as u32),
            max_daily: Some(t.max_daily as u32),
            unavailable: t.unavailable.iter().map(|&(d, i)| (d, i)).collect(),
        })
        .collect();

    let solver_classes: Vec<ClassIn> = CLASSES
        .iter()
        .map(|(name, _, size)| ClassIn { id: name.to_string(), size: *size })
        .collect();

    let solver_rooms: Vec<RoomIn> = ROOMS
        .iter()
        .map(|(name, room_type, capacity)| RoomIn {
            id: name.to_string(),
            capacity: *capacity,
            room_type: room_type.to_string(),
        })
        .collect();

    let input = solver::Input {
        slots,
        weeks: 1,
        lessons: solver_lessons,
        teachers: solver_teachers,
        classes: solver_classes,
        rooms: solver_rooms,
        weights: Weights::default(),
        seed: 20260214,
        // Fixed iteration count instead of a wall-clock budget: the annealing
        // loop with `iterations: None` stops after `time_limit_ms` of real time,
        // so every machine (and every CI run) seeded a *different* demo
        // timetable. Black-box suites that predict the server's conflict rules
        // then saw different data than the ones that passed locally.
        time_limit_ms: 0,
        iterations: Some(1_000_000),
    };
    let requested: usize = seeds.iter().map(|LessonSeed(_, _, _, p, _, _)| *p as usize).sum();
    let output = tokio::task::spawn_blocking(move || solver::run(&input))
        .await
        .map_err(|e| anyhow::anyhow!("solver failed: {e}"))?;

    let mut tx = st.pool.begin().await?;
    let ts = now();
    for p in &output.placed {
        let (day, idx) = (1 + p.slot / 7, {
            let within = p.slot % 7;
            if within >= 3 { within + 2 } else { within + 1 }
        });
        let room_id = p.room.map(|r| room_ids[r].clone());
        sqlx::query(
            "INSERT INTO timetable_entries (id, school_id, lesson_id, week, day, idx, room_id, locked, created_at) VALUES (?, ?, ?, 0, ?, ?, ?, 0, ?)",
        )
        .bind(id())
        .bind(school_id)
        .bind(&lesson_ids[p.lesson])
        .bind(day as i64)
        .bind(idx as i64)
        .bind(room_id)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(SeedSummary {
        school_name: "Demo High School".to_string(),
        placed: output.placed.len(),
        requested,
        teachers: TEACHERS.len(),
        classes: CLASSES.len(),
        rooms: ROOMS.len(),
        lessons: seeds.len(),
    })
}

/// Creates the demo account if missing and returns the admin user id + school id.
pub async fn ensure_demo_account(st: &AppState) -> anyhow::Result<(String, String, bool)> {
    let existing = sqlx::query("SELECT u.id, u.school_id, u.account_id FROM users u WHERE u.email = ?")
        .bind(DEMO_EMAIL)
        .fetch_optional(&st.pool)
        .await?;
    if let Some(r) = existing {
        return Ok((r.get("id"), r.get("school_id"), false));
    }
    let account_id = id();
    let school_id = id();
    let ts = now();
    sqlx::query("INSERT INTO accounts (id, name, created_at) VALUES (?, ?, ?)")
        .bind(&account_id)
        .bind(DEMO_ACCOUNT_NAME)
        .bind(&ts)
        .execute(&st.pool)
        .await?;
    let summary = seed_demo(st, &account_id, &school_id).await?;
    let admin_id = sqlx::query("SELECT id FROM users WHERE email = ?")
        .bind(DEMO_EMAIL)
        .fetch_one(&st.pool)
        .await?
        .get::<String, _>("id");
    tracing::info!(
        "demo school seeded: {} placed / {} requested periods",
        summary.placed,
        summary.requested
    );
    Ok((admin_id, school_id, true))
}

#[allow(dead_code)]
pub fn demo_summary_json(s: &SeedSummary) -> serde_json::Value {
    json!({
        "school": s.school_name,
        "placed": s.placed,
        "requested": s.requested,
        "teachers": s.teachers,
        "classes": s.classes,
        "rooms": s.rooms,
        "lessons": s.lessons,
    })
}
