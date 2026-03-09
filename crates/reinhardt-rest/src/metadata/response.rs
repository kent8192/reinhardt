//! Metadata response structures

use super::fields::FieldInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Action metadata (for POST, PUT, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetadata {
	/// The HTTP method for this action (e.g., `"POST"`, `"PUT"`).
	pub method: String,
	/// Field metadata keyed by field name.
	pub fields: HashMap<String, FieldInfo>,
}

/// Complete metadata response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataResponse {
	/// The display name of the view.
	pub name: String,
	/// A description of the view.
	pub description: String,
	/// Content types the view can render.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub renders: Option<Vec<String>>,
	/// Content types the view can parse.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub parses: Option<Vec<String>>,
	/// Action metadata keyed by HTTP method, containing field info.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub actions: Option<HashMap<String, HashMap<String, FieldInfo>>>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_metadata_serialization() {
		let response = MetadataResponse {
			name: "Test View".to_string(),
			description: "Test description".to_string(),
			renders: Some(vec!["application/json".to_string()]),
			parses: Some(vec!["application/json".to_string()]),
			actions: None,
		};

		let json = serde_json::to_string(&response).unwrap();
		assert!(json.contains("Test View"));
		assert!(json.contains("application/json"));
	}
}
