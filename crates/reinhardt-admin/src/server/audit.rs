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

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::DEFAULT_PAGE_SIZE;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::database::canonicalize_pk_value;
#[cfg(server)]
use crate::core::history::{NewHistoryEvent, count_object_history, list_object_history};
#[cfg(server)]
use crate::core::inline::{InlineSaveOperation, InlineSaveOutcome};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminQuery, AdminRequestContext, AdminSiteKey};
use crate::types::HistoryResponse;
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

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

#[cfg(server)]
pub(crate) fn new_history_event(
	actor: &str,
	action_name: &str,
	model_name: &str,
	table_name: &str,
	object_id: &str,
	changed_fields: Vec<String>,
	affected_count: u64,
) -> NewHistoryEvent {
	NewHistoryEvent {
		occurred_at: chrono::Utc::now(),
		actor: actor.to_string(),
		action_name: action_name.to_string(),
		model_name: model_name.to_string(),
		table_name: table_name.to_string(),
		object_id: object_id.to_string(),
		object_repr: format!("{model_name} ({object_id})"),
		changed_fields,
		affected_count,
		success: true,
	}
}

/// Get a stable, paginated history for one admin object.
///
/// The lookup checks model view permission and the request-aware object scope
/// before filtering the persistent history table by the canonical registered
/// model name, table name, and exact object ID. The current object must remain
/// visible through the model admin queryset. History remains persisted after
/// deletion, but deleted objects are no longer available through this endpoint.
#[server_fn]
pub async fn get_history(
	model_name: String,
	object_id: String,
	page: u64,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<HistoryResponse, ServerFnError> {
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let admin_query = model_admin
		.get_queryset(
			user.as_ref(),
			&request_context,
			AdminQuery::new(model_admin.table_name()),
		)
		.await
		.map_server_fn_error()?;

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let object_id = canonicalize_pk_value(&table_name, model_admin.pk_field(), &object_id);
	if db
		.get_admin_query(&admin_query, model_admin.pk_field(), &object_id)
		.await
		.map_server_fn_error()?
		.is_none()
	{
		return Err(ServerFnError::server(404, "Object not found"));
	}
	let page = page.max(1);
	let page_size = DEFAULT_PAGE_SIZE;
	let offset = (page - 1)
		.checked_mul(page_size)
		.filter(|offset| *offset <= i64::MAX as u64)
		.ok_or_else(|| ServerFnError::application("History page is too large"))?;
	let mut connection = *db.connection();
	let count = count_object_history(&mut connection, &model_name, &table_name, &object_id)
		.await
		.map_err(|_| ServerFnError::server(500, "History query failed"))?;
	let results = list_object_history(
		&mut connection,
		&model_name,
		&table_name,
		&object_id,
		offset,
		page_size,
	)
	.await
	.map_err(|_| ServerFnError::server(500, "History query failed"))?
	.into_iter()
	.map(|event| crate::types::AdminHistoryEntry {
		id: event.id,
		actor: event.actor,
		timestamp: event
			.occurred_at
			.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
		action_name: event.action_name,
		model_name: event.model_name,
		object_id: event.object_id,
		object_repr: event.object_repr,
		changed_fields: event.changed_fields,
		affected_count: event.affected_count,
		success: event.success,
	})
	.collect();
	let total_pages = count.div_ceil(page_size).max(1);

	Ok(HistoryResponse {
		model_name,
		object_id,
		count,
		page,
		page_size,
		total_pages,
		results,
	})
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

/// Logs child mutations after their parent transaction commits.
#[cfg(server)]
fn inline_audit_model_name(site: &AdminSite, outcome: &InlineSaveOutcome) -> String {
	site.get_model_admin_by_table_name(&outcome.table_name)
		.map(|admin| admin.model_name().to_owned())
		.unwrap_or_else(|_| outcome.model_identity.clone())
}

#[cfg(server)]
pub(crate) fn log_inline_outcomes(site: &AdminSite, user_id: &str, outcomes: &[InlineSaveOutcome]) {
	for outcome in outcomes {
		let action = match outcome.operation {
			InlineSaveOperation::Create => AuditAction::Create,
			InlineSaveOperation::Update => AuditAction::Update,
			InlineSaveOperation::Delete => AuditAction::Delete,
		};
		let entry = AuditEntry {
			timestamp: chrono::Utc::now().to_rfc3339(),
			user_id: user_id.to_owned(),
			action,
			model_name: inline_audit_model_name(site, outcome),
			record_id: Some(outcome.object_id.clone()),
			changed_fields: (outcome.operation != InlineSaveOperation::Delete)
				.then(|| outcome.changed_fields.clone()),
			success: true,
			affected_count: Some(1),
		};
		emit_audit_log(&entry);
	}
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
		record_id: Some(
			serde_json::to_string(&record_ids).unwrap_or_else(|_| record_ids.join(",")),
		),
		changed_fields: None,
		success,
		affected_count: Some(affected),
	};

	emit_audit_log(&entry);
}

/// Logs a registered action executed against selected records.
#[cfg(server)]
pub(crate) fn log_action(
	user_id: &str,
	model_name: &str,
	record_ids: &[String],
	action_name: &str,
	affected: u64,
	success: bool,
) {
	let entry = action_entry(
		user_id,
		model_name,
		record_ids,
		action_name,
		affected,
		success,
	);

	emit_action_audit_log(&entry);
}

#[cfg(server)]
fn action_entry(
	user_id: &str,
	model_name: &str,
	record_ids: &[String],
	action_name: &str,
	affected: u64,
	success: bool,
) -> ActionAuditEntry {
	ActionAuditEntry {
		timestamp: chrono::Utc::now().to_rfc3339(),
		user_id: user_id.to_string(),
		model_name: model_name.to_string(),
		record_ids: record_ids.to_vec(),
		action_name: action_name.to_string(),
		success,
		affected_count: affected,
	}
}

#[cfg(server)]
#[derive(Debug, Clone)]
struct ActionAuditEntry {
	timestamp: String,
	user_id: String,
	model_name: String,
	record_ids: Vec<String>,
	action_name: String,
	affected_count: u64,
	success: bool,
}

#[cfg(server)]
impl fmt::Display for ActionAuditEntry {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let record_ids = self
			.record_ids
			.iter()
			.map(|id| format!("\"{}\"", id.escape_default()))
			.collect::<Vec<_>>()
			.join(",");
		write!(
			f,
			"[ADMIN_AUDIT] {} user=\"{}\" action=ACTION model=\"{}\" record_id=[{}] affected={} action_name=\"{}\" success={}",
			self.timestamp.escape_default(),
			self.user_id.escape_default(),
			self.model_name.escape_default(),
			record_ids,
			self.affected_count,
			self.action_name.escape_default(),
			self.success,
		)
	}
}

