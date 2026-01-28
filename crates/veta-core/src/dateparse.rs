//! Human-readable date parsing.
//!
//! Parses strings like "2 days ago", "yesterday", "in 1 week" into
//! SQLite-compatible datetime strings.

use chrono::{Duration, NaiveDateTime, Utc};

/// Parse a human-readable date string into a SQLite datetime string.
///
/// Supports:
/// - SQLite datetime format: "2026-01-28 12:00:00" (passed through)
/// - Relative past: "2 days ago", "1 week ago", "3 hours ago"
/// - Relative future: "in 2 days", "in 1 week"
/// - Named: "today", "yesterday", "tomorrow", "now"
///
/// Returns None if the string cannot be parsed.
pub fn parse_human_date(input: &str) -> Option<String> {
    let input = input.trim().to_lowercase();

    // Try SQLite datetime format first (YYYY-MM-DD HH:MM:SS)
    if let Ok(_) = NaiveDateTime::parse_from_str(&input, "%Y-%m-%d %H:%M:%S") {
        return Some(input);
    }

    // Try date-only format (YYYY-MM-DD) - append midnight
    if let Ok(date) = chrono::NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        return Some(format!("{} 00:00:00", date));
    }

    let now = Utc::now().naive_utc();

    // Named dates
    match input.as_str() {
        "now" => return Some(format_datetime(now)),
        "today" => return Some(format_datetime(start_of_day(now))),
        "yesterday" => return Some(format_datetime(start_of_day(now) - Duration::days(1))),
        "tomorrow" => return Some(format_datetime(start_of_day(now) + Duration::days(1))),
        _ => {}
    }

    // "X unit(s) ago" pattern
    if let Some(duration) = parse_ago(&input) {
        return Some(format_datetime(now - duration));
    }

    // "in X unit(s)" pattern
    if let Some(duration) = parse_in_future(&input) {
        return Some(format_datetime(now + duration));
    }

    None
}

fn format_datetime(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn start_of_day(dt: NaiveDateTime) -> NaiveDateTime {
    dt.date().and_hms_opt(0, 0, 0).unwrap()
}

/// Parse "X unit(s) ago" pattern
fn parse_ago(input: &str) -> Option<Duration> {
    let input = input.trim();
    if !input.ends_with(" ago") {
        return None;
    }

    let without_ago = input.strip_suffix(" ago")?;
    parse_duration(without_ago)
}

/// Parse "in X unit(s)" pattern
fn parse_in_future(input: &str) -> Option<Duration> {
    let input = input.trim();
    if !input.starts_with("in ") {
        return None;
    }

    let without_in = input.strip_prefix("in ")?;
    parse_duration(without_in)
}

/// Parse a duration like "2 days", "1 week", "3 hours"
fn parse_duration(input: &str) -> Option<Duration> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.len() == 2 {
        // "2 days"
        let num: i64 = parts[0].parse().ok()?;
        return unit_to_duration(parts[1], num);
    }

    if parts.len() == 1 {
        // "1day" or just try to parse as a single word
        let s = parts[0];
        // Try to find where number ends and unit begins
        let num_end = s.chars().take_while(|c| c.is_ascii_digit()).count();
        if num_end > 0 && num_end < s.len() {
            let num: i64 = s[..num_end].parse().ok()?;
            let unit = &s[num_end..];
            return unit_to_duration(unit, num);
        }
    }

    None
}

fn unit_to_duration(unit: &str, num: i64) -> Option<Duration> {
    let unit = unit.trim_end_matches('s'); // Handle plural
    match unit {
        "second" | "sec" => Some(Duration::seconds(num)),
        "minute" | "min" => Some(Duration::minutes(num)),
        "hour" | "hr" | "h" => Some(Duration::hours(num)),
        "day" | "d" => Some(Duration::days(num)),
        "week" | "wk" | "w" => Some(Duration::weeks(num)),
        "month" | "mon" => Some(Duration::days(num * 30)), // Approximate
        "year" | "yr" | "y" => Some(Duration::days(num * 365)), // Approximate
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_format_passthrough() {
        assert_eq!(
            parse_human_date("2026-01-28 12:00:00"),
            Some("2026-01-28 12:00:00".to_string())
        );
    }

    #[test]
    fn test_date_only() {
        assert_eq!(
            parse_human_date("2026-01-28"),
            Some("2026-01-28 00:00:00".to_string())
        );
    }

    #[test]
    fn test_named_dates() {
        // These will vary based on current time, so just check they return Some
        assert!(parse_human_date("today").is_some());
        assert!(parse_human_date("yesterday").is_some());
        assert!(parse_human_date("tomorrow").is_some());
        assert!(parse_human_date("now").is_some());
    }

    #[test]
    fn test_ago_pattern() {
        // These will vary based on current time, so just check they return Some
        assert!(parse_human_date("2 days ago").is_some());
        assert!(parse_human_date("1 week ago").is_some());
        assert!(parse_human_date("3 hours ago").is_some());
        assert!(parse_human_date("30 minutes ago").is_some());
    }

    #[test]
    fn test_in_future_pattern() {
        assert!(parse_human_date("in 2 days").is_some());
        assert!(parse_human_date("in 1 week").is_some());
    }

    #[test]
    fn test_invalid() {
        assert!(parse_human_date("not a date").is_none());
        assert!(parse_human_date("blah blah").is_none());
    }
}
