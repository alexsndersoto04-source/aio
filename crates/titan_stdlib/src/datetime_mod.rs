//! Real date/time handling (`std::datetime::*`) backed by `chrono`.
//!
//! All timestamps are `i64` seconds since the Unix epoch (positive = after
//! 1970-01-01 UTC). Timezone names use IANA identifiers when possible
//! ("UTC", "America/Caracas") but only fixed offsets are implemented today
//! to keep the dependency footprint small; `chrono-tz` can be added later
//! behind another feature.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Timelike, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DateTimeError {
    #[error("invalid timestamp {0}")]
    Timestamp(i64),
    #[error("invalid format string '{format}': {source}")]
    Format { format: String, #[source] source: chrono::ParseError },
    #[error("could not parse '{text}' with format '{format}': {source}")]
    Parse { text: String, format: String, #[source] source: chrono::ParseError },
    #[error("unsupported timezone '{0}' (only 'UTC' and fixed offsets like '-04:00' work today)")]
    Timezone(String),
}

fn from_ts(ts: i64) -> Result<DateTime<Utc>, DateTimeError> {
    DateTime::<Utc>::from_timestamp(ts, 0).ok_or(DateTimeError::Timestamp(ts))
}

/// Current UTC time as seconds since Unix epoch.
pub fn now() -> i64 { Utc::now().timestamp() }

/// Current UTC time as an ISO 8601 / RFC 3339 string.
pub fn now_iso() -> String { Utc::now().to_rfc3339() }

/// Format a Unix timestamp using [`chrono`] format directives (e.g. `%Y-%m-%d %H:%M:%S`).
pub fn format(ts: i64, fmt: &str) -> Result<String, DateTimeError> {
    Ok(from_ts(ts)?.format(fmt).to_string())
}

/// Format a Unix timestamp as RFC 3339 (`2026-07-25T12:00:00+00:00`).
pub fn to_rfc3339(ts: i64) -> Result<String, DateTimeError> {
    Ok(from_ts(ts)?.to_rfc3339())
}

/// Format a Unix timestamp as RFC 2822 (email-style: `Sat, 25 Jul 2026 12:00:00 +0000`).
pub fn to_rfc2822(ts: i64) -> Result<String, DateTimeError> {
    Ok(from_ts(ts)?.to_rfc2822())
}

/// Parse an ISO 8601 / RFC 3339 timestamp into Unix seconds.
pub fn parse_rfc3339(text: &str) -> Result<i64, DateTimeError> {
    DateTime::parse_from_rfc3339(text)
        .map(|dt| dt.timestamp())
        .map_err(|source| DateTimeError::Parse { text: text.into(), format: "RFC 3339".into(), source })
}

/// Parse a datetime given a chrono format string (no timezone → assumed UTC).
pub fn parse(text: &str, fmt: &str) -> Result<i64, DateTimeError> {
    NaiveDateTime::parse_from_str(text, fmt)
        .map(|ndt| ndt.and_utc().timestamp())
        .map_err(|source| DateTimeError::Parse { text: text.into(), format: fmt.into(), source })
}

/// Timestamp for a given UTC date/time (returns 0 on invalid components).
pub fn utc_ymd_hms(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second).single().map(|dt| dt.timestamp()).unwrap_or(0)
}

/// Add seconds to a timestamp (negative subtracts).
pub fn add_seconds(ts: i64, seconds: i64) -> i64 { ts.saturating_add(seconds) }

/// Add days to a timestamp.
pub fn add_days(ts: i64, days: i64) -> i64 { ts.saturating_add(days.saturating_mul(86_400)) }

/// Difference in seconds `later - earlier`.
pub fn diff_seconds(later: i64, earlier: i64) -> i64 { later.saturating_sub(earlier) }

// -- Field accessors (UTC) --------------------------------------------
pub fn year(ts: i64) -> Result<i32, DateTimeError> { Ok(from_ts(ts)?.year()) }
pub fn month(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.month()) }
pub fn day(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.day()) }
pub fn hour(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.hour()) }
pub fn minute(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.minute()) }
pub fn second(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.second()) }
/// 0=Monday .. 6=Sunday
pub fn weekday(ts: i64) -> Result<u32, DateTimeError> { Ok(from_ts(ts)?.weekday().num_days_from_monday()) }

/// Format a timestamp in a fixed offset (e.g. offset_minutes = -240 for -04:00 Caracas).
pub fn format_offset(ts: i64, fmt: &str, offset_minutes: i32) -> Result<String, DateTimeError> {
    let offset = FixedOffset::east_opt(offset_minutes.saturating_mul(60))
        .ok_or_else(|| DateTimeError::Timezone(format!("{offset_minutes} minutes")))?;
    Ok(from_ts(ts)?.with_timezone(&offset).format(fmt).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_monotonic_and_recent() {
        let a = now();
        let b = now();
        assert!(b >= a);
        // Sanity: sometime after 2020-01-01.
        assert!(a > 1_577_836_800);
    }

    #[test]
    fn format_and_parse_round_trip() {
        let ts = 1_720_000_000; // 2024-07-03 09:46:40 UTC
        let formatted = format(ts, "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(formatted, "2024-07-03 09:46:40");
        let parsed = parse(&formatted, "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(parsed, ts);
    }

    #[test]
    fn rfc_helpers() {
        let ts = utc_ymd_hms(2026, 7, 25, 12, 0, 0);
        assert_eq!(to_rfc3339(ts).unwrap(), "2026-07-25T12:00:00+00:00");
        assert_eq!(parse_rfc3339("2026-07-25T12:00:00Z").unwrap(), ts);
        assert!(to_rfc2822(ts).unwrap().contains("25 Jul 2026"));
    }

    #[test]
    fn field_accessors() {
        let ts = utc_ymd_hms(2026, 7, 25, 14, 30, 45);
        assert_eq!(year(ts).unwrap(), 2026);
        assert_eq!(month(ts).unwrap(), 7);
        assert_eq!(day(ts).unwrap(), 25);
        assert_eq!(hour(ts).unwrap(), 14);
        assert_eq!(minute(ts).unwrap(), 30);
        assert_eq!(second(ts).unwrap(), 45);
        assert_eq!(weekday(ts).unwrap(), 5); // 2026-07-25 is a Saturday (5 days from Monday)
    }

    #[test]
    fn arithmetic() {
        let ts = 1_000_000;
        assert_eq!(add_seconds(ts, 42), 1_000_042);
        assert_eq!(add_days(ts, 3), 1_000_000 + 3 * 86_400);
        assert_eq!(diff_seconds(1_000_100, 1_000_050), 50);
    }

    #[test]
    fn caracas_offset() {
        let ts = utc_ymd_hms(2026, 7, 25, 16, 0, 0);
        // Caracas is UTC-4 → 12:00 local.
        assert_eq!(format_offset(ts, "%H:%M", -240).unwrap(), "12:00");
    }

    #[test]
    fn errors_are_reported_not_panicked() {
        assert!(parse("garbage", "%Y-%m-%d").is_err());
        assert!(parse_rfc3339("also garbage").is_err());
    }
}
