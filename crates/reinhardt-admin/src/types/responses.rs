//! Response types for admin panel API

use crate::types::models::{
	AdminAction, ColumnInfo, DateHierarchyInfo, Fieldset, FilterInfo, InlineFormInfo, ModelInfo,
	PrepopulatedField, RelationOption,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_pk_field() -> String {
	"id".to_string()
}

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
	/// Primary key field for row detail and mutation operations.
	#[serde(default = "default_pk_field")]
	pub pk_field: String,
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

/// Response for the versioned date-hierarchy list endpoint.
///
/// The legacy [`ListResponse`] remains unchanged; this wrapper adds hierarchy
/// metadata without breaking existing Rust struct literals and consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateHierarchyListResponse {
	/// Legacy list response fields.
	#[serde(flatten)]
	pub response: ListResponse,
	/// Date hierarchy metadata for changelist drill-down.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub date_hierarchy: Option<DateHierarchyInfo>,
}

impl From<DateHierarchyListResponse> for ListResponse {
	fn from(response: DateHierarchyListResponse) -> Self {
		response.response
	}
}

/// Bounded related-object lookup response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationLookupResponse {
	/// Related options on the requested page.
	pub results: Vec<RelationOption>,
	/// Normalized one-indexed page number.
	pub page: u64,
	/// Whether another result page is available.
	pub has_next: bool,
}

/// Response for list action metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListActionMetadataResponse {
	/// Configured primary key field name.
	pub pk_field: String,
	/// Actions available for the model.
	pub actions: Vec<AdminAction>,
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
	/// Canonical registered model name.
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

/// Successfully committed changelist row update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineEditOutcome {
	/// Primary key value for the updated row.
	pub object_id: String,
	/// Fields written for the row, sorted by name.
	pub changed_fields: Vec<String>,
}

/// Row-local or request-wide changelist validation or lookup error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineEditError {
	/// Primary key value for the affected row, or empty for request-wide errors.
	pub object_id: String,
	/// Field associated with the error, if any.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub field: Option<String>,
	/// Stable user-facing error message.
	pub message: String,
}

/// Response from an atomic changelist inline-edit request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineEditResponse {
	/// Number of rows committed.
	pub updated: u64,
	/// Per-row outcomes returned only after commit.
	pub outcomes: Vec<InlineEditOutcome>,
	/// Validation or missing-row errors.
	pub errors: Vec<InlineEditError>,
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
	/// Related child forms, empty for existing parent-only admins.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub inlines: Vec<InlineFormInfo>,
	/// Client-side rules for deriving empty field values from other fields.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub prepopulated_fields: Vec<PrepopulatedField>,
	/// Existing field values (for edit forms)
	/// None for create forms, Some(values) for edit forms
	#[serde(skip_serializing_if = "Option::is_none")]
	pub values: Option<HashMap<String, serde_json::Value>>,
}

/// Paginated relation options for a many-to-many selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManyToManyLookupResponse {
	/// Relation options on this page.
	pub options: Vec<RelationOption>,
	/// Current page number.
	pub page: u64,
	/// Whether another page is available.
	pub has_more: bool,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::RelationOption;
	use rstest::rstest;
	use serde_json::json;

	#[test]
	fn date_hierarchy_response_keeps_legacy_fields_flat() {
		let response = DateHierarchyListResponse {
			response: ListResponse {
				model_name: "Article".to_string(),
				pk_field: "id".to_string(),
				count: 1,
				page: 1,
				page_size: 25,
				total_pages: 1,
				results: vec![],
				available_filters: None,
				columns: None,
			},
			date_hierarchy: None,
		};

		let value = serde_json::to_value(response).expect("list response should serialize");
		assert_eq!(value["model_name"], serde_json::json!("Article"));
		assert!(value.get("response").is_none());
	}

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

	#[rstest]
	fn fields_response_defaults_omitted_inlines_to_empty() {
		let response: FieldsResponse =
			serde_json::from_str(r#"{"model_name":"Parent","fields":[],"values":null}"#).unwrap();

		assert!(response.inlines.is_empty());
		assert!(response.prepopulated_fields.is_empty());
	}

	#[rstest]
	fn relation_lookup_response_round_trips() {
		// Arrange
		let response = RelationLookupResponse {
			results: vec![RelationOption {
				id: "9".to_string(),
				label: "Related object".to_string(),
			}],
			page: 3,
			has_next: true,
		};

		// Act
		let serialized =
			serde_json::to_value(&response).expect("relation lookup response should serialize");
		let deserialized: RelationLookupResponse = serde_json::from_value(serialized.clone())
			.expect("relation lookup response should deserialize");

		// Assert
		assert_eq!(
			serialized,
			json!({
				"results": [{"id": "9", "label": "Related object"}],
				"page": 3,
				"has_next": true
			})
		);
		assert_eq!(deserialized, response);
	}
}
