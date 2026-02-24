//! CREATE EVENT statement builder (MySQL-specific)
//!
//! This module provides a builder for MySQL CREATE EVENT statements,
//! which are used to create scheduled tasks that execute SQL statements
//! at specified times or intervals.
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // CREATE EVENT one_time_task
//! // ON SCHEDULE AT '2026-12-31 23:59:59'
//! // DO INSERT INTO logs VALUES (NOW())
//! let stmt = Query::create_event()
//!     .name("one_time_task")
//!     .on_schedule_at("2026-12-31 23:59:59")
//!     .do_body("INSERT INTO logs VALUES (NOW())");
//!
//! // CREATE EVENT IF NOT EXISTS daily_cleanup
//! // ON SCHEDULE EVERY 1 DAY
//! // STARTS '2026-01-01 00:00:00'
//! // ON COMPLETION PRESERVE
//! // ENABLE
//! // COMMENT 'Daily cleanup task'
//! // DO DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 7 DAY
//! let stmt = Query::create_event()
//!     .if_not_exists()
//!     .name("daily_cleanup")
//!     .on_schedule_every("1 DAY")
//!     .starts("2026-01-01 00:00:00")
//!     .on_completion_preserve()
//!     .enable()
//!     .comment("Daily cleanup task")
//!     .do_body("DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 7 DAY");
//! ```

use crate::types::event::{EventCompletion, EventSchedule};
use crate::types::{DynIden, IntoIden};

/// CREATE EVENT statement builder (MySQL-specific)
///
/// This struct provides a builder for constructing CREATE EVENT statements
/// for MySQL scheduled tasks.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Simple one-time event
/// let stmt = Query::create_event()
///     .name("my_event")
///     .on_schedule_at("2026-12-31 23:59:59")
///     .do_body("INSERT INTO logs VALUES (NOW())");
///
/// // Recurring event with options
/// let stmt = Query::create_event()
///     .if_not_exists()
///     .name("recurring_event")
///     .on_schedule_every("1 HOUR")
///     .starts("2026-01-01 00:00:00")
///     .ends("2026-12-31 23:59:59")
///     .on_completion_preserve()
///     .enable()
///     .comment("Hourly task")
///     .do_body("CALL my_procedure()");
/// ```
#[derive(Debug, Clone)]
pub struct CreateEventStatement {
	pub(crate) if_not_exists: bool,
	pub(crate) name: Option<DynIden>,
	pub(crate) schedule: Option<EventSchedule>,
	pub(crate) completion: Option<EventCompletion>,
	pub(crate) enable: bool,
	pub(crate) comment: Option<String>,
	pub(crate) body: Option<String>,
}

impl CreateEventStatement {
	/// Create a new CREATE EVENT statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::event::CreateEventStatement;
	///
	/// let stmt = CreateEventStatement::new();
	/// ```
	pub fn new() -> Self {
		Self {
			if_not_exists: false,
			name: None,
			schedule: None,
			completion: None,
			enable: true,
			comment: None,
			body: None,
		}
	}

	/// Set IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Set event name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event");
	/// ```
	pub fn name<N: IntoIden>(&mut self, name: N) -> &mut Self {
		self.name = Some(name.into_iden());
		self
	}

	/// Set ON SCHEDULE AT clause (execute once at specific timestamp)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_schedule_at("2026-12-31 23:59:59");
	/// ```
	pub fn on_schedule_at<T: Into<String>>(&mut self, timestamp: T) -> &mut Self {
		self.schedule = Some(EventSchedule::At {
			timestamp: timestamp.into(),
		});
		self
	}

	/// Set ON SCHEDULE EVERY clause (execute repeatedly at intervals)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_schedule_every("1 DAY");
	/// ```
	pub fn on_schedule_every<I: Into<String>>(&mut self, interval: I) -> &mut Self {
		// Preserve existing starts/ends if already set
		let (starts, ends) = if let Some(EventSchedule::Every { starts, ends, .. }) = &self.schedule
		{
			(starts.clone(), ends.clone())
		} else {
			(None, None)
		};

		self.schedule = Some(EventSchedule::Every {
			interval: interval.into(),
			starts,
			ends,
		});
		self
	}

