//! Request types for admin panel API

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::types::DateHierarchySelection;

/// Maximum number of filter parameters allowed in a single request.
///
/// Prevents abuse through excessive filter parameters which could lead to
/// complex database queries or resource exhaustion.
const MAX_FILTER_COUNT: usize = 20;

/// Maximum length for a single filter key or value (in bytes).
///
/// Prevents excessively long filter strings from reaching the database layer.
const MAX_FILTER_STRING_LENGTH: usize = 500;

/// Query parameters for list endpoint.
///
/// Filter parameters are explicitly provided via the `filters` field rather than
/// captured via `serde(flatten)`, preventing unrecognized query parameters from
/// silently becoming database filters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListQueryParams {
	/// Page number (1-indexed)
	pub page: Option<u64>,
	/// Items per page
	pub page_size: Option<u64>,
	/// Search query
	pub search: Option<String>,
	/// Sort field (prefix with "-" for descending, e.g., "created_at" or "-created_at")
	pub sort_by: Option<String>,
	/// Filter field=value pairs.
	///
	/// Only explicitly provided filter parameters are accepted.
	/// Each filter key and value is validated for length constraints.
	#[serde(default, deserialize_with = "deserialize_validated_filters")]
	pub filters: HashMap<String, String>,
}

/// Query parameters for the date-hierarchy list endpoint.
///
/// This extension keeps [`ListQueryParams`] source-compatible for existing
/// callers while adding the optional hierarchy selection to a versioned
/// server function.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DateHierarchyListQueryParams {
	/// The legacy list query parameters.
	#[serde(flatten)]
	pub list: ListQueryParams,
	/// Current date hierarchy selection.
	pub date_hierarchy: Option<DateHierarchySelection>,
}

impl From<ListQueryParams> for DateHierarchyListQueryParams {
	fn from(list: ListQueryParams) -> Self {
		Self {
			list,
			date_hierarchy: None,
		}
	}
}

impl Deref for DateHierarchyListQueryParams {
	type Target = ListQueryParams;

	fn deref(&self) -> &Self::Target {
		&self.list
	}
}

impl DerefMut for DateHierarchyListQueryParams {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.list
	}
}

/// Relation lookup operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RelationLookupRequest {
	/// Search related objects using the related admin's search fields.
	Search {
		/// Search text.
		query: String,
		/// One-indexed result page.
		page: Option<u64>,
		/// Requested number of results per page.
		page_size: Option<u64>,
	},
	/// Resolve one exact related primary key.
	Resolve {
		/// Related object's primary key.
		id: String,
	},
}

/// Deserializes and validates filter parameters.
///
/// Enforces:
/// - Maximum number of filters (`MAX_FILTER_COUNT`)
/// - Maximum length for filter keys and values (`MAX_FILTER_STRING_LENGTH`)
/// - Filter keys must be non-empty and contain only alphanumeric characters, underscores, or hyphens
fn deserialize_validated_filters<'de, D>(
	deserializer: D,
) -> Result<HashMap<String, String>, D::Error>
where
	D: Deserializer<'de>,
{
	let filters: HashMap<String, String> = HashMap::deserialize(deserializer)?;

	if filters.len() > MAX_FILTER_COUNT {
		return Err(serde::de::Error::custom(format!(
			"too many filter parameters: {} (max {})",
			filters.len(),
			MAX_FILTER_COUNT
		)));
	}

	for (key, value) in &filters {
		if key.is_empty() {
			return Err(serde::de::Error::custom("filter key must not be empty"));
		}

		if key.len() > MAX_FILTER_STRING_LENGTH {
			return Err(serde::de::Error::custom(format!(
				"filter key '{}...' exceeds maximum length of {} bytes",
				&key[..32.min(key.len())],
				MAX_FILTER_STRING_LENGTH
			)));
		}

		if value.len() > MAX_FILTER_STRING_LENGTH {
			return Err(serde::de::Error::custom(format!(
				"filter value for '{}' exceeds maximum length of {} bytes",
				key, MAX_FILTER_STRING_LENGTH
			)));
		}

		// Validate filter key format: only alphanumeric, underscores, hyphens, and dots
		if !key
			.chars()
			.all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
		{
			return Err(serde::de::Error::custom(format!(
				"filter key '{}' contains invalid characters (allowed: alphanumeric, '_', '-', '.')",
				key
			)));
		}
	}

	Ok(filters)
}

/// Request body for create/update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRequest {
	/// CSRF token for mutation verification (double-submit cookie pattern).
	///
	/// The client must send the CSRF token received from the dashboard response
	/// in this field. The server validates this value against the `csrftoken`
	/// cookie set by the dashboard endpoint. An attacker on a different origin
	/// cannot read the cookie, preventing CSRF attacks.
	pub csrf_token: String,
	/// Data to create/update
	#[serde(flatten)]
	pub data: HashMap<String, serde_json::Value>,
}

