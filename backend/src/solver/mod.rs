//! Timetable constraint solver.
//!
//! Two-phase approach:
//!  1. Greedy placement (most-constrained-first) for an initial feasible solution.
//!  2. Simulated annealing over relocate/swap/insert moves to optimise soft goals.
//!
//! Hard constraints: no class/teacher/room clashes, teacher unavailability,
//! lesson max periods per day, teacher daily/weekly workload caps, fixed (locked) placements.
//! Soft goals: compact class day (minimise gaps), spread same subject across the week,
//! compact teacher day, balanced teacher load across days.

pub mod engine;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    pub same_subject_per_day: f64,
    pub class_gap: f64,
    pub teacher_gap: f64,
    pub teacher_balance: f64,
    pub week_balance: f64,
    pub unplaced: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            same_subject_per_day: 4.0,
            class_gap: 1.5,
            teacher_gap: 0.4,
            teacher_balance: 0.05,
            week_balance: 0.4,
            unplaced: 1000.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub day: i64,
    pub idx: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct FixedPlacement {
    pub week: usize,
    pub slot: usize,
    pub room: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct LessonIn {
    #[allow(dead_code)]
    pub id: String,
    pub class: usize,
    pub teacher: usize,
    pub subject: usize,
    pub periods: u32,
    pub max_per_day: u32,
    pub room: Option<usize>,
    pub required_room_type: Option<String>,
    pub fixed: Vec<FixedPlacement>,
}

#[derive(Debug, Clone)]
pub struct TeacherIn {
    #[allow(dead_code)]
    pub id: String,
    pub max_weekly: Option<u32>,
    pub max_daily: Option<u32>,
    /// (day, idx) pairs the teacher can never be scheduled at.
    pub unavailable: Vec<(i64, i64)>,
}

#[derive(Debug, Clone)]
pub struct ClassIn {
    #[allow(dead_code)]
    pub id: String,
    pub size: i64,
}

#[derive(Debug, Clone)]
pub struct RoomIn {
    #[allow(dead_code)]
    pub id: String,
    pub capacity: i64,
    pub room_type: String,
}

#[derive(Debug, Clone)]
pub struct Input {
    /// Schedulable slots (breaks excluded), sorted by (day, idx).
    pub slots: Vec<Slot>,
    pub weeks: usize,
    pub lessons: Vec<LessonIn>,
    pub teachers: Vec<TeacherIn>,
    pub classes: Vec<ClassIn>,
    pub rooms: Vec<RoomIn>,
    pub weights: Weights,
    pub seed: u64,
    pub time_limit_ms: u64,
    /// When set, runs a fixed number of iterations instead of a time budget (deterministic).
    pub iterations: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Placed {
    pub lesson: usize,
    pub week: usize,
    pub slot: usize,
    pub room: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ReasonCounts {
    pub teacher_unavailable: u32,
    pub class_busy: u32,
    pub teacher_busy: u32,
    pub room_busy: u32,
    pub limits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnplacedInfo {
    pub lesson: usize,
    pub count: u32,
    pub reasons: ReasonCounts,
    pub dominant: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoftCosts {
    pub class_gaps: f64,
    pub teacher_gaps: f64,
    pub same_subject_per_day: f64,
    pub teacher_balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub placed: Vec<Placed>,
    pub unplaced: Vec<UnplacedInfo>,
    pub cost: f64,
    pub soft: SoftCosts,
    pub iterations: u64,
    pub elapsed_ms: u64,
}

impl Output {
    pub fn total_unplaced(&self) -> u32 {
        self.unplaced.iter().map(|u| u.count).sum()
    }
}

pub fn run(input: &Input) -> Output {
    engine::run(input)
}