	/// Set STARTS clause for EVERY schedule
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_schedule_every("1 DAY")
	///     .starts("2026-01-01 00:00:00");
	/// ```
	pub fn starts<T: Into<String>>(&mut self, timestamp: T) -> &mut Self {
		if let Some(EventSchedule::Every {
			interval,
			ends,
			starts: _,
		}) = &self.schedule
		{
			self.schedule = Some(EventSchedule::Every {
				interval: interval.clone(),
				starts: Some(timestamp.into()),
				ends: ends.clone(),
			});
		}
		self
	}

	/// Set ENDS clause for EVERY schedule
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_schedule_every("1 DAY")
	///     .ends("2026-12-31 23:59:59");
	/// ```
	pub fn ends<T: Into<String>>(&mut self, timestamp: T) -> &mut Self {
		if let Some(EventSchedule::Every {
			interval,
			starts,
			ends: _,
		}) = &self.schedule
		{
			self.schedule = Some(EventSchedule::Every {
				interval: interval.clone(),
				starts: starts.clone(),
				ends: Some(timestamp.into()),
			});
		}
		self
	}

	/// Set ON COMPLETION PRESERVE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_completion_preserve();
	/// ```
	pub fn on_completion_preserve(&mut self) -> &mut Self {
		self.completion = Some(EventCompletion::Preserve);
		self
	}

	/// Set ON COMPLETION NOT PRESERVE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .on_completion_not_preserve();
	/// ```
	pub fn on_completion_not_preserve(&mut self) -> &mut Self {
		self.completion = Some(EventCompletion::NotPreserve);
		self
	}

	/// Set ENABLE clause (default)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .enable();
	/// ```
	pub fn enable(&mut self) -> &mut Self {
		self.enable = true;
		self
	}

	/// Set DISABLE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .disable();
	/// ```
	pub fn disable(&mut self) -> &mut Self {
		self.enable = false;
		self
	}

	/// Set COMMENT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .comment("Daily cleanup task");
	/// ```
	pub fn comment<C: Into<String>>(&mut self, comment: C) -> &mut Self {
		self.comment = Some(comment.into());
		self
	}

	/// Set DO clause (event body)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::create_event()
	///     .name("my_event")
	///     .do_body("DELETE FROM logs WHERE created_at < NOW() - INTERVAL 30 DAY");
	/// ```
	pub fn do_body<B: Into<String>>(&mut self, body: B) -> &mut Self {
		self.body = Some(body.into());
		self
	}
}

impl Default for CreateEventStatement {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_event_new() {
		let stmt = CreateEventStatement::new();
		assert!(!stmt.if_not_exists);
		assert!(stmt.name.is_none());
		assert!(stmt.schedule.is_none());
		assert!(stmt.completion.is_none());
		assert!(stmt.enable);
		assert!(stmt.comment.is_none());
		assert!(stmt.body.is_none());
	}

	#[rstest]
	fn test_create_event_if_not_exists() {
		let mut stmt = CreateEventStatement::new();
		stmt.if_not_exists();
		assert!(stmt.if_not_exists);
	}

	#[rstest]
	fn test_create_event_name() {
		let mut stmt = CreateEventStatement::new();
		stmt.name("my_event");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
	}