/// Request body for atomic changelist inline edits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineEditRequest {
	/// CSRF token for mutation verification.
	pub csrf_token: String,
	/// Dirty row updates to apply atomically.
	pub updates: Vec<InlineEditMutation>,
}

/// Changed fields for one changelist row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineEditMutation {
	/// Primary key value for the row.
	pub object_id: String,
	/// Dirty fields and their new values.
	pub changes: HashMap<String, serde_json::Value>,
}

/// Request body for bulk delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDeleteRequest {
	/// CSRF token for mutation verification (double-submit cookie pattern).
	///
	/// The client must send the CSRF token received from the dashboard response
	/// in this field. The server validates this value against the `csrftoken`
	/// cookie set by the dashboard endpoint. An attacker on a different origin
	/// cannot read the cookie, preventing CSRF attacks.
	pub csrf_token: String,
	/// IDs to delete
	pub ids: Vec<String>,
}

/// Request body for executing an admin action on selected records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminActionRequest {
	/// CSRF token for mutation verification.
	pub csrf_token: String,
	/// Machine-readable action name.
	pub action: String,
	/// IDs of the records to process.
	pub ids: Vec<String>,
}

impl AdminActionRequest {
	/// Creates an admin action request.
	pub fn new(csrf_token: impl Into<String>, action: impl Into<String>, ids: Vec<String>) -> Self {
		Self {
			csrf_token: csrf_token.into(),
			action: action.into(),
			ids,
		}
	}
}

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ExportFormat {
	/// JSON format (default).
	#[default]
	Json,
	/// Comma-separated values format.
	Csv,
	/// Tab-separated values format.
	Tsv,
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json;

	// Helper to deserialize ListQueryParams from JSON
	fn parse_list_query(json: &str) -> Result<ListQueryParams, serde_json::Error> {
		serde_json::from_str(json)
	}

	#[rstest]
	#[case(
		RelationLookupRequest::Search {
			query: "ada".to_string(),
			page: Some(2),
			page_size: Some(25),
		},
		serde_json::json!({
			"mode": "search",
			"query": "ada",
			"page": 2,
			"page_size": 25
		})
	)]
	#[case(
		RelationLookupRequest::Resolve {
			id: "42".to_string(),
		},
		serde_json::json!({"mode": "resolve", "id": "42"})
	)]
	fn relation_lookup_request_round_trips(
		#[case] request: RelationLookupRequest,
		#[case] expected: serde_json::Value,
	) {
		// Act
		let serialized =
			serde_json::to_value(&request).expect("relation lookup request should serialize");
		let deserialized: RelationLookupRequest = serde_json::from_value(serialized.clone())
			.expect("relation lookup request should deserialize");

		// Assert
		assert_eq!(serialized, expected);
		assert_eq!(deserialized, request);
	}

	// ==================== Filter count validation ====================

	#[rstest]
	fn test_filters_within_limit_accepted() {
		// Arrange: 5 filters (well within limit of 20)
		let json = r#"{"filters": {"a": "1", "b": "2", "c": "3", "d": "4", "e": "5"}}"#;

		// Act
		let result = parse_list_query(json);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap().filters.len(), 5);
	}

	#[rstest]
	fn test_filters_at_exact_limit_accepted() {
		// Arrange: Exactly 20 filters
		let mut filters = serde_json::Map::new();
		for i in 0..20 {
			filters.insert(
				format!("field_{}", i),
				serde_json::Value::String(format!("value_{}", i)),
			);
		}
		let json = serde_json::json!({"filters": filters}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap().filters.len(), 20);
	}

	#[rstest]
	fn test_filters_exceeding_max_count_rejected() {
		// Arrange: 21 filters (exceeds limit of 20)
		let mut filters = serde_json::Map::new();
		for i in 0..21 {
			filters.insert(
				format!("field_{}", i),
				serde_json::Value::String(format!("value_{}", i)),
			);
		}
		let json = serde_json::json!({"filters": filters}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("too many filter parameters"),
			"Error should mention filter count limit: {}",
			err
		);
	}

	// ==================== Filter key/value length validation ====================

	#[rstest]
	fn test_filter_key_exceeding_max_length_rejected() {
		// Arrange: Key of 501 bytes
		let long_key = "a".repeat(501);
		let json = serde_json::json!({"filters": {long_key: "value"}}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("exceeds maximum length"),
			"Error should mention length limit: {}",
			err
		);
	}

	#[rstest]
	fn test_filter_value_exceeding_max_length_rejected() {
		// Arrange: Value of 501 bytes
		let long_value = "v".repeat(501);
		let json = serde_json::json!({"filters": {"field": long_value}}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("exceeds maximum length"),
			"Error should mention length limit: {}",
			err
		);
	}

	// ==================== Filter key format validation ====================

	#[rstest]
	fn test_empty_filter_key_rejected() {
		// Arrange: Empty key
		let json = r#"{"filters": {"": "value"}}"#;

		// Act
		let result = parse_list_query(json);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("must not be empty"),
			"Error should mention empty key: {}",
			err
		);
	}

	#[rstest]
	#[case("field_name", true)] // underscore allowed
	#[case("field-name", true)] // hyphen allowed
	#[case("field.name", true)] // period allowed
	#[case("fieldName123", true)] // alphanumeric allowed
	fn test_filter_key_with_valid_chars_accepted(#[case] key: &str, #[case] _expected_valid: bool) {
		// Arrange
		let json = serde_json::json!({"filters": {key: "value"}}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert!(result.is_ok(), "Key '{}' should be accepted", key);
	}

	#[rstest]
	fn test_filter_key_with_invalid_chars_rejected() {
		// Arrange: Key with semicolon (potential SQL injection vector)
		let json = r#"{"filters": {"field;DROP TABLE users": "value"}}"#;

		// Act
		let result = parse_list_query(json);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(
			err.contains("invalid character"),
			"Error should mention invalid character: {}",
			err
		);
	}

	// ==================== Default behavior ====================

	#[rstest]
	fn test_empty_filters_accepted() {
		// Arrange
		let json = r#"{"filters": {}}"#;

		// Act
		let result = parse_list_query(json);

		// Assert
		assert!(result.is_ok());
		assert!(result.unwrap().filters.is_empty());
	}

	#[rstest]
	fn test_missing_filters_uses_default() {
		// Arrange: No filters field at all
		let json = r#"{}"#;

		// Act
		let result = parse_list_query(json);

		// Assert
		assert!(result.is_ok());
		assert!(result.unwrap().filters.is_empty());
	}

	// ==================== Boundary value: filter count ====================

	#[rstest]
	#[case::zero_filters(0, true)]
	#[case::nineteen_filters(19, true)]
	#[case::twenty_filters(20, true)]
	#[case::twentyone_filters(21, false)]
	fn test_filter_count_boundary(#[case] count: usize, #[case] should_pass: bool) {
		// Arrange
		let mut filters = serde_json::Map::new();
		for i in 0..count {
			filters.insert(
				format!("field_{}", i),
				serde_json::Value::String(format!("value_{}", i)),
			);
		}
		let json = serde_json::json!({"filters": filters}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"count={}, expected pass={}, got {:?}",
			count,
			should_pass,
			result
		);
	}

	// ==================== Boundary value: filter key length ====================

	#[rstest]
	#[case::short_key(10, true)]
	#[case::at_limit(500, true)]
	#[case::above_limit(501, false)]
	fn test_filter_key_length_boundary(#[case] length: usize, #[case] should_pass: bool) {
		// Arrange: key composed of alphanumeric chars only
		let key: String = "a".repeat(length);
		let json = serde_json::json!({"filters": {key: "value"}}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"key_length={}, expected pass={}, got {:?}",
			length,
			should_pass,
			result
		);
	}

	// ==================== Equivalence partitioning: filter key format ====================

	#[rstest]
	#[case::alphanumeric("status", true)]
	#[case::with_underscore("created_at", true)]
	#[case::with_hyphen("is-active", true)]
	#[case::with_dot("user.name", true)]
	#[case::with_semicolon("status;DROP", false)]
	#[case::with_space("some field", false)]
	#[case::with_quotes("field\"name", false)]
	fn test_filter_key_format_equivalence(#[case] key: &str, #[case] should_pass: bool) {
		// Arrange
		let mut filters = HashMap::new();
		filters.insert(key.to_string(), "value".to_string());
		let json = serde_json::json!({"filters": filters}).to_string();

		// Act
		let result = parse_list_query(&json);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"key='{}', expected pass={}, got {:?}",
			key,
			should_pass,
			result
		);
	}

	#[test]
	fn date_hierarchy_query_serializes_as_flattened_list_request() {
		let params = DateHierarchyListQueryParams {
			list: ListQueryParams {
				page: Some(2),
				..Default::default()
			},
			date_hierarchy: Some(DateHierarchySelection {
				year: Some(2024),
				month: Some(2),
				day: None,
			}),
		};

		let value = serde_json::to_value(params).expect("date hierarchy request should serialize");
		assert_eq!(value["page"], serde_json::json!(2));
		assert_eq!(value["date_hierarchy"]["year"], serde_json::json!(2024));
		assert_eq!(value["date_hierarchy"]["month"], serde_json::json!(2));
		assert!(value.get("list").is_none());
	}
}
