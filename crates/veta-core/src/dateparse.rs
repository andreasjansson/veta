//! Human-readable date parsing for Veta.

use chrono::NaiveDateTime;
use parse_datetime::parse_datetime;

use crate::Error;

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
/// Returns an error if the string cannot be parsed.
pub fn parse_human_date(input: &str) -> Result<String, Error> {
    let input = input.trim();

    // Try SQLite datetime format first (YYYY-MM-DD HH:MM:SS) - pass through
    if NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S").is_ok() {
        return Ok(input.to_string());
    }

    // Try date-only format (YYYY-MM-DD) - append midnight
    if chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d").is_ok() {
        return Ok(format!("{} 00:00:00", input));
    }

    // Use parse_datetime for everything else
    match parse_datetime(input) {
        Ok(zoned) => {
            // Get the datetime and format as SQLite datetime
            let dt = zoned.datetime();
            Ok(format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            ))
        }
        Err(_) => Err(Error::Validation(format!(
            "Could not parse date: '{}'. Try formats like '2 days ago', 'yesterday', or '2024-01-28'.",
            input
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_format_passthrough() {
        let result = parse_human_date("2026-01-28 12:30:45").unwrap();
        assert_eq!(result, "2026-01-28 12:30:45");
    }

    #[test]
    fn test_date_only() {
        let result = parse_human_date("2026-01-28").unwrap();
        assert_eq!(result, "2026-01-28 00:00:00");
    }

    #[test]
    fn test_relative_days_ago() {
        // Just verify it parses without error
        let result = parse_human_date("2 days ago");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yesterday() {
        let result = parse_human_date("yesterday");
        assert!(result.is_ok());
    }

    #[test]
    fn test_today() {
        let result = parse_human_date("today");
        assert!(result.is_ok());
    }

    #[test]
    fn test_now() {
        let result = parse_human_date("now");
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid() {
        let result = parse_human_date("not a date");
        assert!(result.is_err());
    }
}
