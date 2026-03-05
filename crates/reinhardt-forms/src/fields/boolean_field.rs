//! Boolean field with Django-compatible required semantics

use crate::field::{FieldError, FieldResult, FormField, Widget};

/// Boolean field for checkbox input
#[derive(Debug, Clone)]
pub struct BooleanField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub initial: Option<serde_json::Value>,
}

impl BooleanField {
	/// Create a new BooleanField with the given name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::BooleanField;
	///
	/// let field = BooleanField::new("accept_terms".to_string());
	/// assert_eq!(field.name, "accept_terms");
	/// assert!(!field.required);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: false,
			help_text: None,
			initial: None,
		}
	}

	/// Set the field as required
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::BooleanField;
	///
	/// let field = BooleanField::new("terms".to_string()).required();
	/// assert!(field.required);
	/// ```
	pub fn required(mut self) -> Self {
		self.required = true;
		self
	}

	/// Set the label for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::BooleanField;
	///
	/// let field = BooleanField::new("agree".to_string()).with_label("I agree");
	/// assert_eq!(field.label, Some("I agree".to_string()));
	/// ```
	pub fn with_label(mut self, label: impl Into<String>) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Set the help text for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::BooleanField;
	///
	/// let field = BooleanField::new("newsletter".to_string()).with_help_text("Subscribe to newsletter");
	/// assert_eq!(field.help_text, Some("Subscribe to newsletter".to_string()));
	/// ```
	pub fn with_help_text(mut self, help_text: impl Into<String>) -> Self {
		self.help_text = Some(help_text.into());
		self
	}

	/// Set the initial value for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::BooleanField;
	///
	/// let field = BooleanField::new("enabled".to_string()).with_initial(true);
	/// assert_eq!(field.initial, Some(serde_json::json!(true)));
	/// ```
	pub fn with_initial(mut self, initial: bool) -> Self {
		self.initial = Some(serde_json::json!(initial));
		self
	}
}

// Note: Default trait is not implemented because BooleanField requires a name

impl FormField for BooleanField {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> Option<&str> {
		self.label.as_deref()
	}

	fn required(&self) -> bool {
		self.required
	}

	fn help_text(&self) -> Option<&str> {
		self.help_text.as_deref()
	}

	fn widget(&self) -> &Widget {
		&Widget::CheckboxInput
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.initial.as_ref()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		match value {
			None => {
				if self.required {
					Err(FieldError::Required(self.name.clone()))
				} else {
					Ok(serde_json::Value::Bool(false))
				}
			}
			Some(v) => {
				// Convert various types to boolean (Django-like behavior)
				let b = match v {
					serde_json::Value::Bool(b) => *b,
					serde_json::Value::String(s) => {
						// String conversion: "false", "False", "0", "" -> false, others -> true
						let s_lower = s.to_lowercase();
						!(s.is_empty() || s_lower == "false" || s == "0")
					}
					serde_json::Value::Number(n) => {
						// Numbers: 0 -> false, non-zero -> true
						if let Some(i) = n.as_i64() {
							i != 0
						} else if let Some(f) = n.as_f64() {
							f != 0.0
						} else {
							true
						}
					}
					serde_json::Value::Null => false,
					_ => {
						return Err(FieldError::Validation(
							"Cannot convert to boolean".to_string(),
						));
					}
				};

				// Django behavior: a required BooleanField must be True.
				// This ensures consent checkboxes (e.g., "I agree to Terms")
				// cannot be submitted unchecked.
				if self.required && !b {
					return Err(FieldError::Required(self.name.clone()));
				}

				Ok(serde_json::Value::Bool(b))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	fn test_required_boolean_rejects_false() {
		// Arrange: required BooleanField should require true (Django behavior)
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert: false should be rejected for required BooleanField
		assert!(
			field.clean(Some(&json!(false))).is_err(),
			"required BooleanField should reject false"
		);
	}

	#[rstest]
	fn test_required_boolean_accepts_true() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act
		let result = field.clean(Some(&json!(true)));

		// Assert
		assert_eq!(result.unwrap(), json!(true));
	}

	#[rstest]
	fn test_required_boolean_rejects_none() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert
		assert!(field.clean(None).is_err());
	}

	#[rstest]
	fn test_required_boolean_rejects_false_string() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert: "false" string converts to false, which should be rejected
		assert!(field.clean(Some(&json!("false"))).is_err());
		assert!(field.clean(Some(&json!("0"))).is_err());
		assert!(field.clean(Some(&json!(""))).is_err());
	}

	#[rstest]
	fn test_required_boolean_rejects_zero() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert: numeric 0 converts to false, which should be rejected
		assert!(field.clean(Some(&json!(0))).is_err());
	}

	#[rstest]
	fn test_optional_boolean_accepts_false() {
		// Arrange: optional BooleanField should accept false
		let field = BooleanField::new("newsletter".to_string());

		// Act
		let result = field.clean(Some(&json!(false)));

		// Assert
		assert_eq!(result.unwrap(), json!(false));
	}

	#[rstest]
	fn test_optional_boolean_accepts_none() {
		// Arrange
		let field = BooleanField::new("newsletter".to_string());

		// Act
		let result = field.clean(None);

		// Assert
		assert_eq!(result.unwrap(), json!(false));
	}

	#[rstest]
	fn test_required_boolean_accepts_truthy_string() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert: truthy strings should be accepted
		assert_eq!(field.clean(Some(&json!("true"))).unwrap(), json!(true));
		assert_eq!(field.clean(Some(&json!("yes"))).unwrap(), json!(true));
		assert_eq!(field.clean(Some(&json!("1"))).unwrap(), json!(true));
	}

	#[rstest]
	fn test_required_boolean_rejects_null() {
		// Arrange
		let field = BooleanField::new("terms".to_string()).required();

		// Act & Assert: null converts to false, which should be rejected
		assert!(field.clean(Some(&json!(null))).is_err());
	}
}
