//! Database lifecycle event helpers
//!
//! These helpers provide convenient database lifecycle events similar to SQLAlchemy,
//! integrated with the Django-style signal system.

use super::core::SignalName;
use super::registry::get_signal;
use super::signal::Signal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Generic database event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbEvent {
	/// Type of database event (e.g., "insert", "update", "delete").
	pub event_type: String,
	/// Name of the database table affected.
	pub table: String,
	/// Optional primary key of the affected row.
	pub id: Option<String>,
	/// Additional key-value data associated with the event.
	pub data: HashMap<String, String>,
}

impl DbEvent {
	/// Creates a new database event with the given type and table name.
	pub fn new(event_type: impl Into<String>, table: impl Into<String>) -> Self {
		Self {
			event_type: event_type.into(),
			table: table.into(),
			id: None,
			data: HashMap::new(),
		}
	}

	/// Sets the primary key of the affected row.
	pub fn with_id(mut self, id: impl Into<String>) -> Self {
		self.id = Some(id.into());
		self
	}

	/// Adds a key-value data pair to the event.
	pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.data.insert(key.into(), value.into());
		self
	}
}

/// Before insert signal
pub fn before_insert() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_BEFORE_INSERT)
}

/// After insert signal
pub fn after_insert() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_AFTER_INSERT)
}

/// Before update signal
pub fn before_update() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_BEFORE_UPDATE)
}

/// After update signal
pub fn after_update() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_AFTER_UPDATE)
}

/// Before delete signal
pub fn before_delete() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_BEFORE_DELETE)
}

/// After delete signal
pub fn after_delete() -> Signal<DbEvent> {
	get_signal::<DbEvent>(SignalName::DB_AFTER_DELETE)
}
