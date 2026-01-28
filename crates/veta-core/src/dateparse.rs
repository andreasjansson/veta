//! Human-readable date parsing using the `parse_datetime` crate.
//!
//! Parses strings like "2 days ago", "yesterday", "in 1 week" into
//! SQLite-compatible datetime strings.

use chrono::NaiveDateTime;

/// Parse a human-readable date string into a SQLite datetime string.
///
/// Supports a wide variety of formats including:
/// - SQLite datetime format: "2026-01-28 12:00:00" (passed through)
/// - ISO 8601: "2026-01-28T12:00:00Z"
/// - Relative past: "2 days ago", "1 week ago", "3 hours ago"
/// - Relative future: "in 2 days", "in 1 week"
/// - Named: "today", "yesterday", "tomorrow", "now"
/// - And many more formats supported by the `parse_datetime` crate
///
/// Returns None if the string cannot be parsed.
pub fn parse_human_date(input: &str) -> Option<String> {
    let input = input.trim();

    // Try SQLite datetime format first (YYYY-MM-DD HH:MM:SS) - pass through
    if NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S").is_ok() {
        return Some(input.to_string());
    }

    // Try date-only format (YYYY-MM-DD) - append midnight
    if chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d").is_ok() {
        return Some(format!("{} 00:00:00", input));
    }

    // Use parse_datetime for everything else
    match parse_datetime::parse_datetime(input) {
        Ok(zoned) => {
            // Get the datetime and format as SQLite datetime
            let dt = zoned.datetime();
            Some(format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            ))
        }
        Err(_) => None,
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
    fn test_invalid() {
        assert!(parse_human_date("not a date").is_none());
        assert!(parse_human_date("blah blah").is_none());
    }
}
