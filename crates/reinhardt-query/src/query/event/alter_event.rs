//! ALTER EVENT statement builder (MySQL-specific)
//!
//! This module provides a builder for MySQL ALTER EVENT statements,
//! which are used to modify existing scheduled tasks.
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // ALTER EVENT my_event RENAME TO new_event
//! let stmt = Query::alter_event()
//!     .name("my_event")
//!     .rename_to("new_event");
//!
//! // ALTER EVENT my_event
//! // ON SCHEDULE EVERY 2 HOUR
//! // ON COMPLETION PRESERVE
//! // ENABLE
//! let stmt = Query::alter_event()
//!     .name("my_event")
//!     .on_schedule_every("2 HOUR")
//!     .on_completion_preserve()
//!     .enable();
//! ```

use crate::types::event::{EventCompletion, EventOperation, EventSchedule};
use crate::types::{DynIden, IntoIden};

/// ALTER EVENT statement builder (MySQL-specific)
///
/// This struct provides a builder for constructing ALTER EVENT statements
/// for modifying MySQL scheduled tasks.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Rename event
/// let stmt = Query::alter_event()
///     .name("my_event")
///     .rename_to("new_event");
///
/// // Change schedule
/// let stmt = Query::alter_event()
///     .name("my_event")
///     .on_schedule_at("2026-12-31 23:59:59");
///
/// // Enable/disable event
/// let stmt = Query::alter_event()
///     .name("my_event")
///     .enable();
/// ```
#[derive(Debug, Clone)]
pub struct AlterEventStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) operations: Vec<EventOperation>,
}

impl AlterEventStatement {
	/// Create a new ALTER EVENT statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::event::AlterEventStatement;
	///
	/// let stmt = AlterEventStatement::new();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			operations: Vec::new(),
		}
	}

	/// Set event name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event");
	/// ```
	pub fn name<N: IntoIden>(&mut self, name: N) -> &mut Self {
		self.name = Some(name.into_iden());
		self
	}

	/// Add RENAME TO operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .rename_to("new_event");
	/// ```
	pub fn rename_to<N: IntoIden>(&mut self, new_name: N) -> &mut Self {
		self.operations
			.push(EventOperation::RenameTo(new_name.into_iden()));
		self
	}

	/// Add ON SCHEDULE AT operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .on_schedule_at("2026-12-31 23:59:59");
	/// ```
	pub fn on_schedule_at<T: Into<String>>(&mut self, timestamp: T) -> &mut Self {
		self.operations
			.push(EventOperation::OnSchedule(EventSchedule::At {
				timestamp: timestamp.into(),
			}));
		self
	}

	/// Add ON SCHEDULE EVERY operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .on_schedule_every("2 HOUR");
	/// ```
	pub fn on_schedule_every<I: Into<String>>(&mut self, interval: I) -> &mut Self {
		self.operations
			.push(EventOperation::OnSchedule(EventSchedule::Every {
				interval: interval.into(),
				starts: None,
				ends: None,
			}));
		self
	}

	/// Add ON COMPLETION PRESERVE operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .on_completion_preserve();
	/// ```
	pub fn on_completion_preserve(&mut self) -> &mut Self {
		self.operations
			.push(EventOperation::OnCompletion(EventCompletion::Preserve));
		self
	}

	/// Add ON COMPLETION NOT PRESERVE operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .on_completion_not_preserve();
	/// ```
	pub fn on_completion_not_preserve(&mut self) -> &mut Self {
		self.operations
			.push(EventOperation::OnCompletion(EventCompletion::NotPreserve));
		self
	}

	/// Add ENABLE operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .enable();
	/// ```
	pub fn enable(&mut self) -> &mut Self {
		self.operations.push(EventOperation::Enable);
		self
	}

	/// Add DISABLE operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .disable();
	/// ```
	pub fn disable(&mut self) -> &mut Self {
		self.operations.push(EventOperation::Disable);
		self
	}

	/// Add COMMENT operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::alter_event()
	///     .name("my_event")
	///     .comment("Updated comment");
	/// ```
	pub fn comment<C: Into<String>>(&mut self, comment: C) -> &mut Self {
		self.operations
			.push(EventOperation::Comment(comment.into()));
		self
	}
}

impl Default for AlterEventStatement {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_event_new() {
		let stmt = AlterEventStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.operations.is_empty());
	}

	#[rstest]
	fn test_alter_event_name() {
		let mut stmt = AlterEventStatement::new();
		stmt.name("my_event");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
	}

	#[rstest]
	fn test_alter_event_rename_to() {
		let mut stmt = AlterEventStatement::new();
		stmt.rename_to("new_event");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(&stmt.operations[0], EventOperation::RenameTo(_)));
	}

	#[rstest]
	fn test_alter_event_on_schedule_at() {
		let mut stmt = AlterEventStatement::new();
		stmt.on_schedule_at("2026-12-31 23:59:59");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			EventOperation::OnSchedule(EventSchedule::At { .. })
		));
	}

	#[rstest]
	fn test_alter_event_on_schedule_every() {
		let mut stmt = AlterEventStatement::new();
		stmt.on_schedule_every("2 HOUR");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			EventOperation::OnSchedule(EventSchedule::Every { .. })
		));
	}

	#[rstest]
	fn test_alter_event_on_completion_preserve() {
		let mut stmt = AlterEventStatement::new();
		stmt.on_completion_preserve();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			EventOperation::OnCompletion(EventCompletion::Preserve)
		));
	}

	#[rstest]
	fn test_alter_event_on_completion_not_preserve() {
		let mut stmt = AlterEventStatement::new();
		stmt.on_completion_not_preserve();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			EventOperation::OnCompletion(EventCompletion::NotPreserve)
		));
	}

	#[rstest]
	fn test_alter_event_enable() {
		let mut stmt = AlterEventStatement::new();
		stmt.enable();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(&stmt.operations[0], EventOperation::Enable));
	}

	#[rstest]
	fn test_alter_event_disable() {
		let mut stmt = AlterEventStatement::new();
		stmt.disable();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(&stmt.operations[0], EventOperation::Disable));
	}

	#[rstest]
	fn test_alter_event_comment() {
		let mut stmt = AlterEventStatement::new();
		stmt.comment("Updated comment");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(&stmt.operations[0], EventOperation::Comment(_)));
	}

	#[rstest]
	fn test_alter_event_multiple_operations() {
		let mut stmt = AlterEventStatement::new();
		stmt.name("my_event")
			.on_schedule_every("2 HOUR")
			.on_completion_preserve()
			.enable()
			.comment("Updated");

		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
		assert_eq!(stmt.operations.len(), 4);
		assert!(matches!(
			&stmt.operations[0],
			EventOperation::OnSchedule(EventSchedule::Every { .. })
		));
		assert!(matches!(
			&stmt.operations[1],
			EventOperation::OnCompletion(EventCompletion::Preserve)
		));
		assert!(matches!(&stmt.operations[2], EventOperation::Enable));
		assert!(matches!(&stmt.operations[3], EventOperation::Comment(_)));
	}
}
