//! Advanced field types for specialized data validation

use crate::Widget;
use crate::field::{FieldError, FieldResult, FormField};
use serde_json::Value;
use std::collections::HashMap;

/// A field for UUID validation
///
/// Validates that the input is a valid UUID (Universally Unique Identifier).
/// Supports UUID v4 by default.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::UUIDField;
/// use reinhardt_forms::Field;
/// use serde_json::json;
///
/// let field = UUIDField::new("id");
///
// Valid UUID v4
/// let result = field.clean(Some(&json!("550e8400-e29b-41d4-a716-446655440000")));
/// assert!(result.is_ok());
///
// Invalid UUID
/// let result = field.clean(Some(&json!("not-a-uuid")));
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct UUIDField {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
}

impl UUIDField {
	/// Create a new UUIDField
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert("invalid".to_string(), "Enter a valid UUID.".to_string());

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::TextInput,
			help_text: String::new(),
			initial: None,
		}
	}

	/// Set whether this field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set the help text
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}

	/// Set the initial value
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}

	/// Set a custom error message
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}

	/// Validate UUID format
	fn validate_uuid(&self, s: &str) -> bool {
		// UUID format: 8-4-4-4-12 hexadecimal digits
		let parts: Vec<&str> = s.split('-').collect();
		if parts.len() != 5 {
			return false;
		}

		if parts[0].len() != 8
			|| parts[1].len() != 4
			|| parts[2].len() != 4
			|| parts[3].len() != 4
			|| parts[4].len() != 12
		{
			return false;
		}

		parts
			.iter()
			.all(|part| part.chars().all(|c| c.is_ascii_hexdigit()))
	}
}

impl FormField for UUIDField {
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

		let s = match value.unwrap() {
			Value::String(s) => s.trim(),
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid")
					.cloned()
					.unwrap_or_else(|| "Enter a valid UUID.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		if s.is_empty() {
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

		if !self.validate_uuid(s) {
			let error_msg = self
				.error_messages
				.get("invalid")
				.cloned()
				.unwrap_or_else(|| "Enter a valid UUID.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		Ok(Value::String(s.to_lowercase()))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(Value::String(a)), Some(Value::String(b))) => {
				a.to_lowercase() != b.to_lowercase()
			}
			(Some(a), Some(b)) => a != b,
		}
	}
}

/// A field for ISO 8601 duration validation
///
/// Validates that the input is a valid ISO 8601 duration format (e.g., "P1Y2M3DT4H5M6S").
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::DurationField;
/// use reinhardt_forms::Field;
/// use serde_json::json;
///
/// let field = DurationField::new("duration");
///
// Valid duration
/// let result = field.clean(Some(&json!("P1Y2M3DT4H5M6S")));
/// assert!(result.is_ok());
///
// Another valid duration (1 day)
/// let result = field.clean(Some(&json!("P1D")));
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct DurationField {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
}

impl DurationField {
	/// Create a new DurationField
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert(
			"invalid".to_string(),
			"Enter a valid ISO 8601 duration.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::TextInput,
			help_text: String::new(),
			initial: None,
		}
	}

	/// Set whether this field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set the help text
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}

	/// Set the initial value
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}

	/// Set a custom error message
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}

	/// Validate ISO 8601 duration format
	/// Format: P[n]Y[n]M[n]DT[n]H[n]M[n]S or P[n]W
	fn validate_duration(&self, s: &str) -> bool {
		if !s.starts_with('P') {
			return false;
		}

		let s = &s[1..]; // Remove 'P' prefix

		if s.is_empty() {
			return false;
		}

		// Week format: P[n]W
		if let Some(num_part) = s.strip_suffix('W') {
			return num_part.chars().all(|c| c.is_ascii_digit());
		}

		// Date and time format
		let parts: Vec<&str> = s.split('T').collect();

		if parts.is_empty() || parts.len() > 2 {
			return false;
		}

		// Validate date part (Y, M, D)
		let date_valid = self.validate_duration_part(parts[0], &['Y', 'M', 'D']);

		// Validate time part if present (H, M, S)
		let time_valid = if parts.len() == 2 {
			!parts[1].is_empty() && self.validate_duration_part(parts[1], &['H', 'M', 'S'])
		} else {
			true
		};

		date_valid && time_valid
	}

	/// Validate a duration part (either date or time)
	fn validate_duration_part(&self, part: &str, units: &[char]) -> bool {
		if part.is_empty() {
			return true; // Empty parts are okay
		}

		let mut current_num = String::new();

		for ch in part.chars() {
			if ch.is_ascii_digit() || ch == '.' {
				current_num.push(ch);
			} else if units.contains(&ch) {
				if current_num.is_empty() {
					return false;
				}
				current_num.clear();
			} else {
				return false;
			}
		}

		current_num.is_empty() // Should have consumed all digits
	}
}

