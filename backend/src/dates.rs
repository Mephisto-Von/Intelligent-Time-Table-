use chrono::{Datelike, NaiveDate};

use crate::error::{ApiError, ApiResult};

pub fn parse_date(s: &str) -> ApiResult<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest(format!("invalid date '{s}', expected YYYY-MM-DD")))
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// School day numbering: 1 = Monday ... 7 = Sunday.
pub fn day_of(date: &NaiveDate) -> i64 {
    date.weekday().num_days_from_monday() as i64 + 1
}

/// Which week of the cycle a calendar date falls in (0-based).
pub fn week_index(date: &NaiveDate, cycle_weeks: i64) -> i64 {
    if cycle_weeks > 1 {
        date.iso_week().week() as i64 % cycle_weeks
    } else {
        0
    }
}

pub fn date_range(start: &NaiveDate, end: &NaiveDate, limit_days: i64) -> ApiResult<Vec<NaiveDate>> {
    if end < start {
        return Err(ApiError::BadRequest("end date must be on or after start date".into()));
    }
    let mut out = Vec::new();
    let mut d = *start;
    while d <= *end {
        out.push(d);
        d += chrono::Duration::days(1);
        if out.len() as i64 > limit_days {
            return Err(ApiError::BadRequest(format!("date range too large (max {limit_days} days)")));
        }
    }
    Ok(out)
}

/// "am" covers the first half of the day's periods, "pm" the second half, "full" everything.
pub fn part_covers(part: &str, idx: i64, am_max: i64) -> bool {
    match part {
        "am" => idx <= am_max,
        "pm" => idx > am_max,
        _ => true,
    }
}
