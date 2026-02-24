//! Email field with validation

use crate::field::{FieldError, FieldResult, FormField, Widget};
use regex::Regex;

/// Email field with format validation
#[derive(Debug, Clone)]
pub struct EmailField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_length: Option<usize>,
	pub min_length: Option<usize>,
}

impl EmailField {
	/// Create a new EmailField with the given name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("email".to_string());
	/// assert_eq!(field.name, "email");
	/// assert!(!field.required);
	/// assert_eq!(field.max_length, Some(320));
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: false,
			help_text: None,
			widget: Widget::EmailInput,
			initial: None,
			max_length: Some(320), // RFC standard: 64 (local) + @ + 255 (domain)
			min_length: None,
		}
	}

	/// Set the field as required
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("contact".to_string()).required();
	/// assert!(field.required);
	/// ```
	pub fn required(mut self) -> Self {
		self.required = true;
		self
	}

	/// Set the maximum length for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("email".to_string()).with_max_length(100);
	/// assert_eq!(field.max_length, Some(100));
	/// ```
	pub fn with_max_length(mut self, max_length: usize) -> Self {
		self.max_length = Some(max_length);
		self
	}

	/// Set the minimum length for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("email".to_string()).with_min_length(5);
	/// assert_eq!(field.min_length, Some(5));
	/// ```
	pub fn with_min_length(mut self, min_length: usize) -> Self {
		self.min_length = Some(min_length);
		self
	}

	/// Set the label for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("contact_email".to_string()).with_label("Email Address");
	/// assert_eq!(field.label, Some("Email Address".to_string()));
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
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("email".to_string()).with_help_text("We'll never share your email");
	/// assert_eq!(field.help_text, Some("We'll never share your email".to_string()));
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
	/// use reinhardt_forms::fields::EmailField;
	///
	/// let field = EmailField::new("email".to_string()).with_initial("user@example.com");
	/// assert_eq!(field.initial, Some(serde_json::json!("user@example.com")));
	/// ```
	pub fn with_initial(mut self, initial: impl Into<String>) -> Self {
		self.initial = Some(serde_json::json!(initial.into()));
		self
	}

	/// Validate email format
	fn validate_email(email: &str) -> bool {
		// Basic email validation regex
		// This is a simplified version - production should use a more robust validator
		let email_regex = Regex::new(
            r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
        ).unwrap();

		email_regex.is_match(email)
	}
}

// Note: Default trait is not implemented because EmailField requires a name

impl FormField for EmailField {
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
		&self.widget
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.initial.as_ref()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		match value {
			None if self.required => Err(FieldError::Required(self.name.clone())),
			None => Ok(serde_json::Value::String(String::new())),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::Validation("Expected string".to_string()))?;

				// Trim whitespace
				let s = s.trim();

				// Return empty string if not required and empty
				if s.is_empty() {
					if self.required {
						return Err(FieldError::Required(self.name.clone()));
					}
					return Ok(serde_json::Value::String(String::new()));
				}

				// Check length constraints using character count (not byte count)
				// for correct multi-byte character handling
				let char_count = s.chars().count();
				if let Some(max) = self.max_length
					&& char_count > max
				{
					return Err(FieldError::Validation(format!(
						"Ensure this value has at most {} characters (it has {})",
						max, char_count
					)));
				}

				if let Some(min) = self.min_length
					&& char_count < min
				{
					return Err(FieldError::Validation(format!(
						"Ensure this value has at least {} characters (it has {})",
						min, char_count
					)));
				}

				// Validate email format
				if !Self::validate_email(s) {
					return Err(FieldError::Validation(
						"Enter a valid email address".to_string(),
					));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}