impl FormField for DurationField {
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

		let s = match value.unwrap() {
			Value::String(s) => s.trim(),
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid")
					.cloned()
					.unwrap_or_else(|| "Enter a valid ISO 8601 duration.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		if s.is_empty() {
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

		if !self.validate_duration(s) {
			let error_msg = self
				.error_messages
				.get("invalid")
				.cloned()
				.unwrap_or_else(|| "Enter a valid ISO 8601 duration.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		Ok(Value::String(s.to_uppercase()))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(Value::String(a)), Some(Value::String(b))) => {
				a.to_uppercase() != b.to_uppercase()
			}
			(Some(a), Some(b)) => a != b,
		}
	}
}

/// A field that combines multiple field validators
///
/// ComboField runs all provided validators in sequence and requires all to pass.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::ComboField;
/// use reinhardt_forms::{Field, CharField, EmailField};
/// use serde_json::json;
///
// Create validators with constraints
/// let mut email_field = EmailField::new("email".to_string());
/// let mut char_field = CharField::new("email".to_string());
/// char_field.min_length = Some(5);
/// char_field.max_length = Some(100);
///
// Combine email validation with length validation
/// let field = ComboField::new("email")
///     .add_validator(Box::new(email_field))
///     .add_validator(Box::new(char_field));
///
// Valid: passes both email and length checks
/// let result = field.clean(Some(&json!("user@example.com")));
/// assert!(result.is_ok());
///
// Invalid: fails email validation
/// let result = field.clean(Some(&json!("not-an-email")));
/// assert!(result.is_err());
///
// Invalid: too short (less than 5 characters)
/// let result = field.clean(Some(&json!("a@b")));
/// assert!(result.is_err());
/// ```
pub struct ComboField {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
	pub validators: Vec<Box<dyn FormField>>,
}

impl ComboField {
	/// Create a new ComboField
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::TextInput,
			help_text: String::new(),
			initial: None,
			validators: Vec::new(),
		}
	}

	/// Add a validator field
	pub fn add_validator(mut self, validator: Box<dyn FormField>) -> Self {
		self.validators.push(validator);
		self
	}

	/// Set whether this field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set the help text
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}

	/// Set the initial value
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}

	/// Set a custom error message
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}
}

impl FormField for ComboField {
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

		// Run all validators
		let mut result = value.unwrap().clone();
		for validator in &self.validators {
			result = validator.clean(Some(&result))?;
		}

		Ok(result)
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(a), Some(b)) => a != b,
		}
	}
}

/// A field for color validation (hex format)
///
/// Validates that the input is a valid hex color code (e.g., "#FF0000" or "#F00").
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::ColorField;
/// use reinhardt_forms::Field;
/// use serde_json::json;
///
/// let field = ColorField::new("color");
///
/// // Valid 6-digit hex color
/// let result = field.clean(Some(&json!("#FF0000")));
/// assert!(result.is_ok());
///
/// // Valid 3-digit hex color
/// let result = field.clean(Some(&json!("#F00")));
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct ColorField {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
}

impl ColorField {
	/// Create a new ColorField
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert(
			"invalid".to_string(),
			"Enter a valid hex color code (e.g., #FF0000).".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::TextInput,
			help_text: String::new(),
			initial: None,
		}
	}

	/// Set whether this field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set the help text
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}

	/// Set the initial value
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}

	/// Validate hex color format
	fn validate_color(&self, s: &str) -> bool {
		if !s.starts_with('#') {
			return false;
		}

		let hex = &s[1..];
		if hex.len() != 3 && hex.len() != 6 {
			return false;
		}

		hex.chars().all(|c| c.is_ascii_hexdigit())
	}
}

impl FormField for ColorField {
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

