//! Request types for admin panel API

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

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
#[derive(Debug, Deserialize, Default)]
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
	/// Data to create/update
	#[serde(flatten)]
	pub data: HashMap<String, serde_json::Value>,
}

/// Request body for bulk delete
#[derive(Debug, Deserialize)]
pub struct BulkDeleteRequest {
	/// IDs to delete
	pub ids: Vec<String>,
}

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ExportFormat {
	#[default]
	Json,
	Csv,
	Tsv,
}
