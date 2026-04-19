//! Parameter Error Context
//!
//! This module provides detailed error context for HTTP parameter extraction failures.
//! It supports various parameter types (JSON, Query, Path, Form, Header, Cookie, Body)
//! and provides structured error information including field names, expected types,
//! and raw values for debugging.

/// Parameter type for error context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
	/// JSON request body parameter.
	Json,
	/// URL query string parameter.
	Query,
	/// URL path parameter.
	Path,
	/// Form-encoded body parameter.
	Form,
	/// HTTP header parameter.
	Header,
	/// Cookie parameter.
	Cookie,
	/// Raw request body parameter.
	Body,
}

impl std::fmt::Display for ParamType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ParamType::Json => write!(f, "Json"),
			ParamType::Query => write!(f, "Query"),
			ParamType::Path => write!(f, "Path"),
			ParamType::Form => write!(f, "Form"),
			ParamType::Header => write!(f, "Header"),
			ParamType::Cookie => write!(f, "Cookie"),
			ParamType::Body => write!(f, "Body"),
		}
	}
}

/// Detailed context for parameter extraction errors
#[derive(Debug, Clone)]
pub struct ParamErrorContext {
	/// Parameter type (Json, Query, Path, Form, Header, etc.)
	pub param_type: ParamType,
	/// Field name if identifiable
	pub field_name: Option<String>,
	/// Error message
	pub message: String,
	/// Original source error (not cloneable, so we store the message)
	pub source_message: Option<String>,
	/// Original value (for debugging, sensitive data should be excluded)
	pub raw_value: Option<String>,
	/// Expected type name
	pub expected_type: Option<String>,
}

impl ParamErrorContext {
	/// Create a new ParamErrorContext
	pub fn new(param_type: ParamType, message: impl Into<String>) -> Self {
		Self {
			param_type,
			field_name: None,
			message: message.into(),
			source_message: None,
			raw_value: None,
			expected_type: None,
		}
	}

	/// Set the field name
	pub fn with_field(mut self, field: impl Into<String>) -> Self {
		self.field_name = Some(field.into());
		self
	}

	/// Set the source error
	pub fn with_source(mut self, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
		self.source_message = Some(source.to_string());
		self
	}

	/// Set the raw value (truncated if too long)
	pub fn with_raw_value(mut self, value: impl Into<String>) -> Self {
		let value = value.into();
		// Truncate to ~500 bytes max to avoid log spam.
		// Use char_indices to find a safe truncation point on a char boundary,
		// preventing panics on multi-byte UTF-8 strings (e.g., Japanese, emoji).
		if value.len() > 500 {
			let truncation_point = value
				.char_indices()
				.map(|(idx, _)| idx)
				.take_while(|&idx| idx <= 500)
				.last()
				.unwrap_or(0);
			self.raw_value = Some(format!("{}...[truncated]", &value[..truncation_point]));
		} else {
			self.raw_value = Some(value);
		}
		self
	}

	/// Set the expected type
	pub fn with_expected_type<T>(mut self) -> Self {
		self.expected_type = Some(std::any::type_name::<T>().to_string());
		self
	}

	/// Format error as single line (for Display trait)
	pub fn format_error(&self) -> String {
		let mut parts = vec![format!("{} parameter extraction failed", self.param_type)];

		if let Some(ref field) = self.field_name {
			parts.push(format!("field: '{}'", field));
		}

		parts.push(format!("error: {}", self.message));

		if let Some(ref expected) = self.expected_type {
			parts.push(format!("expected type: {}", expected));
		}

		parts.join(", ")
	}

	/// Format error as multiple lines (for detailed logging)
	pub fn format_multiline(&self, include_raw_value: bool) -> String {
		let mut lines = vec![
			format!("  {} parameter extraction failed", self.param_type),
			format!("  Error: {}", self.message),
		];

		if let Some(ref field) = self.field_name {
			lines.push(format!("  Field: {}", field));
		}

		if let Some(ref expected) = self.expected_type {
			lines.push(format!("  Expected type: {}", expected));
		}

		if include_raw_value && let Some(ref raw) = self.raw_value {
			lines.push(format!("  Received: {}", raw));
		}

		lines.join("\n")
	}
}

