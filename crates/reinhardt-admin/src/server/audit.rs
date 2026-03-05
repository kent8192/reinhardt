//! Audit logging for admin CRUD operations
//!
//! This module provides structured audit logging for all administrative
//! operations (create, update, delete) to support security monitoring
//! and compliance requirements.
//!
//! Audit log entries include:
//! - Timestamp of the operation
//! - User identifier (from authentication state)
//! - Operation type (create, update, delete, bulk_delete)
//! - Target model and record ID
//! - Summary of changed fields (for updates)

use std::collections::HashMap;
use std::fmt;

/// Types of admin operations that are audit-logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
	/// A new record was created
	Create,
	/// An existing record was updated
	Update,
	/// A single record was deleted
	Delete,
	/// Multiple records were deleted
	BulkDelete,
	/// Data was exported
	Export,
	/// Data was imported
	Import,
}

impl fmt::Display for AuditAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AuditAction::Create => write!(f, "CREATE"),
			AuditAction::Update => write!(f, "UPDATE"),
			AuditAction::Delete => write!(f, "DELETE"),
			AuditAction::BulkDelete => write!(f, "BULK_DELETE"),
			AuditAction::Export => write!(f, "EXPORT"),
			AuditAction::Import => write!(f, "IMPORT"),
		}
	}
}

/// A single audit log entry representing an admin operation.
#[derive(Debug, Clone)]
pub struct AuditEntry {
	/// When the operation occurred (ISO 8601)
	pub timestamp: String,
	/// User identifier (user ID or "anonymous")
	pub user_id: String,
	/// Type of operation performed
	pub action: AuditAction,
	/// Name of the model affected
	pub model_name: String,
	/// Primary key of the affected record(s)
	pub record_id: Option<String>,
	/// Field names that were modified (for updates)
	pub changed_fields: Option<Vec<String>>,
	/// Whether the operation succeeded
	pub success: bool,
	/// Number of records affected (for bulk operations)
	pub affected_count: Option<u64>,
}

impl fmt::Display for AuditEntry {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"[ADMIN_AUDIT] {} user={} action={} model={}",
			self.timestamp, self.user_id, self.action, self.model_name,
		)?;

		if let Some(ref id) = self.record_id {
			write!(f, " record_id={}", id)?;
		}

		if let Some(ref fields) = self.changed_fields {
			write!(f, " changed_fields=[{}]", fields.join(", "))?;
		}

		if let Some(count) = self.affected_count {
			write!(f, " affected={}", count)?;
		}

		write!(f, " success={}", self.success)
	}
}

/// Logs a create operation to the audit trail.
///
/// Records that a new record was created, including which fields were set.
///
/// # Arguments
///
/// * `user_id` - The authenticated user's identifier
/// * `model_name` - The model being created
/// * `data` - The fields being set on the new record
/// * `success` - Whether the operation succeeded
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::audit::log_create;
/// use std::collections::HashMap;
///
/// let mut data = HashMap::new();
/// data.insert("name".to_string(), serde_json::json!("Alice"));
/// log_create("user-42", "User", &data, true);
/// ```
pub fn log_create(
	user_id: &str,
	model_name: &str,
	data: &HashMap<String, serde_json::Value>,
	success: bool,
) {
	let entry = AuditEntry {
		timestamp: chrono::Utc::now().to_rfc3339(),
		user_id: user_id.to_string(),
		action: AuditAction::Create,
		model_name: model_name.to_string(),
		record_id: None,
		changed_fields: Some(data.keys().cloned().collect()),
		success,
		affected_count: if success { Some(1) } else { None },
	};

	emit_audit_log(&entry);
}

