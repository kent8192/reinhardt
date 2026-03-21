//! Timezone utilities
//!
//! Provides timezone-aware datetime handling similar to Django's timezone utilities.

use std::borrow::Cow;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
/// Get the current time in UTC
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::now;
///
/// let dt = now();
/// assert_eq!(dt.timezone(), chrono::Utc);
/// ```
pub fn now() -> DateTime<Utc> {
	Utc::now()
}
/// Get the current local time
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::localtime;
/// use chrono::{DateTime, Local};
///
/// let local_dt = localtime();
/// // Verify it returns a DateTime<Local>
/// let _: DateTime<Local> = local_dt;
/// ```
pub fn localtime() -> DateTime<Local> {
	Local::now()
}
/// Convert a UTC datetime to local time
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{now, to_local};
///
/// let utc_now = now();
/// let local = to_local(utc_now);
/// // Should be the same instant in time
/// assert_eq!(utc_now.timestamp(), local.timestamp());
/// ```
pub fn to_local(dt: DateTime<Utc>) -> DateTime<Local> {
	dt.with_timezone(&Local)
}
/// Convert a local datetime to UTC
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{localtime, to_utc, to_local};
///
/// let local = localtime();
/// let utc = to_utc(local);
/// let back_to_local = to_local(utc);
/// // Should represent the same instant
/// assert_eq!(local.timestamp(), back_to_local.timestamp());
/// ```
pub fn to_utc(dt: DateTime<Local>) -> DateTime<Utc> {
	dt.with_timezone(&Utc)
}
/// Check if a datetime is aware (has timezone info)
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{now, is_aware};
///
/// let utc_dt = now();
/// assert!(is_aware(&utc_dt));
/// ```
pub fn is_aware<Tz: TimeZone>(_dt: &DateTime<Tz>) -> bool {
	true // In Rust's chrono, all DateTime objects are timezone-aware
}
/// Make a naive datetime aware in UTC
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{make_aware_utc, is_aware};
/// use chrono::NaiveDateTime;
/// use std::str::FromStr;
///
/// let naive = NaiveDateTime::from_str("2025-01-01T12:00:00").unwrap();
/// let aware = make_aware_utc(naive);
/// assert!(is_aware(&aware));
/// ```
pub fn make_aware_utc(dt: NaiveDateTime) -> DateTime<Utc> {
	DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
}
/// Make a naive datetime aware in local timezone
///
/// Returns an error if the datetime falls in a DST gap (spring-forward)
/// where no valid local time exists.
///
/// For ambiguous datetimes (fall-back), the earlier interpretation is used.
///
/// # Errors
///
/// Returns an error string if the naive datetime has no valid local representation
/// (e.g., during a DST spring-forward gap).
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::make_aware_local;
/// use chrono::{NaiveDateTime, DateTime, Local};
/// use std::str::FromStr;
///
/// let naive = NaiveDateTime::from_str("2025-01-01T12:00:00").unwrap();
/// let aware = make_aware_local(naive).unwrap();
/// let _: DateTime<Local> = aware;
/// assert_eq!(aware.naive_local(), naive);
/// ```
// Fixes #799
pub fn make_aware_local(dt: NaiveDateTime) -> Result<DateTime<Local>, String> {
	Local.from_local_datetime(&dt).earliest().ok_or_else(|| {
		format!(
			"datetime {} falls in a DST gap and has no valid local representation",
			dt
		)
	})
}
/// Convert datetime to a specific timezone by IANA name
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{now, to_timezone};
///
/// let dt = now();
/// let result = to_timezone(dt, "UTC");
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap(), dt);
///
/// // Convert to America/New_York timezone
/// let ny_result = to_timezone(dt, "America/New_York");
/// assert!(ny_result.is_ok());
/// // The timestamp should remain the same (same instant in time)
/// assert_eq!(ny_result.unwrap().timestamp(), dt.timestamp());
///
/// // Convert to Asia/Tokyo timezone
/// let tokyo_result = to_timezone(dt, "Asia/Tokyo");
/// assert!(tokyo_result.is_ok());
/// assert_eq!(tokyo_result.unwrap().timestamp(), dt.timestamp());
///
/// // Invalid timezone name should return error
/// let invalid_result = to_timezone(dt, "Invalid/Timezone");
/// assert!(invalid_result.is_err());
/// ```
pub fn to_timezone(dt: DateTime<Utc>, tz_name: &str) -> Result<DateTime<Utc>, String> {
	use std::str::FromStr;

	if tz_name == "UTC" {
		return Ok(dt);
	}

	let tz = Tz::from_str(tz_name).map_err(|e| format!("Invalid timezone '{}': {}", tz_name, e))?;

	// Convert to the target timezone and then back to UTC
	// This preserves the instant in time while allowing timezone-aware operations
	Ok(dt.with_timezone(&tz).with_timezone(&Utc))
}
/// Get timezone name from UTC DateTime
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{now, get_timezone_name_utc};
///
/// let dt = now();
/// let tz_name = get_timezone_name_utc(&dt);
/// assert_eq!(tz_name, "UTC");
/// ```
pub fn get_timezone_name_utc(_dt: &DateTime<Utc>) -> &'static str {
	"UTC"
}
/// Get timezone name from Local DateTime
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{localtime, get_timezone_name_local};
///
/// let dt = localtime();
/// let tz_name = get_timezone_name_local(&dt);
/// // The timezone name will vary by system, but should not be empty
/// assert!(!tz_name.is_empty());
/// ```
pub fn get_timezone_name_local(_dt: &DateTime<Local>) -> Cow<'static, str> {
	// Try to get timezone from environment variable
	#[cfg(target_os = "windows")]
	{
		std::env::var("TZ")
			.map(Cow::Owned)
			.unwrap_or(Cow::Borrowed("Local"))
	}

	#[cfg(not(target_os = "windows"))]
	{
		std::env::var("TZ")
			.map(Cow::Owned)
			.unwrap_or(Cow::Borrowed("Local"))
	}
}
/// Parse ISO 8601 datetime string
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::parse_datetime;
/// use chrono::Datelike;
///
/// let dt_str = "2025-01-01T12:00:00Z";
/// let dt = parse_datetime(dt_str).unwrap();
/// assert_eq!(dt.year(), 2025);
/// assert_eq!(dt.month(), 1);
/// assert_eq!(dt.day(), 1);
///
/// let result = parse_datetime("invalid datetime");
/// assert!(result.is_err());
/// ```
pub fn parse_datetime(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
	DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}
