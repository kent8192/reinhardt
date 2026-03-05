//! JSONField implementation for handling JSON data in forms

use crate::Widget;
use crate::field::{FieldError, FieldResult, FormField};
use serde_json::{self, Value};

/// A field for JSON data
///
/// Validates that the input is valid JSON and optionally enforces a schema.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::JSONField;
/// use reinhardt_forms::Field;
/// use serde_json::json;
///
/// let field = JSONField::new("data");
///
/// // Valid JSON object
/// let result = field.clean(Some(&json!(r#"{"name": "John", "age": 30}"#)));
/// assert!(result.is_ok());
///
/// // Valid JSON array
/// let result = field.clean(Some(&json!(r#"[1, 2, 3]"#)));
/// assert!(result.is_ok());
///
/// // Invalid JSON
/// let result = field.clean(Some(&json!(r#"{invalid}"#)));
/// assert!(result.is_err());
/// ```
/// Default maximum nesting depth for JSON deserialization
const DEFAULT_MAX_DEPTH: usize = 64;

#[derive(Debug, Clone)]
pub struct JSONField {
	pub name: String,
	pub required: bool,
	pub error_messages: std::collections::HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
	/// Whether to validate JSON is an object (not array, string, etc.)
	pub require_object: bool,
	/// Whether to validate JSON is an array
	pub require_array: bool,
	/// Required keys for JSON objects
	pub required_keys: Vec<String>,
	/// Maximum nesting depth for JSON deserialization to prevent stack overflow
	pub max_depth: usize,
}