		let s = match value.unwrap() {
			Value::String(s) => s.trim(),
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid")
					.cloned()
					.unwrap_or_else(|| "Enter a valid hex color code.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		if s.is_empty() {
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

		if !self.validate_color(s) {
			let error_msg = self
				.error_messages
				.get("invalid")
				.cloned()
				.unwrap_or_else(|| "Enter a valid hex color code.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		Ok(Value::String(s.to_uppercase()))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(Value::String(a)), Some(Value::String(b))) => {
				a.to_uppercase() != b.to_uppercase()
			}
			(Some(a), Some(b)) => a != b,
		}
	}
}

/// Redacted placeholder value stored in cleaned data for password fields.
///
/// Password fields store this constant instead of the plaintext password
/// to prevent accidental exposure of credentials in cleaned form data.
pub const PASSWORD_REDACTED: &str = "**********";

/// A field for password validation with strength requirements
///
/// Validates password strength including minimum length, required character types.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::fields::PasswordField;
/// use reinhardt_forms::Field;
/// use serde_json::json;
///
/// let field = PasswordField::new("password")
///     .min_length(8)
///     .require_uppercase(true)
///     .require_digit(true);
///
/// // Valid strong password
/// let result = field.clean(Some(&json!("SecurePass123")));
/// assert!(result.is_ok());
///
/// // Invalid: too short
/// let result = field.clean(Some(&json!("Pass1")));
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct PasswordField {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
	pub min_length: usize,
	pub require_uppercase: bool,
	pub require_lowercase: bool,
	pub require_digit: bool,
	pub require_special: bool,
}

impl PasswordField {
	/// Create a new PasswordField
	pub fn new(name: impl Into<String>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert(
			"too_short".to_string(),
			"Password must be at least {min_length} characters.".to_string(),
		);
		error_messages.insert(
			"no_uppercase".to_string(),
			"Password must contain at least one uppercase letter.".to_string(),
		);
		error_messages.insert(
			"no_lowercase".to_string(),
			"Password must contain at least one lowercase letter.".to_string(),
		);
		error_messages.insert(
			"no_digit".to_string(),
			"Password must contain at least one digit.".to_string(),
		);
		error_messages.insert(
			"no_special".to_string(),
			"Password must contain at least one special character.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::PasswordInput,
			help_text: String::new(),
			initial: None,
			min_length: 8,
			require_uppercase: false,
			require_lowercase: false,
			require_digit: false,
			require_special: false,
		}
	}

	/// Set minimum length
	pub fn min_length(mut self, length: usize) -> Self {
		self.min_length = length;
		self
	}

	/// Require uppercase letter
	pub fn require_uppercase(mut self, required: bool) -> Self {
		self.require_uppercase = required;
		self
	}

	/// Require lowercase letter
	pub fn require_lowercase(mut self, required: bool) -> Self {
		self.require_lowercase = required;
		self
	}

	/// Require digit
	pub fn require_digit(mut self, required: bool) -> Self {
		self.require_digit = required;
		self
	}

	/// Require special character
	pub fn require_special(mut self, required: bool) -> Self {
		self.require_special = required;
		self
	}

	/// Set whether this field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set the help text
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}
}

impl FormField for PasswordField {
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

		let s = match value.unwrap() {
			Value::String(s) => s,
			_ => {
				return Err(FieldError::validation(
					Some(&self.name),
					"Invalid password format.",
				));
			}
		};