/// Emits an audit log entry via the tracing infrastructure.
///
/// Uses `info!` level for successful operations and `warn!` level for failures.
#[cfg(server)]
fn emit_audit_log(entry: &AuditEntry) {
	if entry.success {
		tracing::info!("{}", entry);
	} else {
		tracing::warn!("{}", entry);
	}
}

#[cfg(server)]
fn emit_action_audit_log(entry: &ActionAuditEntry) {
	if entry.success {
		tracing::info!("{}", entry);
	} else {
		tracing::warn!("{}", entry);
	}
}

/// No-op audit log on WASM targets (tracing is server-only).
#[cfg(client)]
fn emit_audit_log(_entry: &AuditEntry) {}

#[cfg(all(test, server))]
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

	#[rstest]
	fn test_public_audit_types_keep_the_original_exhaustive_shape() {
		let entry = AuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user-42".to_string(),
			action: AuditAction::Create,
			model_name: "Article".to_string(),
			record_id: None,
			changed_fields: None,
			success: true,
			affected_count: Some(1),
		};

		let action = match entry.action {
			AuditAction::Create => "CREATE",
			AuditAction::Update => "UPDATE",
			AuditAction::Delete => "DELETE",
			AuditAction::BulkDelete => "BULK_DELETE",
			AuditAction::Export => "EXPORT",
			AuditAction::Import => "IMPORT",
		};

		assert_eq!(action, "CREATE");
	}

	#[rstest]
	fn inline_audit_uses_the_registered_child_model_name() {
		let site = AdminSite::new("Admin");
		site.register(
			"line-item-route",
			crate::core::ModelAdminConfig::builder()
				.model_name("LineItem")
				.table_name("line_items")
				.build()
				.expect("child admin should build"),
		)
		.expect("child admin should register");
		let outcome = InlineSaveOutcome {
			inline_key: "line-items".to_owned(),
			submitted_index: 0,
			operation: InlineSaveOperation::Create,
			model_identity: "Line Item".to_owned(),
			table_name: "line_items".to_owned(),
			object_id: "1".to_owned(),
			changed_fields: vec!["name".to_owned()],
			previous_values: HashMap::new(),
		};

		assert_eq!(inline_audit_model_name(&site, &outcome), "LineItem");
	}

	#[rstest]
	fn test_audit_entry_display_action_preserves_zero_and_escapes_name() {
		let entry = ActionAuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user-42".to_string(),
			model_name: "CanonicalModel".to_string(),
			record_ids: vec!["1".to_string()],
			action_name: "publish\nnow".to_string(),
			success: true,
			affected_count: 0,
		};

		assert_eq!(
			entry.to_string(),
			"[ADMIN_AUDIT] 2024-01-01T00:00:00Z user=\"user-42\" action=ACTION model=\"CanonicalModel\" record_id=[\"1\"] affected=0 action_name=\"publish\\nnow\" success=true"
		);
	}

	#[rstest]
	fn test_audit_entry_display_failed_action_includes_registered_name() {
		let entry = ActionAuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user-42".to_string(),
			model_name: "CanonicalModel".to_string(),
			record_ids: vec!["1".to_string()],
			action_name: "publish".to_string(),
			success: false,
			affected_count: 0,
		};

		assert_eq!(
			entry.to_string(),
			"[ADMIN_AUDIT] 2024-01-01T00:00:00Z user=\"user-42\" action=ACTION model=\"CanonicalModel\" record_id=[\"1\"] affected=0 action_name=\"publish\" success=false"
		);
	}

	#[rstest]
	fn test_action_audit_boundary_preserves_canonical_dispatch_values() {
		let successful_ids = vec!["7".to_string(), "11".to_string()];

		let entry = action_entry(
			"user-42",
			"CanonicalActionModel",
			&successful_ids,
			"publish",
			3,
			true,
		);

		assert_eq!(entry.model_name, "CanonicalActionModel");
		assert_eq!(entry.record_ids, successful_ids);
		assert_eq!(entry.action_name, "publish");
		assert_eq!(entry.affected_count, 3);
		assert!(entry.success);
	}

	#[rstest]
	fn test_action_audit_boundary_escapes_untrusted_log_fields() {
		let entry = ActionAuditEntry {
			timestamp: "2024-01-01T00:00:00Z".to_string(),
			user_id: "user\n42".to_string(),
			model_name: "Unknown\rModel success=true".to_string(),
			record_ids: vec!["1\u{0085}2".to_string(), "3\u{2028}4".to_string()],
			action_name: "publish\tnow success=true".to_string(),
			affected_count: 0,
			success: false,
		};

		assert_eq!(
			entry.to_string(),
			"[ADMIN_AUDIT] 2024-01-01T00:00:00Z user=\"user\\n42\" action=ACTION model=\"Unknown\\rModel success=true\" record_id=[\"1\\u{85}2\",\"3\\u{2028}4\"] affected=0 action_name=\"publish\\tnow success=true\" success=false"
		);
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
			record_id: Some("[\"1\",\"2\",\"3\"]".to_string()),
			changed_fields: None,
			success: true,
			affected_count: Some(3),
		};

		// Act
		let output = entry.to_string();

		// Assert
		assert!(output.contains("action=BULK_DELETE"));
		assert!(output.contains("record_id=[\"1\",\"2\",\"3\"]"));
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

		// Act - construct the AuditEntry the same way log_bulk_delete does
		// to verify the JSON array format used for record_id
		let entry = AuditEntry {
			timestamp: chrono::Utc::now().to_rfc3339(),
			user_id: "user-42".to_string(),
			action: AuditAction::BulkDelete,
			model_name: "User".to_string(),
			record_id: Some(serde_json::to_string(&ids).unwrap_or_else(|_| ids.join(","))),
			changed_fields: None,
			success: true,
			affected_count: Some(3),
		};

		// Assert
		assert_eq!(entry.record_id, Some("[\"1\",\"2\",\"3\"]".to_string()));
		assert_eq!(entry.action, AuditAction::BulkDelete);
		assert!(entry.success);
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

	#[rstest]
	fn persistent_history_event_is_privacy_safe_and_deterministic() {
		// Arrange
		let changed_fields = vec![
			"status".to_string(),
			"email".to_string(),
			"status".to_string(),
		];

		// Act
		let event = new_history_event(
			"staff-7",
			"UPDATE",
			"accounts.User",
			"accounts_users",
			"42",
			changed_fields,
			1,
		);

		// Assert
		assert_eq!(event.actor, "staff-7");
		assert_eq!(event.action_name, "UPDATE");
		assert_eq!(event.model_name, "accounts.User");
		assert_eq!(event.table_name, "accounts_users");
		assert_eq!(event.object_id, "42");
		assert_eq!(event.object_repr, "accounts.User (42)");
		assert_eq!(event.changed_fields, ["status", "email", "status"]);
		assert_eq!(event.affected_count, 1);
		assert!(event.success);
	}
}
