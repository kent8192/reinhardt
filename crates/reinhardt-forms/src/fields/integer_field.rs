//! Integer field

use crate::field::{FieldError, FieldResult, FormField, Widget};

/// Integer field with range validation
#[derive(Debug, Clone)]
pub struct IntegerField {
	/// The field name used as the form data key.
	pub name: String,
	/// Optional human-readable label for display.
	pub label: Option<String>,
	/// Whether this field must be filled in.
	pub required: bool,
	/// Optional help text displayed alongside the field.
	pub help_text: Option<String>,
	/// The widget type used for rendering this field.
	pub widget: Widget,
	/// Optional initial (default) value for the field.
	pub initial: Option<serde_json::Value>,
	/// Maximum allowed value.
	pub max_value: Option<i64>,
	/// Minimum allowed value.
	pub min_value: Option<i64>,
}

impl IntegerField {
	/// Create a new IntegerField with the given name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("age".to_string());
	/// assert_eq!(field.name, "age");
	/// assert!(!field.required);
	/// assert_eq!(field.min_value, None);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: false,
			help_text: None,
			widget: Widget::NumberInput,
			initial: None,
			max_value: None,
			min_value: None,
		}
	}
	/// Set the field as required
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("age".to_string()).required();
	/// assert!(field.required);
	/// ```
	pub fn required(mut self) -> Self {
		self.required = true;
		self
	}
	/// Set the minimum value for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("age".to_string()).with_min_value(0);
	/// assert_eq!(field.min_value, Some(0));
	/// ```
	pub fn with_min_value(mut self, min_value: i64) -> Self {
		self.min_value = Some(min_value);
		self
	}
	/// Set the maximum value for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("count".to_string()).with_max_value(100);
	/// assert_eq!(field.max_value, Some(100));
	/// ```
	pub fn with_max_value(mut self, max_value: i64) -> Self {
		self.max_value = Some(max_value);
		self
	}

	/// Set the label for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("age".to_string()).with_label("Age");
	/// assert_eq!(field.label, Some("Age".to_string()));
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
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("age".to_string()).with_help_text("Enter your age");
	/// assert_eq!(field.help_text, Some("Enter your age".to_string()));
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
	/// use reinhardt_forms::fields::IntegerField;
	///
	/// let field = IntegerField::new("quantity".to_string()).with_initial(1);
	/// assert_eq!(field.initial, Some(serde_json::json!(1)));
	/// ```
	pub fn with_initial(mut self, initial: i64) -> Self {
		self.initial = Some(serde_json::json!(initial));
		self
	}
}

// Note: Default trait is not implemented because IntegerField requires a name

impl FormField for IntegerField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				// Parse integer from either number or string
				let num = if let Some(n) = v.as_i64() {
					n
				} else if let Some(s) = v.as_str() {
					// Trim whitespace
					let s = s.trim();

					// Return None/error for empty string
					if s.is_empty() {
						if self.required {
							return Err(FieldError::Required(self.name.clone()));
						}
						return Ok(serde_json::Value::Null);
					}

					// Parse string to integer
					s.parse::<i64>()
						.map_err(|_| FieldError::Validation("Enter a whole number".to_string()))?
				} else {
					return Err(FieldError::Validation(
						"Expected integer or string".to_string(),
					));
				};

				// Validate range
				if let Some(max) = self.max_value
					&& num > max
				{
					return Err(FieldError::Validation(format!(
						"Ensure this value is less than or equal to {}",
						max
					)));
				}

				if let Some(min) = self.min_value
					&& num < min
				{
					return Err(FieldError::Validation(format!(
						"Ensure this value is greater than or equal to {}",
						min
					)));
				}

				Ok(serde_json::Value::Number(num.into()))
			}
		}
	}
}