		if s.is_empty() {
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

		// Check minimum length
		if s.len() < self.min_length {
			let error_msg = self
				.error_messages
				.get("too_short")
				.cloned()
				.unwrap_or_else(|| {
					format!("Password must be at least {} characters.", self.min_length)
				});
			return Err(FieldError::validation(None, &error_msg));
		}

		// Check uppercase requirement
		if self.require_uppercase && !s.chars().any(|c| c.is_uppercase()) {
			let error_msg = self
				.error_messages
				.get("no_uppercase")
				.cloned()
				.unwrap_or_else(|| {
					"Password must contain at least one uppercase letter.".to_string()
				});
			return Err(FieldError::validation(None, &error_msg));
		}

		// Check lowercase requirement
		if self.require_lowercase && !s.chars().any(|c| c.is_lowercase()) {
			let error_msg = self
				.error_messages
				.get("no_lowercase")
				.cloned()
				.unwrap_or_else(|| {
					"Password must contain at least one lowercase letter.".to_string()
				});
			return Err(FieldError::validation(None, &error_msg));
		}

		// Check digit requirement
		if self.require_digit && !s.chars().any(|c| c.is_ascii_digit()) {
			let error_msg = self
				.error_messages
				.get("no_digit")
				.cloned()
				.unwrap_or_else(|| "Password must contain at least one digit.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		// Check special character requirement
		if self.require_special && !s.chars().any(|c| !c.is_alphanumeric()) {
			let error_msg = self
				.error_messages
				.get("no_special")
				.cloned()
				.unwrap_or_else(|| {
					"Password must contain at least one special character.".to_string()
				});
			return Err(FieldError::validation(None, &error_msg));
		}

		// Redact password to prevent plaintext storage in cleaned data
		Ok(Value::String(PASSWORD_REDACTED.to_string()))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(a), Some(b)) => a != b,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_uuid_field_valid() {
		let field = UUIDField::new("id");

		let result = field.clean(Some(&json!("550e8400-e29b-41d4-a716-446655440000")));
		assert!(result.is_ok());

		// Case insensitive
		let result = field.clean(Some(&json!("550E8400-E29B-41D4-A716-446655440000")));
		assert!(result.is_ok());
	}

	#[test]
	fn test_uuid_field_invalid() {
		let field = UUIDField::new("id");

		// Too short
		let result = field.clean(Some(&json!("550e8400-e29b")));
		assert!(result.is_err());

		// Invalid characters
		let result = field.clean(Some(&json!("550e8400-e29b-41d4-a716-44665544000g")));
		assert!(result.is_err());

		// Wrong format
		let result = field.clean(Some(&json!("not-a-uuid")));
		assert!(result.is_err());
	}

	#[test]
	fn test_duration_field_valid() {
		let field = DurationField::new("duration");

		// Full format
		let result = field.clean(Some(&json!("P1Y2M3DT4H5M6S")));
		assert!(result.is_ok());

		// Days only
		let result = field.clean(Some(&json!("P1D")));
		assert!(result.is_ok());

		// Time only
		let result = field.clean(Some(&json!("PT1H")));
		assert!(result.is_ok());

		// Weeks
		let result = field.clean(Some(&json!("P2W")));
		assert!(result.is_ok());
	}

	#[test]
	fn test_duration_field_invalid() {
		let field = DurationField::new("duration");

		// Missing P prefix
		let result = field.clean(Some(&json!("1Y2M")));
		assert!(result.is_err());

		// Empty after P
		let result = field.clean(Some(&json!("P")));
		assert!(result.is_err());

		// Invalid format
		let result = field.clean(Some(&json!("P1X")));
		assert!(result.is_err());
	}

	#[test]
	fn test_combo_field() {
		use crate::EmailField;

		// Create a validator with length and email constraints
		let mut char_field_min = crate::CharField::new("text".to_string());
		char_field_min.min_length = Some(5);

		let mut char_field_max = crate::CharField::new("text".to_string());
		char_field_max.max_length = Some(50);

		let field = ComboField::new("email")
			.add_validator(Box::new(char_field_min))
			.add_validator(Box::new(char_field_max))
			.add_validator(Box::new(EmailField::new("email".to_string())));

		// Valid
		let result = field.clean(Some(&json!("test@example.com")));
		assert!(result.is_ok());

		// Too short
		let result = field.clean(Some(&json!("a@b")));
		assert!(result.is_err());

		// Not an email
		let result = field.clean(Some(&json!("hello world")));
		assert!(result.is_err());
	}

	#[test]
	fn test_color_field_valid() {
		let field = ColorField::new("color");

		// 6-digit hex
		let result = field.clean(Some(&json!("#FF0000")));
		assert!(result.is_ok());

		// 3-digit hex
		let result = field.clean(Some(&json!("#F00")));
		assert!(result.is_ok());

		// Lowercase
		let result = field.clean(Some(&json!("#ff0000")));
		assert!(result.is_ok());
	}

	#[test]
	fn test_color_field_invalid() {
		let field = ColorField::new("color");

		// Missing #
		let result = field.clean(Some(&json!("FF0000")));
		assert!(result.is_err());

		// Invalid length
		let result = field.clean(Some(&json!("#FF00")));
		assert!(result.is_err());

		// Invalid characters
		let result = field.clean(Some(&json!("#GGGGGG")));
		assert!(result.is_err());
	}

	#[test]
	fn test_password_field_basic() {
		let field = PasswordField::new("password").min_length(6);

		// Valid
		let result = field.clean(Some(&json!("password123")));
		assert!(result.is_ok());

		// Too short
		let result = field.clean(Some(&json!("pass")));
		assert!(result.is_err());
	}

	#[test]
	fn test_password_field_requirements() {
		let field = PasswordField::new("password")
			.min_length(8)
			.require_uppercase(true)
			.require_digit(true)
			.require_special(true);

		// Valid: has uppercase, digit, and special char
		let result = field.clean(Some(&json!("SecurePass123!")));
		assert!(result.is_ok());

		// Missing uppercase
		let result = field.clean(Some(&json!("password123!")));
		assert!(result.is_err());

		// Missing digit
		let result = field.clean(Some(&json!("SecurePassword!")));
		assert!(result.is_err());

		// Missing special char
		let result = field.clean(Some(&json!("SecurePassword123")));
		assert!(result.is_err());
	}
}