/// Logs an update operation to the audit trail.
///
/// Records that an existing record was updated, including the record ID
/// and which fields were modified.
///
/// # Arguments
///
/// * `user_id` - The authenticated user's identifier
/// * `model_name` - The model being updated
/// * `record_id` - The primary key of the record being updated
/// * `data` - The fields being modified
/// * `success` - Whether the operation succeeded
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::audit::log_update;
/// use std::collections::HashMap;
///
/// let mut data = HashMap::new();
/// data.insert("email".to_string(), serde_json::json!("new@example.com"));
/// log_update("user-42", "User", "123", &data, true);
/// ```
pub fn log_update(
	user_id: &str,
	model_name: &str,
	record_id: &str,
	data: &HashMap<String, serde_json::Value>,
	success: bool,
) {
	let entry = AuditEntry {
		timestamp: chrono::Utc::now().to_rfc3339(),
		user_id: user_id.to_string(),
		action: AuditAction::Update,
		model_name: model_name.to_string(),
		record_id: Some(record_id.to_string()),
		changed_fields: Some(data.keys().cloned().collect()),
		success,
		affected_count: if success { Some(1) } else { None },
	};

	emit_audit_log(&entry);
}

/// Logs a delete operation to the audit trail.
///
/// # Arguments
///
/// * `user_id` - The authenticated user's identifier
/// * `model_name` - The model being deleted from
/// * `record_id` - The primary key of the deleted record
/// * `success` - Whether the operation succeeded
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::audit::log_delete;
///
/// log_delete("user-42", "User", "123", true);
/// ```
pub fn log_delete(user_id: &str, model_name: &str, record_id: &str, success: bool) {
	let entry = AuditEntry {
		timestamp: chrono::Utc::now().to_rfc3339(),
		user_id: user_id.to_string(),
		action: AuditAction::Delete,
		model_name: model_name.to_string(),
		record_id: Some(record_id.to_string()),
		changed_fields: None,
		success,
		affected_count: if success { Some(1) } else { None },
	};

	emit_audit_log(&entry);
}

/// Logs a bulk delete operation to the audit trail.
///
/// # Arguments
///
/// * `user_id` - The authenticated user's identifier
/// * `model_name` - The model being deleted from
/// * `record_ids` - The primary keys of the deleted records
/// * `affected` - Number of records actually deleted
/// * `success` - Whether the operation succeeded
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::audit::log_bulk_delete;
///
/// log_bulk_delete("user-42", "User", &["1".to_string(), "2".to_string()], 2, true);
/// ```
pub fn log_bulk_delete(
	user_id: &str,
	model_name: &str,
	record_ids: &[String],
	affected: u64,
	success: bool,
) {
	let entry = AuditEntry {
		timestamp: chrono::Utc::now().to_rfc3339(),
		user_id: user_id.to_string(),
		action: AuditAction::BulkDelete,
		model_name: model_name.to_string(),
		record_id: Some(record_ids.join(",")),
		changed_fields: None,
		success,
		affected_count: Some(affected),
	};

	emit_audit_log(&entry);
}

