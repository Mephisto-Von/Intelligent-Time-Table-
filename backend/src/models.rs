use serde::Deserialize;

use crate::solver::Weights;

#[derive(Debug, Deserialize)]
pub struct SignupInput {
    pub account_name: String,
    pub school_name: String,
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SchoolInput {
    pub name: String,
    pub cycle_weeks: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UserInput {
    pub name: String,
    pub email: String,
    pub password: Option<String>,
    pub role: String,
    pub teacher_id: Option<String>,
    pub class_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectInput {
    pub name: String,
    pub code: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SlotRef {
    pub day: i64,
    pub idx: i64,
}

#[derive(Debug, Deserialize)]
pub struct TeacherInput {
    pub name: String,
    pub initials: Option<String>,
    pub email: Option<String>,
    pub max_weekly: Option<i64>,
    pub max_daily: Option<i64>,
    #[serde(default)]
    pub subject_ids: Vec<String>,
    #[serde(default)]
    pub unavailable: Vec<SlotRef>,
}

#[derive(Debug, Deserialize)]
pub struct ClassInput {
    pub name: String,
    pub grade: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RoomInput {
    pub name: String,
    pub capacity: Option<i64>,
    pub room_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BellSlotInput {
    pub day: i64,
    pub idx: i64,
    pub start_time: String,
    pub end_time: String,
    pub label: Option<String>,
    pub is_break: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BellScheduleInput {
    #[serde(default)]
    pub slots: Vec<BellSlotInput>,
}

#[derive(Debug, Deserialize)]
pub struct LessonInput {
    pub class_id: String,
    pub subject_id: String,
    pub teacher_id: String,
    pub room_id: Option<String>,
    pub required_room_type: Option<String>,
    pub periods_per_week: i64,
    pub max_per_day: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct EntryInput {
    pub lesson_id: String,
    pub week: Option<i64>,
    pub day: i64,
    pub idx: i64,
    pub room_id: Option<String>,
    pub locked: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EntryPatch {
    pub week: Option<i64>,
    pub day: Option<i64>,
    pub idx: Option<i64>,
    pub room_id: Option<Option<String>>,
    pub locked: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub time_limit_ms: Option<u64>,
    pub seed: Option<u64>,
    pub weights: Option<Weights>,
}

#[derive(Debug, Deserialize)]
pub struct LeaveInput {
    pub teacher_id: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub part: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DecideInput {
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
pub struct CoverAssignInput {
    pub date: String,
    pub lesson_id: String,
    pub day: i64,
    pub idx: i64,
    pub substitute_teacher_id: String,
}

#[derive(Debug, Deserialize)]
pub struct EventInput {
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub kind: Option<String>,
    pub notes: Option<String>,
}

pub fn day_name(day: i64) -> &'static str {
    match day {
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        7 => "Sunday",
        _ => "Day",
    }
}
