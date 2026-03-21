//! Time-based access control permissions
//!
//! Provides permissions that restrict access based on time of day,
//! day of week, or specific date ranges.

use crate::{Permission, PermissionContext};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, NaiveTime, Utc, Weekday};

/// Time-based permission
///
/// Allows access only during specified time windows.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::TimeBasedPermission;
///
/// let permission = TimeBasedPermission::new()
///     .add_time_window("09:00", "17:00")  // Business hours
///     .add_weekday(chrono::Weekday::Mon)
///     .add_weekday(chrono::Weekday::Tue)
///     .add_weekday(chrono::Weekday::Wed)
///     .add_weekday(chrono::Weekday::Thu)
///     .add_weekday(chrono::Weekday::Fri);
/// ```
#[derive(Debug, Clone)]
pub struct TimeBasedPermission {
	/// Allowed time windows (start time, end time)
	pub time_windows: Vec<TimeWindow>,
	/// Allowed weekdays
	pub allowed_weekdays: Vec<Weekday>,
	/// Allowed date ranges
	pub date_ranges: Vec<DateRange>,
	/// Timezone for time comparisons
	pub timezone: String,
	/// Whether to allow on parse error
	pub allow_on_error: bool,
}

impl TimeBasedPermission {
	/// Create a new time-based permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new();
	/// ```
	pub fn new() -> Self {
		Self {
			time_windows: Vec::new(),
			allowed_weekdays: Vec::new(),
			date_ranges: Vec::new(),
			timezone: "UTC".to_string(),
			allow_on_error: false,
		}
	}

	/// Add a time window (in 24-hour format)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .add_time_window("09:00", "17:00");
	/// ```
	pub fn add_time_window(mut self, start: impl AsRef<str>, end: impl AsRef<str>) -> Self {
		if let (Ok(start_time), Ok(end_time)) = (
			NaiveTime::parse_from_str(start.as_ref(), "%H:%M"),
			NaiveTime::parse_from_str(end.as_ref(), "%H:%M"),
		) {
			self.time_windows.push(TimeWindow {
				start: start_time,
				end: end_time,
			});
		}
		self
	}

	/// Add an allowed weekday
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	/// use chrono::Weekday;
	///
	/// let permission = TimeBasedPermission::new()
	///     .add_weekday(Weekday::Mon)
	///     .add_weekday(Weekday::Tue);
	/// ```
	pub fn add_weekday(mut self, weekday: Weekday) -> Self {
		if !self.allowed_weekdays.contains(&weekday) {
			self.allowed_weekdays.push(weekday);
		}
		self
	}

	/// Add weekdays in bulk
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	/// use chrono::Weekday;
	///
	/// let permission = TimeBasedPermission::new()
	///     .add_weekdays(&[Weekday::Mon, Weekday::Tue, Weekday::Wed]);
	/// ```
	pub fn add_weekdays(mut self, weekdays: &[Weekday]) -> Self {
		for &weekday in weekdays {
			if !self.allowed_weekdays.contains(&weekday) {
				self.allowed_weekdays.push(weekday);
			}
		}
		self
	}

	/// Add business days (Monday through Friday)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .business_days();
	/// ```
	pub fn business_days(self) -> Self {
		self.add_weekdays(&[
			Weekday::Mon,
			Weekday::Tue,
			Weekday::Wed,
			Weekday::Thu,
			Weekday::Fri,
		])
	}

	/// Add weekend days (Saturday and Sunday)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .weekend_days();
	/// ```
	pub fn weekend_days(self) -> Self {
		self.add_weekdays(&[Weekday::Sat, Weekday::Sun])
	}

	/// Add a date range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .add_date_range("2024-01-01", "2024-12-31");
	/// ```
	pub fn add_date_range(mut self, start: impl AsRef<str>, end: impl AsRef<str>) -> Self {
		if let (Ok(start_date), Ok(end_date)) = (
			DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", start.as_ref())),
			DateTime::parse_from_rfc3339(&format!("{}T23:59:59Z", end.as_ref())),
		) {
			self.date_ranges.push(DateRange {
				start: start_date.with_timezone(&Utc),
				end: end_date.with_timezone(&Utc),
			});
		}
		self
	}

