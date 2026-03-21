//! Admin panel integration test fixtures
//!
//! This module provides rstest fixtures for admin panel integration tests,
//! including pre-populated audit loggers and test data sets.

use reinhardt_conf::settings::audit::{
	AuditBackend, AuditEvent, ChangeRecord, EventType, backends::MemoryAuditBackend,
};
use rstest::*;
use std::{collections::HashMap, sync::Arc};

/// Fixture providing a MemoryAuditBackend with 10,000 test log entries
///
/// This fixture creates a MemoryAuditBackend pre-populated with 10,000 audit log entries
/// for performance and functionality testing.
///
/// Test data distribution:
/// - Users: 100 different users (user_0 ~ user_99)
/// - Models: 10 different models (Model_0 ~ Model_9)
/// - Actions: Alternating "ConfigCreate" and "ConfigUpdate" actions
/// - Each user has approximately 100 log entries (10,000 / 100)
/// - Each model has approximately 1,000 log entries (10,000 / 10)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::admin::audit_logger_with_test_data;
/// use reinhardt_conf::settings::audit::{AuditBackend, EventFilter};
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_audit_query(
///     #[future] audit_logger_with_test_data: Arc<MemoryAuditBackend>
/// ) {
///     let backend = audit_logger_with_test_data.await;
///
///     // Query for specific user
///     let filter = EventFilter {
///         user: Some("user_50".to_string()),
///         ..Default::default()
///     };
///     let results = backend.get_events(Some(filter)).await.unwrap();
///
///     // Expected: 10,000 / 100 users = 100 logs per user
///     assert_eq!(results.len(), 100);
/// }
/// ```
///
/// # Note
/// This fixture is async and requires `#[future]` attribute when used in rstest.
/// The returned Arc allows sharing the backend across multiple test assertions
/// without cloning the data.
#[fixture]
pub async fn audit_logger_with_test_data() -> Arc<MemoryAuditBackend> {
	let backend = MemoryAuditBackend::new();

	// Generate 10,000 test entries
	for i in 0..10000 {
		let user = format!("user_{}", i % 100);
		let model_name = format!("Model_{}", i % 10);
		let object_id = i.to_string();
		let event_type = if i % 2 == 0 {
			EventType::ConfigCreate
		} else {
			EventType::ConfigUpdate
		};

		// Store model_name and object_id in changes HashMap
		let mut changes = HashMap::new();
		changes.insert(
			"model".to_string(),
			ChangeRecord {
				old_value: None,
				new_value: Some(serde_json::json!({
					"name": model_name,
					"id": object_id,
				})),
			},
		);

		let event = AuditEvent::new(event_type, Some(user), changes);
		backend.log_event(event).await.expect("Failed to log event");
	}

	Arc::new(backend)
}

/// Simplified audit log entry for testing
///
/// This struct represents a single audit log entry for use in tests.
/// It matches the structure expected by MemoryAuditLogger.
#[derive(Clone, Debug)]
pub struct TestAuditLogEntry {
	/// Identifier of the user who performed the action.
	pub user_id: String,
	/// Name of the model that was affected.
	pub model_name: String,
	/// Identifier of the affected object.
	pub object_id: String,
	/// Description of the action performed (e.g., `"create"`, `"update"`).
	pub action: String,
	/// Optional JSON value describing the changes made.
	pub changes: Option<serde_json::Value>,
	/// Timestamp when the action occurred.
	pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Generate a single test audit log entry
///
/// # Arguments
/// * `index` - Entry index (used for generating user/model IDs)
/// * `user_count` - Total number of users for modulo distribution
/// * `model_count` - Total number of models for modulo distribution
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::admin::generate_test_audit_entry;
///
/// let entry = generate_test_audit_entry(0, 100, 10);
/// assert_eq!(entry.user_id, "user_0");
/// assert_eq!(entry.model_name, "Model_0");
/// assert_eq!(entry.action, "Create");
/// ```
pub fn generate_test_audit_entry(
	index: usize,
	user_count: usize,
	model_count: usize,
) -> TestAuditLogEntry {
	TestAuditLogEntry {
		user_id: format!("user_{}", index % user_count),
		model_name: format!("Model_{}", index % model_count),
		object_id: index.to_string(),
		action: if index.is_multiple_of(2) {
			"Create".to_string()
		} else {
			"Update".to_string()
		},
		changes: None,
		timestamp: chrono::Utc::now(),
	}
}

/// Generate multiple test audit log entries
///
/// # Arguments
/// * `count` - Number of entries to generate
/// * `user_count` - Total number of users for distribution
/// * `model_count` - Total number of models for distribution
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::admin::generate_test_audit_entries;
///
/// let entries = generate_test_audit_entries(10000, 100, 10);
/// assert_eq!(entries.len(), 10000);
///
/// // Each user should have approximately 100 entries
/// let user_50_entries: Vec<_> = entries
///     .iter()
///     .filter(|e| e.user_id == "user_50")
///     .collect();
/// assert_eq!(user_50_entries.len(), 100);
/// ```
pub fn generate_test_audit_entries(
	count: usize,
	user_count: usize,
	model_count: usize,
) -> Vec<TestAuditLogEntry> {
	(0..count)
		.map(|i| generate_test_audit_entry(i, user_count, model_count))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_generate_test_audit_entry() {
		let entry = generate_test_audit_entry(0, 100, 10);
		assert_eq!(entry.user_id, "user_0");
		assert_eq!(entry.model_name, "Model_0");
		assert_eq!(entry.action, "Create");

		let entry = generate_test_audit_entry(1, 100, 10);
		assert_eq!(entry.user_id, "user_1");
		assert_eq!(entry.model_name, "Model_1");
		assert_eq!(entry.action, "Update");

		let entry = generate_test_audit_entry(150, 100, 10);
		assert_eq!(entry.user_id, "user_50");
		assert_eq!(entry.model_name, "Model_0");
	}

	#[test]
	fn test_generate_test_audit_entries() {
		let entries = generate_test_audit_entries(10000, 100, 10);
		assert_eq!(entries.len(), 10000);

		// Verify user distribution
		let user_50_entries: Vec<_> = entries.iter().filter(|e| e.user_id == "user_50").collect();
		assert_eq!(user_50_entries.len(), 100);

		// Verify model distribution
		let model_5_entries: Vec<_> = entries
			.iter()
			.filter(|e| e.model_name == "Model_5")
			.collect();
		assert_eq!(model_5_entries.len(), 1000);
	}
}
