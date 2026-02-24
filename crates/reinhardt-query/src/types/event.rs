//! Event type definitions for MySQL scheduled tasks
//!
//! This module provides types for event-related DDL operations (MySQL-specific):
//!
//! - [`EventSchedule`]: Schedule configuration (AT or EVERY)
//! - [`EventCompletion`]: Event completion behavior (PRESERVE or NOT PRESERVE)
//! - [`EventDef`]: Event definition for CREATE EVENT
//! - [`EventOperation`]: Operations for ALTER EVENT
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_query::types::event::{EventDef, EventSchedule, EventCompletion};
//!
//! // CREATE EVENT one_time_event
//! // ON SCHEDULE AT '2026-12-31 23:59:59'
//! // DO INSERT INTO logs VALUES (NOW())
//! let event = EventDef::new("one_time_event")
//!     .schedule(EventSchedule::At {
//!         timestamp: "2026-12-31 23:59:59".to_string(),
//!     })
//!     .body("INSERT INTO logs VALUES (NOW())");
//!
//! // CREATE EVENT recurring_event
//! // ON SCHEDULE EVERY 1 HOUR
//! // DO DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 1 DAY
//! let event = EventDef::new("recurring_event")
//!     .schedule(EventSchedule::Every {
//!         interval: "1 HOUR".to_string(),
//!         starts: None,
//!         ends: None,
//!     })
//!     .body("DELETE FROM temp_data WHERE created_at < NOW() - INTERVAL 1 DAY");
//! ```

use crate::types::{DynIden, IntoIden};

/// Event schedule configuration
///
/// Specifies when and how often an event should execute.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::event::EventSchedule;
///
/// // Execute once at specific timestamp
/// let schedule = EventSchedule::At {
///     timestamp: "2026-12-31 23:59:59".to_string(),
/// };
///
/// // Execute every interval
/// let schedule = EventSchedule::Every {
///     interval: "1 DAY".to_string(),
///     starts: Some("2026-01-01 00:00:00".to_string()),
///     ends: Some("2026-12-31 23:59:59".to_string()),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSchedule {
	/// Execute once at specific timestamp
	At {
		/// Timestamp when event should execute
		timestamp: String,
	},
	/// Execute repeatedly at intervals
	Every {
		/// Interval expression (e.g., "1 DAY", "2 HOUR")
		interval: String,
		/// Optional start timestamp
		starts: Option<String>,
		/// Optional end timestamp
		ends: Option<String>,
	},
}

/// Event completion behavior
///
/// Specifies whether the event should be preserved or dropped after execution.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::event::EventCompletion;
///
/// let completion = EventCompletion::Preserve;    // Keep event after execution
/// let completion = EventCompletion::NotPreserve; // Drop event after execution
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventCompletion {
	/// Keep event after execution (ON COMPLETION PRESERVE)
	Preserve,
	/// Drop event after execution (ON COMPLETION NOT PRESERVE)
	NotPreserve,
}

/// Event definition for CREATE EVENT
///
/// This struct represents an event definition, including its name,
/// schedule, completion behavior, enable status, comment, and body.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::event::{EventDef, EventSchedule, EventCompletion};
///
/// // CREATE EVENT my_event ON SCHEDULE AT '2026-12-31 23:59:59'
/// // DO INSERT INTO logs VALUES (NOW())
/// let event = EventDef::new("my_event")
///     .schedule(EventSchedule::At {
///         timestamp: "2026-12-31 23:59:59".to_string(),
///     })
///     .body("INSERT INTO logs VALUES (NOW())");
/// ```
#[derive(Debug, Clone)]
pub struct EventDef {
	#[allow(dead_code)] // Will be used in backend implementations for event name reference
	pub(crate) name: DynIden,
	pub(crate) schedule: Option<EventSchedule>,
	pub(crate) completion: Option<EventCompletion>,
	pub(crate) enable: Option<bool>,
	pub(crate) comment: Option<String>,
	pub(crate) body: Option<String>,
}

/// Operations for ALTER EVENT
///
/// Specifies modifications to an existing event.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::event::{EventOperation, EventSchedule, EventCompletion};
/// use reinhardt_query::types::IntoIden;
///
/// let op = EventOperation::RenameTo("new_event".into_iden());
/// let op = EventOperation::OnSchedule(EventSchedule::Every {
///     interval: "2 HOUR".to_string(),
///     starts: None,
///     ends: None,
/// });
/// let op = EventOperation::OnCompletion(EventCompletion::Preserve);
/// let op = EventOperation::Enable;
/// let op = EventOperation::Disable;
/// let op = EventOperation::Comment("Updated comment".to_string());
/// ```
#[derive(Debug, Clone)]
pub enum EventOperation {
	/// Rename event to new name
	RenameTo(DynIden),
	/// Change event schedule
	OnSchedule(EventSchedule),
	/// Change completion behavior
	OnCompletion(EventCompletion),
	/// Enable event
	Enable,
	/// Disable event
	Disable,
	/// Set event comment
	Comment(String),
}