impl JSONField {
	/// Create a new JSONField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::JSONField;
	///
	/// let field = JSONField::new("config");
	/// assert_eq!(field.name, "config");
	/// assert!(field.required);
	/// ```
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = std::collections::HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert("invalid".to_string(), "Enter valid JSON.".to_string());
		error_messages.insert("invalid_type".to_string(), "Invalid JSON type.".to_string());
		error_messages.insert(
			"missing_keys".to_string(),
			"Missing required keys.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::TextArea,
			help_text: String::new(),
			initial: None,
			require_object: false,
			require_array: false,
			required_keys: Vec::new(),
			max_depth: DEFAULT_MAX_DEPTH,
		}
	}
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}
	pub fn require_object(mut self) -> Self {
		self.require_object = true;
		self
	}
	pub fn require_array(mut self) -> Self {
		self.require_array = true;
		self
	}
	pub fn required_keys(mut self, keys: Vec<String>) -> Self {
		self.required_keys = keys;
		self
	}
	/// Set the maximum nesting depth for JSON deserialization.
	///
	/// This prevents stack overflow attacks from deeply nested JSON payloads.
	/// Default is 64.
	pub fn max_depth(mut self, depth: usize) -> Self {
		self.max_depth = depth;
		self
	}
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}

	/// Check if the parsed JSON exceeds the maximum nesting depth.
	fn check_depth(value: &Value, max_depth: usize) -> bool {
		Self::depth_check_recursive(value, 0, max_depth)
	}

	fn depth_check_recursive(value: &Value, current: usize, max: usize) -> bool {
		if current > max {
			return false;
		}
		match value {
			Value::Array(arr) => arr
				.iter()
				.all(|v| Self::depth_check_recursive(v, current + 1, max)),
			Value::Object(map) => map
				.values()
				.all(|v| Self::depth_check_recursive(v, current + 1, max)),
			_ => true,
		}
	}

	/// Validate that required keys are present in JSON object
	fn validate_required_keys(&self, obj: &serde_json::Map<String, Value>) -> FieldResult<()> {
		if self.required_keys.is_empty() {
			return Ok(());
		}

		let missing_keys: Vec<&String> = self
			.required_keys
			.iter()
			.filter(|key| !obj.contains_key(*key))
			.collect();

		if !missing_keys.is_empty() {
			let error_msg = self
				.error_messages
				.get("missing_keys")
				.cloned()
				.unwrap_or_else(|| "Missing required keys.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		Ok(())
	}
}

impl FormField for JSONField {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> Option<&str> {
		None
	}

	fn widget(&self) -> &Widget {
		&self.widget
	}

	fn required(&self) -> bool {
		self.required
	}

	fn initial(&self) -> Option<&Value> {
		self.initial.as_ref()
	}

	fn help_text(&self) -> Option<&str> {
		if self.help_text.is_empty() {
			None
		} else {
			Some(&self.help_text)
		}
	}

	fn clean(&self, value: Option<&Value>) -> FieldResult<Value> {
		// Handle None/null
		if value.is_none() || value == Some(&Value::Null) {
			if self.required {
				let error_msg = self
					.error_messages
					.get("required")
					.cloned()
					.unwrap_or_else(|| "This field is required.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
			return Ok(Value::Null);
		}

		let json_str = match value.unwrap() {
			Value::String(s) => s.as_str(),
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid")
					.cloned()
					.unwrap_or_else(|| "Enter valid JSON.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		// Empty string handling
		if json_str.trim().is_empty() {
			if self.required {
				let error_msg = self
					.error_messages
					.get("required")
					.cloned()
					.unwrap_or_else(|| "This field is required.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
			return Ok(Value::Null);
		}

		// Parse JSON
		let parsed: Value = match serde_json::from_str(json_str) {
			Ok(v) => v,
			Err(_) => {
				let error_msg = self
					.error_messages
					.get("invalid")
					.cloned()
					.unwrap_or_else(|| "Enter valid JSON.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		// Check nesting depth to prevent stack overflow from deeply nested payloads
		if !Self::check_depth(&parsed, self.max_depth) {
			return Err(FieldError::validation(
				None,
				"JSON structure is too deeply nested.",
			));
		}

		// Validate type constraints
		if self.require_object && !parsed.is_object() {
			let error_msg = self
				.error_messages
				.get("invalid_type")
				.cloned()
				.unwrap_or_else(|| "JSON must be an object.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		if self.require_array && !parsed.is_array() {
			let error_msg = self
				.error_messages
				.get("invalid_type")
				.cloned()
				.unwrap_or_else(|| "JSON must be an array.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		// Validate required keys for objects
		if let Value::Object(ref obj) = parsed {
			self.validate_required_keys(obj)?;
		}

		Ok(parsed)
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(a), Some(b)) => {
				// Normalize both values by parsing and re-serializing
				// This handles different whitespace, key ordering, etc.
				let a_normalized = serde_json::to_string(a).unwrap_or_default();
				let b_normalized = serde_json::to_string(b).unwrap_or_default();
				a_normalized != b_normalized
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_json_field_valid_object() {
		let field = JSONField::new("data");
		let result = field.clean(Some(&json!(r#"{"name": "John", "age": 30}"#)));
		let value = result.unwrap();
		assert!(value.is_object());
	}

	#[test]
	fn test_json_field_valid_array() {
		let field = JSONField::new("data");
		let result = field.clean(Some(&json!(r#"[1, 2, 3, 4, 5]"#)));
		let value = result.unwrap();
		assert!(value.is_array());
	}

	#[test]
	fn test_json_field_invalid() {
		let field = JSONField::new("data");
		let result = field.clean(Some(&json!(r#"{invalid json}"#)));
		assert!(result.is_err());
	}

	#[test]
	fn test_json_field_required() {
		let field = JSONField::new("data").required(true);
		let result = field.clean(None);
		assert!(result.is_err());
	}

	#[test]
	fn test_json_field_not_required() {
		let field = JSONField::new("data").required(false);
		let result = field.clean(None);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Value::Null);
	}

	#[test]
	fn test_json_field_require_object() {
		let field = JSONField::new("data").require_object();

		// Valid object
		let result = field.clean(Some(&json!(r#"{"key": "value"}"#)));
		assert!(result.is_ok());

		// Invalid - array
		let result = field.clean(Some(&json!(r#"[1, 2, 3]"#)));
		assert!(result.is_err());
	}

	#[test]
	fn test_json_field_require_array() {
		let field = JSONField::new("data").require_array();

		// Valid array
		let result = field.clean(Some(&json!(r#"[1, 2, 3]"#)));
		assert!(result.is_ok());

		// Invalid - object
		let result = field.clean(Some(&json!(r#"{"key": "value"}"#)));
		assert!(result.is_err());
	}

	#[test]
	fn test_json_field_required_keys() {
		let field = JSONField::new("data")
			.require_object()
			.required_keys(vec!["name".to_string(), "age".to_string()]);

		// Valid - has all required keys
		let result = field.clean(Some(&json!(
			r#"{"name": "John", "age": 30, "city": "NYC"}"#
		)));
		assert!(result.is_ok());

		// Invalid - missing "age" key
		let result = field.clean(Some(&json!(r#"{"name": "John"}"#)));
		assert!(result.is_err());
	}

	#[test]
	fn test_json_field_has_changed() {
		let field = JSONField::new("data");

		// Same values
		assert!(!field.has_changed(
			Some(&json!({"name": "John"})),
			Some(&json!({"name": "John"}))
		));

		// Different values
		assert!(field.has_changed(
			Some(&json!({"name": "John"})),
			Some(&json!({"name": "Jane"}))
		));

		// None vs Some
		assert!(field.has_changed(None, Some(&json!({"name": "John"}))));
	}
}