	/// Set the timezone
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .timezone("America/New_York");
	/// ```
	pub fn timezone(mut self, tz: impl Into<String>) -> Self {
		self.timezone = tz.into();
		self
	}

	/// Set whether to allow on parse error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	///
	/// let permission = TimeBasedPermission::new()
	///     .allow_on_error(true);
	/// ```
	pub fn allow_on_error(mut self, allow: bool) -> Self {
		self.allow_on_error = allow;
		self
	}

	/// Check if the current time is allowed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeBasedPermission;
	/// use chrono::Utc;
	///
	/// let permission = TimeBasedPermission::new()
	///     .add_time_window("09:00", "17:00");
	///
	/// let now = Utc::now();
	/// let is_allowed = permission.is_allowed_at(&now);
	/// ```
	pub fn is_allowed_at(&self, dt: &DateTime<Utc>) -> bool {
		// If no restrictions are set, allow access
		if self.time_windows.is_empty()
			&& self.allowed_weekdays.is_empty()
			&& self.date_ranges.is_empty()
		{
			return true;
		}

		// Check time windows
		if !self.time_windows.is_empty() {
			let time = dt.time();
			let time_allowed = self
				.time_windows
				.iter()
				.any(|window| window.contains(&time));
			if !time_allowed {
				return false;
			}
		}

		// Check weekdays
		if !self.allowed_weekdays.is_empty() {
			let weekday = dt.weekday();
			if !self.allowed_weekdays.contains(&weekday) {
				return false;
			}
		}

		// Check date ranges
		if !self.date_ranges.is_empty() {
			let date_allowed = self.date_ranges.iter().any(|range| range.contains(dt));
			if !date_allowed {
				return false;
			}
		}

		true
	}
}

impl Default for TimeBasedPermission {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for TimeBasedPermission {
	async fn has_permission(&self, _context: &PermissionContext<'_>) -> bool {
		let now = Utc::now();
		self.is_allowed_at(&now)
	}
}

/// Time window representation
///
/// # Examples
///
/// ```
/// use reinhardt_auth::TimeWindow;
/// use chrono::NaiveTime;
///
/// let window = TimeWindow::new(
///     NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
///     NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
/// );
/// ```
#[derive(Debug, Clone)]
pub struct TimeWindow {
	/// Start time
	pub start: NaiveTime,
	/// End time
	pub end: NaiveTime,
}

impl TimeWindow {
	/// Create a new time window
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeWindow;
	/// use chrono::NaiveTime;
	///
	/// let start = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
	/// let end = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
	/// let window = TimeWindow::new(start, end);
	/// ```
	pub fn new(start: NaiveTime, end: NaiveTime) -> Self {
		Self { start, end }
	}

	/// Check if a time is within this window
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TimeWindow;
	/// use chrono::NaiveTime;
	///
	/// let window = TimeWindow::new(
	///     NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
	///     NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
	/// );
	///
	/// let time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
	/// assert!(window.contains(&time));
	/// ```
	pub fn contains(&self, time: &NaiveTime) -> bool {
		if self.start <= self.end {
			// Normal case: 09:00 - 17:00
			time >= &self.start && time <= &self.end
		} else {
			// Overnight case: 22:00 - 06:00
			time >= &self.start || time <= &self.end
		}
	}
}

/// Date range representation
///
/// # Examples
///
/// ```
/// use reinhardt_auth::DateRange;
/// use chrono::{DateTime, Utc};
///
/// let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
///     .unwrap()
///     .with_timezone(&Utc);
/// let end = DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z")
///     .unwrap()
///     .with_timezone(&Utc);
/// let range = DateRange::new(start, end);
/// ```
#[derive(Debug, Clone)]
pub struct DateRange {
	/// Start date
	pub start: DateTime<Utc>,
	/// End date
	pub end: DateTime<Utc>,
}

impl DateRange {
	/// Create a new date range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::DateRange;
	/// use chrono::{DateTime, Utc};
	///
	/// let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
	///     .unwrap()
	///     .with_timezone(&Utc);
	/// let end = DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z")
	///     .unwrap()
	///     .with_timezone(&Utc);
	/// let range = DateRange::new(start, end);
	/// ```
	pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
		Self { start, end }
	}

