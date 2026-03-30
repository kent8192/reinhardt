//! Content-Type Negotiation for Server Functions
//!
//! This module provides utilities for converting request bodies between
//! different serialization formats. When a client sends a request with
//! a Content-Type that differs from the codec expected by the server
//! function, the body is transparently converted to the target format.
//!
//! ## Supported Conversions
//!
//! - `application/x-www-form-urlencoded` -> JSON codec
//! - `application/json` -> URL codec
//!
//! ## Example
//!
//! ```no_run
//! use reinhardt_pages::server_fn::negotiation::convert_body_for_codec;
//!
//! // Convert a URL-encoded form body to JSON for a JSON-codec server function
//! let json_body = convert_body_for_codec(
//!     "name=Alice&age=30".to_string(),
//!     "application/x-www-form-urlencoded",
//!     "json",
//! ).unwrap();
//! let parsed: serde_json::Value = serde_json::from_str(&json_body).unwrap();
//! assert_eq!(parsed["name"], "Alice");
//! assert_eq!(parsed["age"], "30");
//! ```

/// Convert a request body from its original Content-Type to the format
/// expected by the target codec.
///
/// # Arguments
///
/// * `body` - The raw request body as a string
/// * `content_type` - The Content-Type header value from the request
///   (e.g., `"application/json; charset=utf-8"`)
/// * `target_codec` - The codec name expected by the server function
///   (one of `"json"`, `"url"`, `"msgpack"`)
///
/// # Returns
///
/// The body converted to the target codec's format, or the original body
/// if no conversion is needed.
///
/// # Errors
///
/// Returns an error string if:
/// - The conversion between the given Content-Type and target codec is not supported
/// - The body cannot be parsed in its declared format
/// - The parsed data cannot be serialized to the target format
pub fn convert_body_for_codec(
	body: String,
	content_type: &str,
	target_codec: &str,
) -> Result<String, String> {
	// Extract media type by stripping parameters (e.g., charset) after ';'
	let media_type = content_type
		.split(';')
		.next()
		.unwrap_or("")
		.trim()
		.to_lowercase();

	// Determine the expected content type for the target codec
	let expected_type = match target_codec {
		"json" => "application/json",
		"url" => "application/x-www-form-urlencoded",
		"msgpack" => "application/msgpack",
		other => return Err(format!("Unknown target codec: {other}")),
	};

	// No conversion needed if Content-Type is empty or already matches
	if media_type.is_empty() || media_type == expected_type {
		return Ok(body);
	}

	// Perform the conversion based on source -> target
	match (media_type.as_str(), target_codec) {
		// URL-encoded form data -> JSON
		("application/x-www-form-urlencoded", "json") => {
			let value: serde_json::Value = serde_urlencoded::from_str(&body)
				.map_err(|e| format!("Failed to parse URL-encoded body: {e}"))?;
			serde_json::to_string(&value).map_err(|e| format!("Failed to serialize to JSON: {e}"))
		}
		// JSON -> URL-encoded form data
		("application/json", "url") => {
			let value: serde_json::Value = serde_json::from_str(&body)
				.map_err(|e| format!("Failed to parse JSON body: {e}"))?;
			serde_urlencoded::to_string(&value)
				.map_err(|e| format!("Failed to serialize to URL-encoded format: {e}"))
		}
		// Unsupported conversion
		(source, target) => Err(format!(
			"Unsupported Content-Type conversion: '{source}' -> '{target}' codec"
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn form_to_json_converts_flat_key_value_pairs() {
		// Arrange
		let body = "name=Alice&age=30".to_string();
		let content_type = "application/x-www-form-urlencoded";
		let target_codec = "json";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec).unwrap();

		// Assert
		let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
		assert_eq!(parsed["name"], "Alice");
		assert_eq!(parsed["age"], "30");
	}

	#[rstest]
	fn json_to_url_converts_object_to_form_encoding() {
		// Arrange
		let body = r#"{"query":"rust","page":"1"}"#.to_string();
		let content_type = "application/json";
		let target_codec = "url";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec).unwrap();

		// Assert
		assert!(result.contains("query=rust"));
		assert!(result.contains("page=1"));
	}

	#[rstest]
	fn same_content_type_returns_body_unchanged() {
		// Arrange
		let body = r#"{"id":42}"#.to_string();
		let content_type = "application/json";
		let target_codec = "json";

		// Act
		let result = convert_body_for_codec(body.clone(), content_type, target_codec).unwrap();

		// Assert
		assert_eq!(result, body);
	}

	#[rstest]
	fn empty_content_type_returns_body_unchanged() {
		// Arrange
		let body = r#"{"id":42}"#.to_string();
		let content_type = "";
		let target_codec = "json";

		// Act
		let result = convert_body_for_codec(body.clone(), content_type, target_codec).unwrap();

		// Assert
		assert_eq!(result, body);
	}

	#[rstest]
	fn content_type_with_charset_parameter_is_handled() {
		// Arrange
		let body = r#"{"id":42}"#.to_string();
		let content_type = "application/json; charset=utf-8";
		let target_codec = "json";

		// Act
		let result = convert_body_for_codec(body.clone(), content_type, target_codec).unwrap();

		// Assert
		assert_eq!(result, body);
	}

	#[rstest]
	fn unsupported_conversion_returns_error() {
		// Arrange
		let body = "binary data".to_string();
		let content_type = "application/octet-stream";
		let target_codec = "json";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.contains("Unsupported Content-Type conversion"),
			"Expected unsupported conversion error, got: {err}"
		);
	}

	#[rstest]
	fn malformed_json_to_url_returns_error() {
		// Arrange
		// Invalid JSON body that cannot be parsed
		let body = "not json at all {{{".to_string();
		let content_type = "application/json";
		let target_codec = "url";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.contains("Failed to parse JSON body"),
			"Expected parse error, got: {err}"
		);
	}

	#[rstest]
	fn malformed_json_body_returns_error() {
		// Arrange
		let body = "{ not valid json }".to_string();
		let content_type = "application/json";
		let target_codec = "url";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.contains("Failed to parse JSON body"),
			"Expected parse error, got: {err}"
		);
	}

	#[rstest]
	fn unknown_target_codec_returns_error() {
		// Arrange
		let body = "data".to_string();
		let content_type = "application/json";
		let target_codec = "xml";

		// Act
		let result = convert_body_for_codec(body, content_type, target_codec);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.contains("Unknown target codec"),
			"Expected unknown codec error, got: {err}"
		);
	}
}
