//! Thread-local settings override mechanism for integration testing
//!
//! Provides a per-test override guard that injects values into the settings
//! merge pipeline with highest priority. Designed for TestContainers scenarios
//! where dynamic container ports must override file-based configuration.
//!
//! # Examples
//!
//! ```
//! use reinhardt_conf::settings::testing::overrides::{SettingsOverride, SettingsOverrideGuard};
//! use reinhardt_conf::settings::builder::SettingsBuilder;
//! use reinhardt_conf::settings::sources::DefaultSource;
//! use serde_json::Value;
//!
//! // Activate overrides for this test scope
//! let _guard = SettingsOverride::new()
//!     .set("email.host", "127.0.0.1")
//!     .set("email.port", "2525")
//!     .activate();
//!
//! // SettingsBuilder::build() picks up thread-local overrides automatically
//! let settings = SettingsBuilder::new()
//!     .add_source(
//!         DefaultSource::new()
//!             .with_value("email", Value::Object(serde_json::Map::from_iter([
//!                 ("host".to_string(), Value::String("localhost".to_string())),
//!                 ("port".to_string(), Value::Number(1025.into())),
//!             ])))
//!     )
//!     .build()
//!     .unwrap();
//!
//! // Override values win
//! let email = settings.get_raw("email").unwrap().as_object().unwrap();
//! assert_eq!(email.get("host").unwrap(), &Value::String("127.0.0.1".to_string()));
//! assert_eq!(email.get("port").unwrap(), &Value::String("2525".to_string()));
//!
//! // Guard dropped here — overrides cleared automatically
//! ```

use indexmap::IndexMap;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
	static SETTINGS_OVERRIDES: RefCell<Option<IndexMap<String, Value>>> = const { RefCell::new(None) };
}

/// Builder for per-test settings overrides.
///
/// Creates a thread-local override layer that takes highest priority
/// when resolving settings values. The overrides are automatically
/// cleared when the returned [`SettingsOverrideGuard`] is dropped.
///
/// # Thread Safety
///
/// Overrides are stored in a `thread_local!`, so they are visible only
/// on the thread that called [`activate()`](SettingsOverride::activate).
/// `#[tokio::test]` runs on a single thread by default, which is the
/// intended usage.
pub struct SettingsOverride {
	values: IndexMap<String, Value>,
}

impl SettingsOverride {
	/// Create a new, empty settings override builder.
	pub fn new() -> Self {
		Self {
			values: IndexMap::new(),
		}
	}

	/// Set a dotted key to a string value.
	///
	/// Nested keys like `"email.host"` produce
	/// `{"email": {"host": "127.0.0.1"}}` in the override map.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::overrides::SettingsOverride;
	///
	/// let overrides = SettingsOverride::new()
	///     .set("email.host", "127.0.0.1")
	///     .set("email.port", "2525");
	/// ```
	pub fn set(self, key: &str, value: impl Into<String>) -> Self {
		self.set_value(key, Value::String(value.into()))
	}

	/// Set a dotted key to a typed `serde_json::Value`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::overrides::SettingsOverride;
	/// use serde_json::Value;
	///
	/// let overrides = SettingsOverride::new()
	///     .set_value("email.port", Value::Number(2525.into()));
	/// ```
	pub fn set_value(mut self, key: &str, value: Value) -> Self {
		let nested = build_nested(key, value);
		deep_merge(&mut self.values, nested);
		self
	}

	/// Activate the overrides, returning an RAII guard.
	///
	/// The overrides become visible to [`SettingsBuilder::build()`](crate::settings::builder::SettingsBuilder::build)
	/// on the current thread. When the guard is dropped, the overrides
	/// are cleared automatically.
	///
	/// # Panics
	///
	/// Panics if overrides are already active on this thread (nested
	/// activation is not supported).
	pub fn activate(self) -> SettingsOverrideGuard {
		SETTINGS_OVERRIDES.with(|cell| {
			let mut borrow = cell.borrow_mut();
			assert!(
				borrow.is_none(),
				"SettingsOverride is already active on this thread; \
				 nested activation is not supported"
			);
			*borrow = Some(self.values);
		});
		SettingsOverrideGuard { _private: () }
	}
}

impl Default for SettingsOverride {
	fn default() -> Self {
		Self::new()
	}
}

/// RAII guard that clears thread-local overrides on drop.
///
/// Returned by [`SettingsOverride::activate()`]. When dropped, the
/// thread-local override storage is reset to `None`.
pub struct SettingsOverrideGuard {
	_private: (),
}

impl Drop for SettingsOverrideGuard {
	fn drop(&mut self) {
		SETTINGS_OVERRIDES.with(|cell| {
			*cell.borrow_mut() = None;
		});
	}
}

/// Read the current thread-local overrides (if any).
///
/// Called by `SettingsBuilder::build()` to apply overrides after
/// merging all configuration sources.
pub(crate) fn current_overrides() -> Option<IndexMap<String, Value>> {
	SETTINGS_OVERRIDES.with(|cell| cell.borrow().clone())
}

/// Build a nested `IndexMap` from a dotted key path and leaf value.
///
/// `"email.host"` with value `"127.0.0.1"` produces:
/// `{"email": {"host": "127.0.0.1"}}`
fn build_nested(key: &str, value: Value) -> IndexMap<String, Value> {
	let parts: Vec<&str> = key.split('.').collect();
	let mut result = IndexMap::new();

	if parts.len() == 1 {
		result.insert(parts[0].to_string(), value);
		return result;
	}

	// Build from right to left: wrap the leaf value in nested objects
	// e.g., "a.b.c" with value V  →  {"a": {"b": {"c": V}}}
	let mut current = value;
	for &part in parts[1..].iter().rev() {
		let mut map = serde_json::Map::new();
		map.insert(part.to_string(), current);
		current = Value::Object(map);
	}

	result.insert(parts[0].to_string(), current);
	result
}