impl EventDef {
	/// Create a new event definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::EventDef;
	///
	/// let event = EventDef::new("my_event");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			schedule: None,
			completion: None,
			enable: None,
			comment: None,
			body: None,
		}
	}

	/// Set event schedule
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::{EventDef, EventSchedule};
	///
	/// let event = EventDef::new("my_event")
	///     .schedule(EventSchedule::At {
	///         timestamp: "2026-12-31 23:59:59".to_string(),
	///     });
	/// ```
	pub fn schedule(mut self, schedule: EventSchedule) -> Self {
		self.schedule = Some(schedule);
		self
	}

	/// Set completion behavior
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::{EventDef, EventCompletion};
	///
	/// let event = EventDef::new("my_event")
	///     .completion(EventCompletion::Preserve);
	/// ```
	pub fn completion(mut self, completion: EventCompletion) -> Self {
		self.completion = Some(completion);
		self
	}

	/// Set enable status
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::EventDef;
	///
	/// let event = EventDef::new("my_event")
	///     .enable(true);
	/// ```
	pub fn enable(mut self, enable: bool) -> Self {
		self.enable = Some(enable);
		self
	}

	/// Set event comment
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::EventDef;
	///
	/// let event = EventDef::new("my_event")
	///     .comment("Runs daily cleanup");
	/// ```
	pub fn comment<C: Into<String>>(mut self, comment: C) -> Self {
		self.comment = Some(comment.into());
		self
	}

	/// Set event body
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::event::EventDef;
	///
	/// let event = EventDef::new("my_event")
	///     .body("DELETE FROM logs WHERE created_at < NOW() - INTERVAL 30 DAY");
	/// ```
	pub fn body<B: Into<String>>(mut self, body: B) -> Self {
		self.body = Some(body.into());
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// EventSchedule tests
	#[rstest]
	fn test_event_schedule_at() {
		let schedule = EventSchedule::At {
			timestamp: "2026-12-31 23:59:59".to_string(),
		};
		assert!(matches!(schedule, EventSchedule::At { .. }));
		if let EventSchedule::At { timestamp } = schedule {
			assert_eq!(timestamp, "2026-12-31 23:59:59");
		}
	}

	#[rstest]
	fn test_event_schedule_every_basic() {
		let schedule = EventSchedule::Every {
			interval: "1 DAY".to_string(),
			starts: None,
			ends: None,
		};
		assert!(matches!(schedule, EventSchedule::Every { .. }));
		if let EventSchedule::Every {
			interval,
			starts,
			ends,
		} = schedule
		{
			assert_eq!(interval, "1 DAY");
			assert!(starts.is_none());
			assert!(ends.is_none());
		}
	}

	#[rstest]
	fn test_event_schedule_every_with_starts() {
		let schedule = EventSchedule::Every {
			interval: "2 HOUR".to_string(),
			starts: Some("2026-01-01 00:00:00".to_string()),
			ends: None,
		};
		if let EventSchedule::Every {
			interval,
			starts,
			ends,
		} = schedule
		{
			assert_eq!(interval, "2 HOUR");
			assert_eq!(starts.as_ref().unwrap(), "2026-01-01 00:00:00");
			assert!(ends.is_none());
		}
	}

	#[rstest]
	fn test_event_schedule_every_with_ends() {
		let schedule = EventSchedule::Every {
			interval: "3 WEEK".to_string(),
			starts: None,
			ends: Some("2026-12-31 23:59:59".to_string()),
		};
		if let EventSchedule::Every {
			interval,
			starts,
			ends,
		} = schedule
		{
			assert_eq!(interval, "3 WEEK");
			assert!(starts.is_none());
			assert_eq!(ends.as_ref().unwrap(), "2026-12-31 23:59:59");
		}
	}

	#[rstest]
	fn test_event_schedule_every_with_starts_and_ends() {
		let schedule = EventSchedule::Every {
			interval: "1 MONTH".to_string(),
			starts: Some("2026-01-01 00:00:00".to_string()),
			ends: Some("2026-12-31 23:59:59".to_string()),
		};
		if let EventSchedule::Every {
			interval,
			starts,
			ends,
		} = schedule
		{
			assert_eq!(interval, "1 MONTH");
			assert_eq!(starts.as_ref().unwrap(), "2026-01-01 00:00:00");
			assert_eq!(ends.as_ref().unwrap(), "2026-12-31 23:59:59");
		}
	}

	// EventCompletion tests
	#[rstest]
	fn test_event_completion_preserve() {
		let completion = EventCompletion::Preserve;
		assert_eq!(completion, EventCompletion::Preserve);
	}

	#[rstest]
	fn test_event_completion_not_preserve() {
		let completion = EventCompletion::NotPreserve;
		assert_eq!(completion, EventCompletion::NotPreserve);
	}

	// EventDef tests
	#[rstest]
	fn test_event_def_basic() {
		let event = EventDef::new("my_event");
		assert_eq!(event.name.to_string(), "my_event");
		assert!(event.schedule.is_none());
		assert!(event.completion.is_none());
		assert!(event.enable.is_none());
		assert!(event.comment.is_none());
		assert!(event.body.is_none());
	}

	#[rstest]
	fn test_event_def_with_schedule_at() {
		let event = EventDef::new("my_event").schedule(EventSchedule::At {
			timestamp: "2026-12-31 23:59:59".to_string(),
		});
		assert!(event.schedule.is_some());
		assert!(matches!(
			event.schedule.as_ref().unwrap(),
			EventSchedule::At { .. }
		));
	}

	#[rstest]
	fn test_event_def_with_schedule_every() {
		let event = EventDef::new("my_event").schedule(EventSchedule::Every {
			interval: "1 DAY".to_string(),
			starts: None,
			ends: None,
		});
		assert!(event.schedule.is_some());
		assert!(matches!(
			event.schedule.as_ref().unwrap(),
			EventSchedule::Every { .. }
		));
	}

	#[rstest]
	fn test_event_def_with_completion_preserve() {
		let event = EventDef::new("my_event").completion(EventCompletion::Preserve);
		assert_eq!(event.completion, Some(EventCompletion::Preserve));
	}

	#[rstest]
	fn test_event_def_with_completion_not_preserve() {
		let event = EventDef::new("my_event").completion(EventCompletion::NotPreserve);
		assert_eq!(event.completion, Some(EventCompletion::NotPreserve));
	}

	#[rstest]
	fn test_event_def_enable_true() {
		let event = EventDef::new("my_event").enable(true);
		assert_eq!(event.enable, Some(true));
	}

	#[rstest]
	fn test_event_def_enable_false() {
		let event = EventDef::new("my_event").enable(false);
		assert_eq!(event.enable, Some(false));
	}

	#[rstest]
	fn test_event_def_with_comment() {
		let event = EventDef::new("my_event").comment("Daily cleanup task");
		assert_eq!(event.comment.as_ref().unwrap(), "Daily cleanup task");
	}

	#[rstest]
	fn test_event_def_with_body() {
		let event = EventDef::new("my_event").body("DELETE FROM temp_data");
		assert_eq!(event.body.as_ref().unwrap(), "DELETE FROM temp_data");
	}

	#[rstest]
	fn test_event_def_all_options() {
		let event = EventDef::new("my_event")
			.schedule(EventSchedule::Every {
				interval: "1 DAY".to_string(),
				starts: Some("2026-01-01 00:00:00".to_string()),
				ends: Some("2026-12-31 23:59:59".to_string()),
			})
			.completion(EventCompletion::Preserve)
			.enable(true)
			.comment("Daily cleanup")
			.body("DELETE FROM logs WHERE created_at < NOW() - INTERVAL 30 DAY");

		assert_eq!(event.name.to_string(), "my_event");
		assert!(event.schedule.is_some());
		assert_eq!(event.completion, Some(EventCompletion::Preserve));
		assert_eq!(event.enable, Some(true));
		assert_eq!(event.comment.as_ref().unwrap(), "Daily cleanup");
		assert_eq!(
			event.body.as_ref().unwrap(),
			"DELETE FROM logs WHERE created_at < NOW() - INTERVAL 30 DAY"
		);
	}

	// EventOperation tests
	#[rstest]
	fn test_event_operation_rename_to() {
		let op = EventOperation::RenameTo("new_event".into_iden());
		assert!(matches!(op, EventOperation::RenameTo(_)));
	}

	#[rstest]
	fn test_event_operation_on_schedule_at() {
		let op = EventOperation::OnSchedule(EventSchedule::At {
			timestamp: "2026-12-31 23:59:59".to_string(),
		});
		assert!(matches!(op, EventOperation::OnSchedule(_)));
	}

	#[rstest]
	fn test_event_operation_on_schedule_every() {
		let op = EventOperation::OnSchedule(EventSchedule::Every {
			interval: "2 HOUR".to_string(),
			starts: None,
			ends: None,
		});
		assert!(matches!(op, EventOperation::OnSchedule(_)));
	}

	#[rstest]
	fn test_event_operation_on_completion_preserve() {
		let op = EventOperation::OnCompletion(EventCompletion::Preserve);
		assert!(matches!(
			op,
			EventOperation::OnCompletion(EventCompletion::Preserve)
		));
	}

	#[rstest]
	fn test_event_operation_on_completion_not_preserve() {
		let op = EventOperation::OnCompletion(EventCompletion::NotPreserve);
		assert!(matches!(
			op,
			EventOperation::OnCompletion(EventCompletion::NotPreserve)
		));
	}

	#[rstest]
	fn test_event_operation_enable() {
		let op = EventOperation::Enable;
		assert!(matches!(op, EventOperation::Enable));
	}

	#[rstest]
	fn test_event_operation_disable() {
		let op = EventOperation::Disable;
		assert!(matches!(op, EventOperation::Disable));
	}

	#[rstest]
	fn test_event_operation_comment() {
		let op = EventOperation::Comment("Updated comment".to_string());
		assert!(matches!(op, EventOperation::Comment(_)));
		if let EventOperation::Comment(comment) = op {
			assert_eq!(comment, "Updated comment");
		}
	}
}
