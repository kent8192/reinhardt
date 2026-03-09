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
		// Truncate to 500 chars max to avoid log spam
		if value.len() > 500 {
			self.raw_value = Some(format!("{}...[truncated]", &value[..500]));
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
