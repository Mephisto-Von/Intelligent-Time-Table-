//! iCalendar (RFC 5545) export for timetables.
//!
//! Each timetable entry becomes a weekly-recurring VEVENT anchored on the
//! current week. Two-week cycles are expressed with `INTERVAL=2`.

use chrono::{NaiveDate, Utc};

pub struct Event {
    pub uid: String,
    pub date: NaiveDate,
    /// "HH:MM"
    pub start: String,
    /// "HH:MM"
    pub end: String,
    pub summary: String,
    pub location: Option<String>,
    pub description: Option<String>,
}

fn hhmmss(t: &str) -> String {
    let mut parts = t.trim().split(':');
    let h = parts.next().unwrap_or("00");
    let m = parts.next().unwrap_or("00");
    format!("{:0>2}{:0>2}00", h, m)
}

/// Local (floating) date-time: YYYYMMDDTHHMMSS.
fn stamp(date: NaiveDate, time: &str) -> String {
    format!("{}T{}", date.format("%Y%m%d"), hhmmss(time))
}

/// UTC date-time as required for DTSTAMP: YYYYMMDDTHHMMSSZ.
fn utc_stamp() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' | '\r' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out
}

/// Folds a content line to 75 octets per RFC 5545, never splitting a character.
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut octets = 0usize;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if octets + len > 75 {
            out.push_str("\r\n ");
            octets = 1; // the leading space counts toward the folded line
        }
        out.push(ch);
        octets += len;
    }
    out
}

pub fn render(cal_name: &str, interval: u32, count: u32, events: &[Event]) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("BEGIN:VCALENDAR".into());
    lines.push("VERSION:2.0".into());
    lines.push("PRODID:-//TimetableMaster//Timetable//EN".into());
    lines.push("CALSCALE:GREGORIAN".into());
    lines.push("METHOD:PUBLISH".into());
    lines.push(format!("X-WR-CALNAME:{}", esc(cal_name)));

    let now_stamp = utc_stamp();
    for e in events {
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}", e.uid));
        lines.push(format!("DTSTAMP:{now_stamp}"));
        lines.push(format!("DTSTART:{}", stamp(e.date, &e.start)));
        lines.push(format!("DTEND:{}", stamp(e.date, &e.end)));
        lines.push(format!(
            "RRULE:FREQ=WEEKLY;INTERVAL={};COUNT={}",
            interval.max(1),
            count.max(1)
        ));
        lines.push(format!("SUMMARY:{}", esc(&e.summary)));
        if let Some(loc) = &e.location {
            if !loc.is_empty() {
                lines.push(format!("LOCATION:{}", esc(loc)));
            }
        }
        if let Some(desc) = &e.description {
            if !desc.is_empty() {
                lines.push(format!("DESCRIPTION:{}", esc(desc)));
            }
        }
        lines.push("END:VEVENT".into());
    }

    lines.push("END:VCALENDAR".into());
    let mut out = String::new();
    for l in lines {
        out.push_str(&fold(&l));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn renders_a_valid_weekly_calendar() {
        let events = vec![
            Event {
                uid: "abc-1@timetablemaster".into(),
                date: NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
                start: "08:30".into(),
                end: "09:15".into(),
                summary: "Mathematics · Grade 7A".into(),
                location: Some("Room 101".into()),
                description: Some("Teacher: Sarah Mitchell".into()),
            },
            Event {
                uid: "def-0@timetablemaster".into(),
                date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
                start: "10:00".into(),
                end: "10:45".into(),
                summary: "English, poetry; week 2".into(),
                location: None,
                description: None,
            },
        ];
        let out = render("class timetable: Grade 7A", 2, 12, &events);

        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(out.ends_with("END:VCALENDAR\r\n"));
        assert!(
            out.contains("DTSTART:20261019T083000\r\n"),
            "DTSTART missing; output was:\n{out}"
        );
        assert!(out.contains("DTEND:20261019T091500\r\n"));
        assert!(out.contains("RRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=12\r\n"));
        assert!(out.contains("SUMMARY:Mathematics · Grade 7A"));
        assert!(out.contains("LOCATION:Room 101"));
        // Commas and semicolons are escaped per RFC 5545.
        assert!(out.contains("SUMMARY:English\\, poetry\\; week 2"));
        // Every physical line must be <= 75 octets (plus CRLF).
        for line in out.split("\r\n") {
            assert!(line.len() <= 75, "line too long: {line:?}");
        }
    }
}
