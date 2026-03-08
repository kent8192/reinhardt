//! Browsable response types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response for browsable API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowsableResponse {
	/// The JSON response body.
	pub data: serde_json::Value,
	/// Metadata about the response (status, method, path, headers).
	pub metadata: ResponseMetadata,
}

/// Metadata associated with a browsable API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
	/// The HTTP status code.
	pub status: u16,
	/// The HTTP method used for the request.
	pub method: String,
	/// The request path.
	pub path: String,
	/// Response headers as key-value pairs.
	pub headers: HashMap<String, String>,
}

impl BrowsableResponse {
	/// Create a new BrowsableResponse
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::browsable_api::response::{BrowsableResponse, ResponseMetadata};
	/// use std::collections::HashMap;
	///
	/// let metadata = ResponseMetadata {
	///     status: 200,
	///     method: "GET".to_string(),
	///     path: "/api/test".to_string(),
	///     headers: HashMap::new(),
	/// };
	/// let response = BrowsableResponse::new(serde_json::json!({}), metadata);
	/// ```
	pub fn new(data: serde_json::Value, metadata: ResponseMetadata) -> Self {
		Self { data, metadata }
	}
	/// Create a successful response (200 OK)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::browsable_api::response::BrowsableResponse;
	///
	/// let response = BrowsableResponse::success(
	///     serde_json::json!({"message": "ok"}),
	///     "GET".to_string(),
	///     "/api/test".to_string()
	/// );
	/// ```
	pub fn success(data: serde_json::Value, method: String, path: String) -> Self {
		Self {
			data,
			metadata: ResponseMetadata {
				status: 200,
				method,
				path,
				headers: HashMap::new(),
			},
		}
	}
}