	/// Check if a datetime is within this range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::DateRange;
	/// use chrono::{DateTime, Utc};
	///
	/// let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
	///     .unwrap()
	///     .with_timezone(&Utc);
	/// let end = DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z")
	///     .unwrap()
	///     .with_timezone(&Utc);
	/// let range = DateRange::new(start, end);
	///
	/// let date = DateTime::parse_from_rfc3339("2024-06-15T12:00:00Z")
	///     .unwrap()
	///     .with_timezone(&Utc);
	/// assert!(range.contains(&date));
	/// ```
	pub fn contains(&self, dt: &DateTime<Utc>) -> bool {
		dt >= &self.start && dt <= &self.end
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use chrono::Timelike;
	use hyper::Method;
	use reinhardt_http::Request;
	use rstest::rstest;

	#[test]
	fn test_time_window_creation() {
		let start = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
		let end = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
		let window = TimeWindow::new(start, end);

		assert_eq!(window.start.hour(), 9);
		assert_eq!(window.end.hour(), 17);
	}

	#[test]
	fn test_time_window_contains() {
		let window = TimeWindow::new(
			NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
			NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
		);

		let morning = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
		let early = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
		let late = NaiveTime::from_hms_opt(18, 0, 0).unwrap();

		assert!(window.contains(&morning));
		assert!(!window.contains(&early));
		assert!(!window.contains(&late));
	}

	#[test]
	fn test_time_window_overnight() {
		let window = TimeWindow::new(
			NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
			NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
		);

		let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
		let morning = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
		let evening = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
		let afternoon = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

		assert!(window.contains(&midnight));
		assert!(window.contains(&morning));
		assert!(window.contains(&evening));
		assert!(!window.contains(&afternoon));
	}

	#[test]
	fn test_date_range_creation() {
		let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
			.unwrap()
			.with_timezone(&Utc);
		let end = DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z")
			.unwrap()
			.with_timezone(&Utc);
		let range = DateRange::new(start, end);

		assert_eq!(range.start.year(), 2024);
		assert_eq!(range.end.year(), 2024);
	}

	#[test]
	fn test_date_range_contains() {
		let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
			.unwrap()
			.with_timezone(&Utc);
		let end = DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z")
			.unwrap()
			.with_timezone(&Utc);
		let range = DateRange::new(start, end);

		let in_range = DateTime::parse_from_rfc3339("2024-06-15T12:00:00Z")
			.unwrap()
			.with_timezone(&Utc);
		let before = DateTime::parse_from_rfc3339("2023-12-31T23:59:59Z")
			.unwrap()
			.with_timezone(&Utc);
		let after = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
			.unwrap()
			.with_timezone(&Utc);

		assert!(range.contains(&in_range));
		assert!(!range.contains(&before));
		assert!(!range.contains(&after));
	}

	#[test]
	fn test_permission_creation() {
		let permission = TimeBasedPermission::new();
		assert_eq!(permission.time_windows.len(), 0);
		assert_eq!(permission.allowed_weekdays.len(), 0);
		assert_eq!(permission.date_ranges.len(), 0);
		assert_eq!(permission.timezone, "UTC");
		assert!(!permission.allow_on_error);
	}

	#[test]
	fn test_permission_add_time_window() {
		let permission = TimeBasedPermission::new().add_time_window("09:00", "17:00");

		assert_eq!(permission.time_windows.len(), 1);
	}

	#[test]
	fn test_permission_add_weekday() {
		let permission = TimeBasedPermission::new()
			.add_weekday(Weekday::Mon)
			.add_weekday(Weekday::Tue);

		assert_eq!(permission.allowed_weekdays.len(), 2);
		assert!(permission.allowed_weekdays.contains(&Weekday::Mon));
		assert!(permission.allowed_weekdays.contains(&Weekday::Tue));
	}

	#[test]
	fn test_permission_business_days() {
		let permission = TimeBasedPermission::new().business_days();

		assert_eq!(permission.allowed_weekdays.len(), 5);
		assert!(permission.allowed_weekdays.contains(&Weekday::Mon));
		assert!(permission.allowed_weekdays.contains(&Weekday::Fri));
		assert!(!permission.allowed_weekdays.contains(&Weekday::Sat));
	}

	#[test]
	fn test_permission_weekend_days() {
		let permission = TimeBasedPermission::new().weekend_days();

		assert_eq!(permission.allowed_weekdays.len(), 2);
		assert!(permission.allowed_weekdays.contains(&Weekday::Sat));
		assert!(permission.allowed_weekdays.contains(&Weekday::Sun));
	}

	#[test]
	fn test_permission_add_date_range() {
		let permission = TimeBasedPermission::new().add_date_range("2024-01-01", "2024-12-31");

		assert_eq!(permission.date_ranges.len(), 1);
	}

	#[test]
	fn test_permission_no_restrictions() {
		let permission = TimeBasedPermission::new();
		let now = Utc::now();
		assert!(permission.is_allowed_at(&now));
	}

	#[test]
	fn test_permission_time_window_restriction() {
		let permission = TimeBasedPermission::new().add_time_window("09:00", "17:00");

		let morning = Utc::now()
			.date_naive()
			.and_hms_opt(12, 0, 0)
			.unwrap()
			.and_utc();
		let night = Utc::now()
			.date_naive()
			.and_hms_opt(22, 0, 0)
			.unwrap()
			.and_utc();

		assert!(permission.is_allowed_at(&morning));
		assert!(!permission.is_allowed_at(&night));
	}

	#[tokio::test]
	async fn test_permission_has_permission() {
		let permission = TimeBasedPermission::new().add_time_window("00:00", "23:59");

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(permission.has_permission(&context).await);
	}

	#[test]
	fn test_permission_weekday_restriction() {
		let permission = TimeBasedPermission::new().add_weekday(Weekday::Mon);

		// Create a Monday
		let monday = DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z") // 2024-01-01 was Monday
			.unwrap()
			.with_timezone(&Utc);

		// Create a Tuesday
		let tuesday = DateTime::parse_from_rfc3339("2024-01-02T12:00:00Z")
			.unwrap()
			.with_timezone(&Utc);

		assert!(permission.is_allowed_at(&monday));
		assert!(!permission.is_allowed_at(&tuesday));
	}

	#[rstest]
	fn test_time_window_exact_boundary_start() {
		// Arrange
		let permission = TimeBasedPermission::new().add_time_window("09:00", "17:00");
		let exact_start = Utc::now()
			.date_naive()
			.and_hms_opt(9, 0, 0)
			.unwrap()
			.and_utc();

		// Act
		let result = permission.is_allowed_at(&exact_start);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_time_window_exact_boundary_end() {
		// Arrange
		let permission = TimeBasedPermission::new().add_time_window("09:00", "17:00");
		let exact_end = Utc::now()
			.date_naive()
			.and_hms_opt(17, 0, 0)
			.unwrap()
			.and_utc();
		let one_second_after = Utc::now()
			.date_naive()
			.and_hms_opt(17, 0, 1)
			.unwrap()
			.and_utc();

		// Act
		let result_at_end = permission.is_allowed_at(&exact_end);
		let result_after_end = permission.is_allowed_at(&one_second_after);

		// Assert - end time is inclusive in TimeWindow::contains
		assert!(result_at_end);
		assert!(!result_after_end);
	}

	#[rstest]
	fn test_date_range_single_day() {
		// Arrange - single day range for 2024-06-15
		let permission = TimeBasedPermission::new().add_date_range("2024-06-15", "2024-06-15");

		let within_day = DateTime::parse_from_rfc3339("2024-06-15T12:00:00Z")
			.unwrap()
			.with_timezone(&Utc);
		let different_day = DateTime::parse_from_rfc3339("2024-06-16T12:00:00Z")
			.unwrap()
			.with_timezone(&Utc);

		// Act
		let result_within = permission.is_allowed_at(&within_day);
		let result_different = permission.is_allowed_at(&different_day);

		// Assert
		assert!(result_within);
		assert!(!result_different);
	}
}
