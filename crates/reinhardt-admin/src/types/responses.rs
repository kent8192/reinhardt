//! Response types for admin panel API

use crate::types::models::{ColumnInfo, Fieldset, FilterInfo, ModelInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response for dashboard endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardResponse {
	/// Site name
	pub site_name: String,
	/// Header text shown in admin navigation bar
	pub site_header: String,
	/// URL prefix
	pub url_prefix: String,
	/// Login page URL for authentication redirects
	pub login_url: String,
	/// Logout page URL for sign-out redirects
	pub logout_url: String,
	/// Registered models with their metadata
	pub models: Vec<ModelInfo>,
	/// CSRF token for mutation requests (POST, PUT, DELETE)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub csrf_token: Option<String>,
}

/// Response for list endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
	/// Model name
	pub model_name: String,
	/// Total count of items
	pub count: u64,
	/// Current page
	pub page: u64,
	/// Items per page
	pub page_size: u64,
	/// Total pages
	pub total_pages: u64,
	/// Items on this page
	pub results: Vec<HashMap<String, serde_json::Value>>,
	/// Available filters metadata (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub available_filters: Option<Vec<FilterInfo>>,
	/// Column definitions for list display
	#[serde(skip_serializing_if = "Option::is_none")]
	pub columns: Option<Vec<ColumnInfo>>,
}

/// Response for detail endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailResponse {
	/// Model name
	pub model_name: String,
	/// Item data
	pub data: HashMap<String, serde_json::Value>,
}

/// A persistent admin change-history entry for one object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminHistoryEntry {
	/// Monotonic audit event identifier
	pub id: i64,
	/// Identifier of the user who performed the operation
	pub actor: String,
	/// UTC RFC 3339 timestamp for the operation
	pub timestamp: String,
	/// Name of the action that was performed
	pub action_name: String,
	/// Canonical registered model name
	pub model_name: String,
	/// Primary key of the affected object
	pub object_id: String,
	/// Privacy-safe representation of the affected object
	pub object_repr: String,
	/// Names of fields changed by the operation
	pub changed_fields: Vec<String>,
	/// Number of objects affected by the operation
	pub affected_count: u64,
	/// Whether the operation succeeded
	pub success: bool,
}

/// Paginated change history for one admin object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResponse {
	/// Canonical registered model name
	pub model_name: String,
	/// Primary key of the object whose history was requested
	pub object_id: String,
	/// Total number of matching history entries
	pub count: u64,
	/// Current one-indexed page
	pub page: u64,
	/// Entries per page
	pub page_size: u64,
	/// Total number of pages
	pub total_pages: u64,
	/// History entries on this page
	pub results: Vec<AdminHistoryEntry>,
}

/// Response for create/update/delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResponse {
	/// Success status
	pub success: bool,
	/// Message
	pub message: String,
	/// Affected rows (for update/delete)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub affected: Option<u64>,
	/// Created/Updated data (for create/update)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<HashMap<String, serde_json::Value>>,
}

/// Response for bulk delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDeleteResponse {
	/// Success status
	pub success: bool,
	/// Number of deleted items
	pub deleted: u64,
	/// Message
	pub message: String,
}

/// Response for import endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResponse {
	/// Success status
	pub success: bool,
	/// Number of imported records
	pub imported: u64,
	/// Number of updated records
	pub updated: u64,
	/// Number of skipped records
	pub skipped: u64,
	/// Number of failed records
	pub failed: u64,
	/// Summary message
	pub message: String,
	/// Error messages (if any)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub errors: Option<Vec<String>>,
}

/// Response for export endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportResponse {
	/// Exported data (binary)
	#[serde(with = "serde_bytes")]
	pub data: Vec<u8>,
	/// Filename for download
	pub filename: String,
	/// Content type (e.g., "application/json", "text/csv")
	pub content_type: String,
	/// Whether the export was truncated due to exceeding the maximum record limit
	pub truncated: bool,
	/// Total number of records in the table (before truncation)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub total_count: Option<u64>,
}

/// Response for admin login endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
	/// JWT token for subsequent authenticated requests
	pub token: String,
	/// Authenticated username
	pub username: String,
	/// User's primary key as string
	pub user_id: String,
	/// Whether the user is staff
	pub is_staff: bool,
	/// Whether the user is a superuser
	pub is_superuser: bool,
}

/// Response for fields endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldsResponse {
	/// Model name
	pub model_name: String,
	/// Field definitions for dynamic form generation
	pub fields: Vec<crate::types::models::FieldInfo>,
	/// Optional fieldset layout for the form.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub fieldsets: Option<Vec<Fieldset>>,
	/// Existing field values (for edit forms)
	/// None for create forms, Some(values) for edit forms
	#[serde(skip_serializing_if = "Option::is_none")]
	pub values: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn history_response_serde_shape_round_trips_without_raw_values() {
		// Arrange
		let response = HistoryResponse {
			model_name: "accounts.User".to_string(),
			object_id: "42".to_string(),
			count: 1,
			page: 1,
			page_size: 25,
			total_pages: 1,
			results: vec![AdminHistoryEntry {
				id: 7,
				actor: "admin-1".to_string(),
				timestamp: "2026-08-09T12:34:56.123456Z".to_string(),
				action_name: "UPDATE".to_string(),
				model_name: "accounts.User".to_string(),
				object_id: "42".to_string(),
				object_repr: "accounts.User (42)".to_string(),
				changed_fields: vec!["email".to_string()],
				affected_count: 1,
				success: true,
			}],
		};

		// Act
		let value = serde_json::to_value(&response).expect("history response must serialize");
		let decoded: HistoryResponse =
			serde_json::from_value(value.clone()).expect("history response must deserialize");

		// Assert
		assert_eq!(
			value,
			serde_json::json!({
				"model_name": "accounts.User",
				"object_id": "42",
				"count": 1,
				"page": 1,
				"page_size": 25,
				"total_pages": 1,
				"results": [{
					"id": 7,
					"actor": "admin-1",
					"timestamp": "2026-08-09T12:34:56.123456Z",
					"action_name": "UPDATE",
					"model_name": "accounts.User",
					"object_id": "42",
					"object_repr": "accounts.User (42)",
					"changed_fields": ["email"],
					"affected_count": 1,
					"success": true
				}]
			})
		);
		assert_eq!(decoded.results[0].changed_fields, ["email"]);
	}
}