/// Emits an audit log entry via the tracing infrastructure.
///
/// Uses `info!` level for successful operations and `warn!` level for failures.
fn emit_audit_log(entry: &AuditEntry) {
	if entry.success {
		tracing::info!("{}", entry);
	} else {
		tracing::warn!("{}", entry);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ============================================================
	// AuditAction Display tests
	// ============================================================

	#[rstest]
	fn test_audit_action_create_display() {
		// Assert
		assert_eq!(AuditAction::Create.to_string(), "CREATE");
	}

	#[rstest]
	fn test_audit_action_update_display() {
		// Assert
		assert_eq!(AuditAction::Update.to_string(), "UPDATE");
	}

	#[rstest]
	fn test_audit_action_delete_display() {
		// Assert
		assert_eq!(AuditAction::Delete.to_string(), "DELETE");
	}

	#[rstest]
	fn test_audit_action_bulk_delete_display() {
		// Assert
		assert_eq!(AuditAction::BulkDelete.to_string(), "BULK_DELETE");
	}

	#[rstest]
	fn test_audit_action_export_display() {
		// Assert
		assert_eq!(AuditAction::Export.to_string(), "EXPORT");
	}

	#[rstest]
	fn test_audit_action_import_display() {
		// Assert
		assert_eq!(AuditAction::Import.to_string(), "IMPORT");
	}

	// ============================================================
	// AuditEntry Display tests
	// ============================================================

	#[rstest]
	fn test_audit_entry_display_create() {
		// Arrange
		let entry = AuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user-42".to_string(),
			action: AuditAction::Create,
			model_name: "User".to_string(),
			record_id: None,
			changed_fields: Some(vec!["name".to_string(), "email".to_string()]),
			success: true,
			affected_count: Some(1),
		};

		// Act
		let output = entry.to_string();

		// Assert
		assert!(output.contains("[ADMIN_AUDIT]"));
		assert!(output.contains("user=user-42"));
		assert!(output.contains("action=CREATE"));
		assert!(output.contains("model=User"));
		assert!(output.contains("changed_fields=[name, email]"));
		assert!(output.contains("success=true"));
	}

	#[rstest]
	fn test_audit_entry_display_delete() {
		// Arrange
		let entry = AuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "admin-1".to_string(),
			action: AuditAction::Delete,
			model_name: "Post".to_string(),
			record_id: Some("123".to_string()),
			changed_fields: None,
			success: true,
			affected_count: Some(1),
		};

		// Act
		let output = entry.to_string();

		// Assert
		assert!(output.contains("action=DELETE"));
		assert!(output.contains("model=Post"));
		assert!(output.contains("record_id=123"));
		assert!(output.contains("affected=1"));
	}

	#[rstest]
	fn test_audit_entry_display_bulk_delete() {
		// Arrange
		let entry = AuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "admin-1".to_string(),
			action: AuditAction::BulkDelete,
			model_name: "Comment".to_string(),
			record_id: Some("1,2,3".to_string()),
			changed_fields: None,
			success: true,
			affected_count: Some(3),
		};

		// Act
		let output = entry.to_string();

		// Assert
		assert!(output.contains("action=BULK_DELETE"));
		assert!(output.contains("record_id=1,2,3"));
		assert!(output.contains("affected=3"));
	}

	#[rstest]
	fn test_audit_entry_display_failed_operation() {
		// Arrange
		let entry = AuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user-99".to_string(),
			action: AuditAction::Update,
			model_name: "User".to_string(),
			record_id: Some("456".to_string()),
			changed_fields: Some(vec!["password".to_string()]),
			success: false,
			affected_count: None,
		};

		// Act
		let output = entry.to_string();

		// Assert
		assert!(output.contains("success=false"));
		assert!(output.contains("action=UPDATE"));
	}

	// ============================================================
	// Log function tests (verify entry construction)
	// ============================================================

	#[rstest]
	fn test_log_create_constructs_correct_entry() {
		// Arrange
		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("Alice"));
		data.insert("email".to_string(), serde_json::json!("alice@example.com"));

		// Act - just verify no panic; logging goes to the log infrastructure
		log_create("user-42", "User", &data, true);
	}

	#[rstest]
	fn test_log_update_constructs_correct_entry() {
		// Arrange
		let mut data = HashMap::new();
		data.insert("email".to_string(), serde_json::json!("new@example.com"));

		// Act
		log_update("user-42", "User", "123", &data, true);
	}

	#[rstest]
	fn test_log_delete_constructs_correct_entry() {
		// Act
		log_delete("user-42", "User", "123", true);
	}

	#[rstest]
	fn test_log_bulk_delete_constructs_correct_entry() {
		// Arrange
		let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];

		// Act
		log_bulk_delete("user-42", "User", &ids, 3, true);
	}

	#[rstest]
	fn test_log_create_with_failure() {
		// Arrange
		let data = HashMap::new();

		// Act
		log_create("user-42", "User", &data, false);
	}

	// ============================================================
	// AuditAction equality tests
	// ============================================================

	#[rstest]
	fn test_audit_action_equality() {
		// Assert
		assert_eq!(AuditAction::Create, AuditAction::Create);
		assert_ne!(AuditAction::Create, AuditAction::Delete);
	}

	#[rstest]
	fn test_audit_action_clone() {
		// Arrange
		let action = AuditAction::Update;

		// Act
		let cloned = action;

		// Assert
		assert_eq!(action, cloned);
	}
}