	#[rstest]
	fn test_create_event_on_schedule_at() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_schedule_at("2026-12-31 23:59:59");
		assert!(matches!(
			stmt.schedule.as_ref().unwrap(),
			EventSchedule::At { .. }
		));
		if let Some(EventSchedule::At { timestamp }) = &stmt.schedule {
			assert_eq!(timestamp, "2026-12-31 23:59:59");
		}
	}

	#[rstest]
	fn test_create_event_on_schedule_every() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_schedule_every("1 DAY");
		assert!(matches!(
			stmt.schedule.as_ref().unwrap(),
			EventSchedule::Every { .. }
		));
		if let Some(EventSchedule::Every {
			interval,
			starts,
			ends,
		}) = &stmt.schedule
		{
			assert_eq!(interval, "1 DAY");
			assert!(starts.is_none());
			assert!(ends.is_none());
		}
	}

	#[rstest]
	fn test_create_event_on_schedule_every_with_starts() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_schedule_every("1 HOUR")
			.starts("2026-01-01 00:00:00");
		if let Some(EventSchedule::Every {
			interval,
			starts,
			ends,
		}) = &stmt.schedule
		{
			assert_eq!(interval, "1 HOUR");
			assert_eq!(starts.as_ref().unwrap(), "2026-01-01 00:00:00");
			assert!(ends.is_none());
		}
	}

	#[rstest]
	fn test_create_event_on_schedule_every_with_ends() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_schedule_every("2 WEEK").ends("2026-12-31 23:59:59");
		if let Some(EventSchedule::Every {
			interval,
			starts,
			ends,
		}) = &stmt.schedule
		{
			assert_eq!(interval, "2 WEEK");
			assert!(starts.is_none());
			assert_eq!(ends.as_ref().unwrap(), "2026-12-31 23:59:59");
		}
	}

	#[rstest]
	fn test_create_event_on_schedule_every_with_starts_and_ends() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_schedule_every("1 MONTH")
			.starts("2026-01-01 00:00:00")
			.ends("2026-12-31 23:59:59");
		if let Some(EventSchedule::Every {
			interval,
			starts,
			ends,
		}) = &stmt.schedule
		{
			assert_eq!(interval, "1 MONTH");
			assert_eq!(starts.as_ref().unwrap(), "2026-01-01 00:00:00");
			assert_eq!(ends.as_ref().unwrap(), "2026-12-31 23:59:59");
		}
	}

	#[rstest]
	fn test_create_event_on_completion_preserve() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_completion_preserve();
		assert_eq!(stmt.completion, Some(EventCompletion::Preserve));
	}

	#[rstest]
	fn test_create_event_on_completion_not_preserve() {
		let mut stmt = CreateEventStatement::new();
		stmt.on_completion_not_preserve();
		assert_eq!(stmt.completion, Some(EventCompletion::NotPreserve));
	}

	#[rstest]
	fn test_create_event_enable() {
		let mut stmt = CreateEventStatement::new();
		stmt.enable();
		assert!(stmt.enable);
	}

	#[rstest]
	fn test_create_event_disable() {
		let mut stmt = CreateEventStatement::new();
		stmt.disable();
		assert!(!stmt.enable);
	}

	#[rstest]
	fn test_create_event_comment() {
		let mut stmt = CreateEventStatement::new();
		stmt.comment("Daily cleanup task");
		assert_eq!(stmt.comment.as_ref().unwrap(), "Daily cleanup task");
	}

	#[rstest]
	fn test_create_event_do_body() {
		let mut stmt = CreateEventStatement::new();
		stmt.do_body("DELETE FROM logs");
		assert_eq!(stmt.body.as_ref().unwrap(), "DELETE FROM logs");
	}

	#[rstest]
	fn test_create_event_all_options() {
		let mut stmt = CreateEventStatement::new();
		stmt.if_not_exists()
			.name("my_event")
			.on_schedule_every("1 DAY")
			.starts("2026-01-01 00:00:00")
			.ends("2026-12-31 23:59:59")
			.on_completion_preserve()
			.disable()
			.comment("Test event")
			.do_body("INSERT INTO logs VALUES (NOW())");

		assert!(stmt.if_not_exists);
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
		assert!(matches!(
			stmt.schedule.as_ref().unwrap(),
			EventSchedule::Every { .. }
		));
		assert_eq!(stmt.completion, Some(EventCompletion::Preserve));
		assert!(!stmt.enable);
		assert_eq!(stmt.comment.as_ref().unwrap(), "Test event");
		assert_eq!(
			stmt.body.as_ref().unwrap(),
			"INSERT INTO logs VALUES (NOW())"
		);
	}
}