/// Deep-merge `source` into `target`.
///
/// When both sides have a `Value::Object` at the same key, merge
/// recursively. Otherwise the source value replaces the target.
pub(crate) fn deep_merge(target: &mut IndexMap<String, Value>, source: IndexMap<String, Value>) {
	for (key, source_val) in source {
		match target.get_mut(&key) {
			Some(Value::Object(target_obj)) if source_val.is_object() => {
				// Both are objects — merge recursively
				let source_obj = source_val.as_object().unwrap();
				for (k, v) in source_obj {
					deep_merge_json(target_obj, k.clone(), v.clone());
				}
			}
			_ => {
				target.insert(key, source_val);
			}
		}
	}
}

/// Recursive helper for merging into a `serde_json::Map`.
fn deep_merge_json(target: &mut serde_json::Map<String, Value>, key: String, value: Value) {
	match target.get_mut(&key) {
		Some(Value::Object(target_obj)) if value.is_object() => {
			let source_obj = value.as_object().unwrap();
			for (k, v) in source_obj {
				deep_merge_json(target_obj, k.clone(), v.clone());
			}
		}
		_ => {
			target.insert(key, value);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::builder::SettingsBuilder;
	use crate::settings::sources::DefaultSource;
	use rstest::rstest;

	#[rstest]
	fn test_override_sets_flat_key() {
		// Arrange
		let _guard = SettingsOverride::new().set("port", "8080").activate();

		// Act
		let settings = SettingsBuilder::new().build().unwrap();

		// Assert
		let port: String = settings.get("port").unwrap();
		assert_eq!(port, "8080");
	}

	#[rstest]
	fn test_override_sets_nested_key() {
		// Arrange
		let _guard = SettingsOverride::new()
			.set("email.host", "127.0.0.1")
			.activate();

		// Act
		let settings = SettingsBuilder::new().build().unwrap();

		// Assert
		let email = settings.get_raw("email").unwrap().as_object().unwrap();
		assert_eq!(
			email.get("host").unwrap(),
			&Value::String("127.0.0.1".to_string())
		);
	}

	#[rstest]
	fn test_override_multiple_keys() {
		// Arrange
		let _guard = SettingsOverride::new()
			.set("email.host", "127.0.0.1")
			.set("email.port", "2525")
			.set("debug", "true")
			.activate();

		// Act
		let settings = SettingsBuilder::new().build().unwrap();

		// Assert
		let email = settings.get_raw("email").unwrap().as_object().unwrap();
		assert_eq!(
			email.get("host").unwrap(),
			&Value::String("127.0.0.1".to_string())
		);
		assert_eq!(
			email.get("port").unwrap(),
			&Value::String("2525".to_string())
		);
		let debug: String = settings.get("debug").unwrap();
		assert_eq!(debug, "true");
	}

	#[rstest]
	fn test_override_guard_clears_on_drop() {
		// Arrange — activate and drop
		{
			let _guard = SettingsOverride::new().set("key", "value").activate();
			assert!(current_overrides().is_some());
		}

		// Act / Assert — overrides are gone after guard drop
		assert!(current_overrides().is_none());
	}

	#[rstest]
	fn test_override_wins_over_all_sources() {
		// Arrange
		let _guard = SettingsOverride::new().set("port", "9999").activate();

		// Act
		let settings = SettingsBuilder::new()
			.add_source(DefaultSource::new().with_value("port", Value::Number(8080.into())))
			.build()
			.unwrap();

		// Assert — override wins over DefaultSource
		let port: String = settings.get("port").unwrap();
		assert_eq!(port, "9999");
	}

	#[rstest]
	fn test_nested_merge_preserves_siblings() {
		// Arrange
		let _guard = SettingsOverride::new()
			.set("db.host", "localhost")
			.set("db.port", "5433")
			.activate();

		// Act
		let settings = SettingsBuilder::new().build().unwrap();

		// Assert — both nested keys present
		let db = settings.get_raw("db").unwrap().as_object().unwrap();
		assert_eq!(
			db.get("host").unwrap(),
			&Value::String("localhost".to_string())
		);
		assert_eq!(db.get("port").unwrap(), &Value::String("5433".to_string()));
	}

	#[rstest]
	fn test_set_value_with_typed_value() {
		// Arrange
		let _guard = SettingsOverride::new()
			.set_value("port", Value::Number(2525.into()))
			.activate();

		// Act
		let settings = SettingsBuilder::new().build().unwrap();

		// Assert — type is preserved as Number
		let port: i64 = settings.get("port").unwrap();
		assert_eq!(port, 2525);
	}

	#[rstest]
	fn test_deep_merge_override_into_existing_object() {
		// Arrange — DefaultSource has email object, override changes host
		let _guard = SettingsOverride::new()
			.set("email.host", "override-host")
			.activate();

		// Act
		let settings = SettingsBuilder::new()
			.add_source(DefaultSource::new().with_value(
				"email",
				Value::Object(serde_json::Map::from_iter([
					(
						"host".to_string(),
						Value::String("original-host".to_string()),
					),
					("port".to_string(), Value::Number(1025.into())),
				])),
			))
			.build()
			.unwrap();

		// Assert — host overridden, port preserved
		let email = settings.get_raw("email").unwrap().as_object().unwrap();
		assert_eq!(
			email.get("host").unwrap(),
			&Value::String("override-host".to_string())
		);
		assert_eq!(email.get("port").unwrap(), &Value::Number(1025.into()));
	}
}