/// Extract field name from serde_json::Error message
pub fn extract_field_from_serde_error(err: &serde_json::Error) -> Option<String> {
	let msg = err.to_string();

	// "missing field `xxx`" pattern
	if let Some(start) = msg.find("missing field `") {
		let rest = &msg[start + 15..];
		if let Some(end) = rest.find('`') {
			return Some(rest[..end].to_string());
		}
	}

	// "unknown field `xxx`" pattern
	if let Some(start) = msg.find("unknown field `") {
		let rest = &msg[start + 15..];
		if let Some(end) = rest.find('`') {
			return Some(rest[..end].to_string());
		}
	}

	// "duplicate field `xxx`" pattern
	if let Some(start) = msg.find("duplicate field `") {
		let rest = &msg[start + 17..];
		if let Some(end) = rest.find('`') {
			return Some(rest[..end].to_string());
		}
	}

	None
}

/// Extract field name from serde_urlencoded error message
pub fn extract_field_from_urlencoded_error(err: &serde_urlencoded::de::Error) -> Option<String> {
	let msg = err.to_string();

	// "missing field `xxx`" pattern
	if let Some(start) = msg.find("missing field `") {
		let rest = &msg[start + 15..];
		if let Some(end) = rest.find('`') {
			return Some(rest[..end].to_string());
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	fn with_raw_value_does_not_panic_on_multibyte_utf8() {
		// Arrange - 500+ bytes of multi-byte Japanese characters
		// Each character is 3 bytes in UTF-8, so 200 chars = 600 bytes
		let japanese_str: String = "あ".repeat(200);
		assert!(japanese_str.len() > 500);

		// Act - must not panic on multi-byte boundary
		let ctx = ParamErrorContext::new(ParamType::Json, "test").with_raw_value(japanese_str);

		// Assert
		let raw = ctx.raw_value.unwrap();
		assert!(raw.ends_with("...[truncated]"));
	}

	#[rstest]
	fn with_raw_value_does_not_panic_on_emoji() {
		// Arrange - emoji are 4 bytes each, 150 emojis = 600 bytes
		let emoji_str: String = "\u{1F600}".repeat(150);
		assert!(emoji_str.len() > 500);

		// Act - must not panic on 4-byte char boundary
		let ctx = ParamErrorContext::new(ParamType::Query, "test").with_raw_value(emoji_str);

		// Assert
		let raw = ctx.raw_value.unwrap();
		assert!(raw.ends_with("...[truncated]"));
		// Verify truncated content is valid UTF-8 (would panic if not)
		assert!(raw.is_char_boundary(0));
	}

	#[rstest]
	fn with_raw_value_does_not_truncate_short_strings() {
		// Arrange
		let short = "hello world";

		// Act
		let ctx = ParamErrorContext::new(ParamType::Path, "test").with_raw_value(short);

		// Assert
		assert_eq!(ctx.raw_value.unwrap(), "hello world");
	}

	#[rstest]
	fn with_raw_value_handles_mixed_multibyte_ascii() {
		// Arrange - mix of ASCII and multi-byte characters totaling > 500 bytes
		let mixed: String = "a".repeat(498) + "ああ"; // 498 + 6 = 504 bytes
		assert!(mixed.len() > 500);

		// Act
		let ctx = ParamErrorContext::new(ParamType::Form, "test").with_raw_value(mixed);

		// Assert
		let raw = ctx.raw_value.unwrap();
		assert!(raw.ends_with("...[truncated]"));
	}

	#[rstest]
	fn with_raw_value_preserves_exactly_500_byte_string() {
		// Arrange - exactly 500 ASCII bytes
		let exact = "x".repeat(500);
		assert_eq!(exact.len(), 500);

		// Act
		let ctx = ParamErrorContext::new(ParamType::Header, "test").with_raw_value(exact.clone());

		// Assert - should NOT be truncated (len is not > 500)
		assert_eq!(ctx.raw_value.unwrap(), exact);
	}
}
