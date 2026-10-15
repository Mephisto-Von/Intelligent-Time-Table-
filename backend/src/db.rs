use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS schools (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  cycle_weeks INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  school_id TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL,
  teacher_id TEXT,
  class_id TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS time_slots (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  day INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  start_time TEXT NOT NULL,
  end_time TEXT NOT NULL,
  label TEXT NOT NULL DEFAULT '',
  is_break INTEGER NOT NULL DEFAULT 0,
  UNIQUE (school_id, day, idx)
);
CREATE TABLE IF NOT EXISTS subjects (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  name TEXT NOT NULL,
  code TEXT NOT NULL,
  color TEXT NOT NULL DEFAULT '#6366f1',
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS teachers (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  name TEXT NOT NULL,
  initials TEXT NOT NULL,
  email TEXT,
  max_weekly INTEGER NOT NULL DEFAULT 30,
  max_daily INTEGER NOT NULL DEFAULT 6,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS teacher_subjects (
  teacher_id TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  school_id TEXT NOT NULL,
  PRIMARY KEY (teacher_id, subject_id)
);
CREATE TABLE IF NOT EXISTS classes (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  name TEXT NOT NULL,
  grade TEXT DEFAULT '',
  size INTEGER NOT NULL DEFAULT 30,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS rooms (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  name TEXT NOT NULL,
  capacity INTEGER NOT NULL DEFAULT 30,
  room_type TEXT NOT NULL DEFAULT 'general',
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS lessons (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  class_id TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  teacher_id TEXT NOT NULL,
  room_id TEXT,
  required_room_type TEXT,
  periods_per_week INTEGER NOT NULL,
  max_per_day INTEGER NOT NULL DEFAULT 2,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS teacher_unavailability (
  id TEXT PRIMARY KEY,
  teacher_id TEXT NOT NULL,
  school_id TEXT NOT NULL,
  day INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  UNIQUE (teacher_id, day, idx)
);
CREATE TABLE IF NOT EXISTS timetable_entries (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  lesson_id TEXT NOT NULL,
  week INTEGER NOT NULL DEFAULT 0,
  day INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  room_id TEXT,
  locked INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entries_school ON timetable_entries (school_id, week, day, idx);
CREATE INDEX IF NOT EXISTS idx_entries_lesson ON timetable_entries (lesson_id);
CREATE TABLE IF NOT EXISTS leave_requests (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  teacher_id TEXT NOT NULL,
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  part TEXT NOT NULL DEFAULT 'full',
  reason TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending',
  decided_by TEXT,
  decided_at TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS substitutions (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  date TEXT NOT NULL,
  day INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  lesson_id TEXT NOT NULL,
  original_teacher_id TEXT NOT NULL,
  substitute_teacher_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'assigned',
  note TEXT NOT NULL DEFAULT '',
  score REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  UNIQUE (school_id, date, lesson_id)
);
CREATE TABLE IF NOT EXISTS calendar_events (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL,
  title TEXT NOT NULL,
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'event',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);
"#;

pub async fn connect(path: &str) -> anyhow::Result<SqlitePool> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    let url = format!("sqlite://{path}");
    let opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    init(&pool).await?;
    Ok(pool)
}

pub async fn init(pool: &SqlitePool) -> anyhow::Result<()> {
    for stmt in SCHEMA.split(';') {
        let s = stmt.trim();
        if s.is_empty() {
            continue;
        }
        sqlx::query(sqlx::AssertSqlSafe(s)).execute(pool).await?;
    }
    Ok(())
}

/// Wipes every piece of school-scoped data (used by demo reset).
pub async fn wipe_school_data(pool: &SqlitePool, school_id: &str) -> anyhow::Result<()> {
    let tables = [
        "timetable_entries",
        "substitutions",
        "leave_requests",
        "calendar_events",
        "teacher_unavailability",
        "teacher_subjects",
        "lessons",
        "teachers",
        "classes",
        "rooms",
        "subjects",
        "time_slots",
    ];
    for t in tables {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {t} WHERE school_id = ?")))
            .bind(school_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}
