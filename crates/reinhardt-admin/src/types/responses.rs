//! Response types for admin panel API

use crate::types::models::{ColumnInfo, FilterInfo, ModelInfo};
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
	/// Existing field values (for edit forms)
	/// None for create forms, Some(values) for edit forms
	#[serde(skip_serializing_if = "Option::is_none")]
	pub values: Option<HashMap<String, serde_json::Value>>,
}
