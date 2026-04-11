//! Mock HTTP response builder.

use std::collections::HashMap;

use serde::Serialize;

/// A mock HTTP response returned by handlers.
#[derive(Debug, Clone)]
pub struct MockResponse {
	/// HTTP status code.
	pub status: u16,
	/// Response headers.
	pub headers: HashMap<String, String>,
	/// Response body as a string.
	pub body: String,
}

impl MockResponse {
	/// Create a JSON response with status 200.
	pub fn json<T: Serialize>(data: T) -> Self {
		let body = serde_json::to_string(&data).expect("Failed to serialize JSON response");
		let mut headers = HashMap::new();
		headers.insert("content-type".to_string(), "application/json".to_string());
		Self {
			status: 200,
			headers,
			body,
		}
	}

	/// Create a plain text response with status 200.
	pub fn text(body: impl Into<String>) -> Self {
		let mut headers = HashMap::new();
		headers.insert(
			"content-type".to_string(),
			"text/plain; charset=utf-8".to_string(),
		);
		Self {
			status: 200,
			headers,
			body: body.into(),
		}
	}

	/// Create an empty response with status 200.
	pub fn empty() -> Self {
		Self {
			status: 200,
			headers: HashMap::new(),
			body: String::new(),
		}
	}

	/// Override the HTTP status code.
	pub fn with_status(mut self, status: u16) -> Self {
		self.status = status;
		self
	}

	/// Add a response header.
	pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.headers.insert(name.into(), value.into());
		self
	}
}

impl From<serde_json::Value> for MockResponse {
	fn from(value: serde_json::Value) -> Self {
		Self::json(value)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use serde_json::json;

	#[rstest]
	fn json_response_sets_content_type_and_body() {
		let resp = MockResponse::json(json!({"id": 1}));
		assert_eq!(resp.status, 200);
		assert_eq!(
			resp.headers.get("content-type").unwrap(),
			"application/json"
		);
		assert_eq!(resp.body, r#"{"id":1}"#);
	}

	#[rstest]
	fn text_response() {
		let resp = MockResponse::text("hello");
		assert_eq!(resp.status, 200);
		assert_eq!(resp.body, "hello");
		assert_eq!(
			resp.headers.get("content-type").unwrap(),
			"text/plain; charset=utf-8"
		);
	}

	#[rstest]
	fn empty_response() {
		let resp = MockResponse::empty();
		assert_eq!(resp.status, 200);
		assert!(resp.body.is_empty());
	}

	#[rstest]
	fn with_status_overrides() {
		let resp = MockResponse::empty().with_status(404);
		assert_eq!(resp.status, 404);
	}

	#[rstest]
	fn with_header_adds() {
		let resp = MockResponse::empty().with_header("x-custom", "value");
		assert_eq!(resp.headers.get("x-custom").unwrap(), "value");
	}

	#[rstest]
	fn from_serde_json_value() {
		let resp: MockResponse = json!({"ok": true}).into();
		assert_eq!(resp.status, 200);
		assert_eq!(resp.body, r#"{"ok":true}"#);
	}
}