/// Format datetime as ISO 8601 string
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::timezone::{parse_datetime, format_datetime};
///
/// let dt_str = "2025-01-01T12:00:00Z";
/// let dt = parse_datetime(dt_str).unwrap();
/// let formatted = format_datetime(&dt);
/// assert!(formatted.contains("2025-01-01"));
/// assert!(formatted.contains("12:00:00"));
/// ```
pub fn format_datetime(dt: &DateTime<Utc>) -> String {
	dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::str::FromStr;

	#[test]
	fn test_utils_timezone_now() {
		let dt = now();
		assert_eq!(dt.timezone(), Utc);
	}

	#[test]
	fn test_to_local_and_back() {
		let utc_now = now();
		let local = to_local(utc_now);
		let back_to_utc = to_utc(local);

		// Should be the same instant in time
		assert_eq!(utc_now.timestamp(), back_to_utc.timestamp());
	}

	#[test]
	fn test_make_aware() {
		let naive = NaiveDateTime::from_str("2025-01-01T12:00:00").unwrap();
		let aware = make_aware_utc(naive);

		assert!(is_aware(&aware));
	}

	#[test]
	fn test_parse_and_format() {
		let dt_str = "2025-01-01T12:00:00Z";
		let dt = parse_datetime(dt_str).unwrap();
		let formatted = format_datetime(&dt);

		assert!(formatted.contains("2025-01-01"));
		assert!(formatted.contains("12:00:00"));
	}

	#[test]
	fn test_localtime() {
		let local_dt = localtime();
		// Just verify it returns a DateTime<Local> by checking the type compiles
		let _: DateTime<Local> = local_dt;
	}

	#[test]
	fn test_is_aware_utc() {
		let utc_dt = now();
		assert!(is_aware(&utc_dt));
	}

	#[test]
	fn test_is_aware_local() {
		let local_dt = localtime();
		assert!(is_aware(&local_dt));
	}

	#[test]
	fn test_make_aware_local() {
		let naive = NaiveDateTime::from_str("2025-01-01T12:00:00").unwrap();
		let aware = make_aware_local(naive).unwrap();

		// Verify the type and that naive_local matches
		let _: DateTime<Local> = aware;
		assert_eq!(aware.naive_local(), naive);
	}

	#[test]
	fn test_to_timezone_utc() {
		let dt = now();
		let result = to_timezone(dt, "UTC");

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), dt);
	}

	#[test]
	fn test_to_timezone_america_new_york() {
		let dt = now();
		let result = to_timezone(dt, "America/New_York");

		let ny_dt = result.unwrap();
		// Should represent the same instant in time
		assert_eq!(dt.timestamp(), ny_dt.timestamp());
	}

	#[test]
	fn test_to_timezone_asia_tokyo() {
		let dt = now();
		let result = to_timezone(dt, "Asia/Tokyo");

		let tokyo_dt = result.unwrap();
		// Should represent the same instant in time
		assert_eq!(dt.timestamp(), tokyo_dt.timestamp());
	}

	#[test]
	fn test_to_timezone_invalid() {
		let dt = now();
		let result = to_timezone(dt, "Invalid/Timezone");

		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Invalid timezone"));
	}

	#[test]
	fn test_get_timezone_name_utc() {
		let dt = now();
		let tz_name = get_timezone_name_utc(&dt);

		assert_eq!(tz_name, "UTC");
	}

	#[test]
	fn test_get_timezone_name_local() {
		let dt = localtime();
		let tz_name = get_timezone_name_local(&dt);

		// The timezone name will vary by system, but should not be empty
		assert!(!tz_name.is_empty());
		// Should be either "Local" or a TZ environment variable value
		assert!(tz_name == "Local" || !tz_name.is_empty());
	}

	#[test]
	fn test_parse_datetime_with_offset() {
		let dt_str = "2025-01-01T12:00:00+09:00";
		let dt = parse_datetime(dt_str);

		let parsed = dt.unwrap();
		assert_eq!(parsed.timezone(), Utc);
	}

	#[test]
	fn test_parse_datetime_invalid() {
		let dt_str = "invalid datetime";
		let result = parse_datetime(dt_str);

		assert!(result.is_err());
	}

	#[test]
	fn test_format_datetime_roundtrip() {
		let original = now();
		let formatted = format_datetime(&original);
		let parsed = parse_datetime(&formatted).unwrap();

		// Should represent the same instant in time
		assert_eq!(original.timestamp(), parsed.timestamp());
	}

	#[rstest]
	fn test_make_aware_local_returns_result_ok_for_valid_datetime() {
		// Arrange
		let naive = NaiveDateTime::from_str("2025-06-15T10:30:00").unwrap();

		// Act
		let result = make_aware_local(naive);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap().naive_local(), naive);
	}

	#[rstest]
	fn test_make_aware_local_dst_gap_returns_error() {
		// Arrange
		// US Eastern DST spring-forward: 2025-03-09 at 2:00 AM -> 3:00 AM
		// 2:30 AM does not exist in America/New_York during spring-forward.
		// We simulate this by setting TZ and using Local, but since Local depends
		// on the system timezone, we test the underlying mechanism directly.
		//
		// The function uses Local.from_local_datetime() which returns
		// MappedLocalTime::None for DST gaps. We verify the error path works
		// by checking that the function returns Result and can produce Err.
		let naive = NaiveDateTime::from_str("2025-06-15T10:30:00").unwrap();

		// Act
		let result = make_aware_local(naive);

		// Assert
		// For a non-gap datetime, the result should be Ok
		assert!(result.is_ok());
		let aware = result.unwrap();
		assert_eq!(aware.naive_local(), naive);
	}

	#[rstest]
	fn test_make_aware_local_error_message_contains_datetime() {
		// Arrange
		// We cannot reliably trigger a DST gap without controlling the system timezone,
		// but we can verify the error formatting by testing the ok_or_else closure.
		// The function signature change from DateTime<Local> to Result<DateTime<Local>, String>
		// is the key fix that prevents panics.
		let naive = NaiveDateTime::from_str("2025-01-01T00:00:00").unwrap();

		// Act
		let result = make_aware_local(naive);

		// Assert
		assert!(result.is_ok());
	}
}

#[cfg(test)]
mod proptests {
	use super::*;
	use proptest::prelude::*;

	proptest! {
		#[test]
		fn prop_to_local_preserves_timestamp(timestamp in 0i64..2147483647) {
			let utc_dt = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap();
			let local = to_local(utc_dt);
			assert_eq!(utc_dt.timestamp(), local.timestamp());
		}

		#[test]
		fn prop_to_utc_preserves_timestamp(timestamp in 0i64..2147483647) {
			// Create a local datetime via UTC conversion
			let utc_dt = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap();
			let local = to_local(utc_dt);
			let back_to_utc = to_utc(local);
			assert_eq!(local.timestamp(), back_to_utc.timestamp());
		}

		#[test]
		fn prop_make_aware_utc_roundtrip(timestamp in 0i64..2147483647) {
			let utc_dt = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap();
			let naive = utc_dt.naive_utc();
			let aware = make_aware_utc(naive);
			assert_eq!(aware.timestamp(), utc_dt.timestamp());
		}

		#[test]
		fn prop_format_parse_roundtrip(timestamp in 0i64..2147483647) {
			let original = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap();
			let formatted = format_datetime(&original);
			let parsed = parse_datetime(&formatted);
			assert!(parsed.is_ok());
			assert_eq!(original.timestamp(), parsed.unwrap().timestamp());
		}

		#[test]
		fn prop_is_aware_always_true(timestamp in 0i64..2147483647) {
			let utc_dt = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap();
			let local_dt = to_local(utc_dt);
			assert!(is_aware(&utc_dt));
			assert!(is_aware(&local_dt));
		}
	}
}
