//! ModelChoiceField and ModelMultipleChoiceField for ORM integration

use crate::Widget;
use crate::field::{FieldError, FieldResult, FormField};
use crate::model_form::FormModel;
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

fn choice_value_text(value: &Value) -> String {
	match value {
		Value::String(value) => format!("id:{value}"),
		Value::Number(value) => format!("id:{value}"),
		Value::Bool(value) => format!("bool:{value}"),
		Value::Null => "null:".to_string(),
		Value::Array(value) => format!("array:{}", Value::Array(value.clone())),
		Value::Object(value) => format!("object:{}", Value::Object(value.clone())),
	}
}

/// A field for selecting a single model instance from a queryset
///
/// This field displays model instances as choices in a select widget.
pub struct ModelChoiceField<T: FormModel> {
	/// The field name used as the form data key.
	pub name: String,
	/// Whether a selection is required.
	pub required: bool,
	/// Custom error messages keyed by error type.
	pub error_messages: HashMap<String, String>,
	/// The widget type used for rendering this field.
	pub widget: Widget,
	/// Help text displayed alongside the field.
	pub help_text: String,
	/// Optional initial (default) value for the field.
	pub initial: Option<Value>,
	/// The list of model instances to choose from.
	pub queryset: Vec<T>,
	/// Label for the empty/default option (e.g., "Select one...").
	pub empty_label: Option<String>,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelChoiceField<T> {
	/// Create a new ModelChoiceField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::ModelChoiceField;
	/// use reinhardt_forms::FormField;
	/// use reinhardt_forms::FormModel;
	/// use serde_json::{json, Value};
	///
	/// // Define a simple Category model
	/// #[derive(Clone)]
	/// struct Category {
	///     id: i32,
	///     name: String,
	/// }
	///
	/// impl FormModel for Category {
	///     fn field_names() -> Vec<String> {
	///         vec!["id".to_string(), "name".to_string()]
	///     }
	///
	///     fn get_field(&self, name: &str) -> Option<Value> {
	///         match name {
	///             "id" => Some(json!(self.id)),
	///             "name" => Some(json!(self.name)),
	///             _ => None,
	///         }
	///     }
	///
	///     fn set_field(&mut self, _name: &str, _value: Value) -> Result<(), String> {
	///         Ok(())
	///     }
	///
	///     fn save(&mut self) -> Result<(), String> {
	///         Ok(())
	///     }
	/// }
	///
	/// // Create a queryset with sample categories
	/// let categories = vec![
	///     Category { id: 1, name: "Technology".to_string() },
	///     Category { id: 2, name: "Science".to_string() },
	/// ];
	///
	/// let field = ModelChoiceField::new("category", categories);
	/// assert_eq!(field.name(), "category");
	/// assert!(FormField::required(&field));
	/// ```
	pub fn new(name: impl Into<String>, queryset: Vec<T>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert(
			"invalid_choice".to_string(),
			"Select a valid choice.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::Select {
				choices: Vec::new(),
			},
			help_text: String::new(),
			initial: None,
			queryset,
			empty_label: Some("--------".to_string()),
			_phantom: PhantomData,
		}
	}
	/// Sets whether a selection is required.
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}
	/// Sets the help text displayed alongside the field.
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}
	/// Sets the initial (default) value.
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}
	/// Sets the label for the empty/default option.
	pub fn empty_label(mut self, label: Option<String>) -> Self {
		self.empty_label = label;
		self
	}
	/// Overrides the error message for a specific error type.
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}

	/// Get choices from queryset
	/// Converts model instances to (value, label) pairs for display in select widget
	// Allow dead_code: API reserved for future widget rendering integration
	#[allow(dead_code)]
	fn get_choices(&self) -> Vec<(String, String)> {
		let mut choices = Vec::new();

		if !self.required && self.empty_label.is_some() {
			choices.push(("".to_string(), self.empty_label.clone().unwrap()));
		}

		// Convert queryset items to choices
		for instance in &self.queryset {
			let value = instance.to_choice_value();
			let label = instance.to_choice_label();
			choices.push((value, label));
		}

		choices
	}
}

impl<T: FormModel> FormField for ModelChoiceField<T> {
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
			Value::String(s) => s.as_str(),
			Value::Number(n) => {
				// Convert number to string for validation
				&n.to_string()
			}
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid_choice")
					.cloned()
					.unwrap_or_else(|| "Select a valid choice.".to_string());
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

		// Validate that the choice exists in queryset
		let choice_exists = self
			.queryset
			.iter()
			.any(|instance| instance.to_choice_value() == s);

		if !choice_exists {
			let error_msg = self
				.error_messages
				.get("invalid_choice")
				.cloned()
				.unwrap_or_else(|| "Select a valid choice.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		Ok(Value::String(s.to_string()))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(a), Some(b)) => a != b,
		}
	}
}

/// A field for selecting multiple model instances from a queryset
///
/// This field displays model instances as choices in a multiple select widget.
/// [`Form::has_changed`](crate::Form::has_changed) treats selected arrays as
/// unordered. Numeric IDs and strings with the same textual representation are
/// normalized to the same key (for example, `1` and `"1"`), while booleans,
/// nulls, arrays, and objects retain distinct JSON type tags.
pub struct ModelMultipleChoiceField<T: FormModel> {
	/// The field name used as the form data key.
	pub name: String,
	/// Whether at least one selection is required.
	pub required: bool,
	/// Custom error messages keyed by error type.
	pub error_messages: HashMap<String, String>,
	/// The widget type used for rendering this field.
	pub widget: Widget,
	/// Help text displayed alongside the field.
	pub help_text: String,
	/// Optional initial (default) value for the field.
	pub initial: Option<Value>,
	/// The list of model instances to choose from.
	pub queryset: Vec<T>,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelMultipleChoiceField<T> {
	/// Create a new ModelMultipleChoiceField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::ModelMultipleChoiceField;
	/// use reinhardt_forms::FormField;
	/// use reinhardt_forms::FormModel;
	/// use serde_json::{json, Value};
	///
	/// // Define a simple Tag model
	/// #[derive(Clone)]
	/// struct Tag {
	///     id: i32,
	///     name: String,
	/// }
	///
	/// impl FormModel for Tag {
	///     fn field_names() -> Vec<String> {
	///         vec!["id".to_string(), "name".to_string()]
	///     }
	///
	///     fn get_field(&self, name: &str) -> Option<Value> {
	///         match name {
	///             "id" => Some(json!(self.id)),
	///             "name" => Some(json!(self.name)),
	///             _ => None,
	///         }
	///     }
	///
	///     fn set_field(&mut self, _name: &str, _value: Value) -> Result<(), String> {
	///         Ok(())
	///     }
	///
	///     fn save(&mut self) -> Result<(), String> {
	///         Ok(())
	///     }
	/// }
	///
	/// // Create a queryset with sample tags
	/// let tags = vec![
	///     Tag { id: 1, name: "rust".to_string() },
	///     Tag { id: 2, name: "programming".to_string() },
	///     Tag { id: 3, name: "web".to_string() },
	/// ];
	///
	/// let field = ModelMultipleChoiceField::new("tags", tags);
	/// assert_eq!(field.name(), "tags");
	/// assert!(FormField::required(&field));
	///
	/// // Test with multiple selections
	/// let result = field.clean(Some(&json!(["1", "2"])));
	/// assert!(result.is_ok());
	/// ```
	pub fn new(name: impl Into<String>, queryset: Vec<T>) -> Self {
		let mut error_messages = HashMap::new();
		error_messages.insert(
			"required".to_string(),
			"This field is required.".to_string(),
		);
		error_messages.insert(
			"invalid_choice".to_string(),
			"Select a valid choice.".to_string(),
		);
		error_messages.insert(
			"invalid_list".to_string(),
			"Enter a list of values.".to_string(),
		);

		Self {
			name: name.into(),
			required: true,
			error_messages,
			widget: Widget::Select {
				choices: Vec::new(),
			},
			help_text: String::new(),
			initial: None,
			queryset,
			_phantom: PhantomData,
		}
	}
	/// Sets whether at least one selection is required.
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}
	/// Sets the help text displayed alongside the field.
	pub fn help_text(mut self, text: impl Into<String>) -> Self {
		self.help_text = text.into();
		self
	}
	/// Sets the initial (default) value.
	pub fn initial(mut self, value: Value) -> Self {
		self.initial = Some(value);
		self
	}
	/// Overrides the error message for a specific error type.
	pub fn error_message(
		mut self,
		error_type: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		self.error_messages
			.insert(error_type.into(), message.into());
		self
	}

	/// Get choices from queryset
	// Allow dead_code: API reserved for future widget rendering integration
	#[allow(dead_code)]
	fn get_choices(&self) -> Vec<(String, String)> {
		let mut choices = Vec::new();

		// Convert queryset items to choices
		for instance in &self.queryset {
			let value = instance.to_choice_value();
			let label = instance.to_choice_label();
			choices.push((value, label));
		}

		choices
	}
}

impl<T: FormModel> FormField for ModelMultipleChoiceField<T> {
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
			return Ok(Value::Array(Vec::new()));
		}

		let values = match value.unwrap() {
			Value::Array(arr) => arr.clone(),
			Value::String(s) if s.is_empty() => {
				if self.required {
					let error_msg = self
						.error_messages
						.get("required")
						.cloned()
						.unwrap_or_else(|| "This field is required.".to_string());
					return Err(FieldError::validation(None, &error_msg));
				}
				return Ok(Value::Array(Vec::new()));
			}
			Value::String(s) => {
				// Split comma-separated values
				s.split(',')
					.map(|v| Value::String(v.trim().to_string()))
					.collect()
			}
			_ => {
				let error_msg = self
					.error_messages
					.get("invalid_list")
					.cloned()
					.unwrap_or_else(|| "Enter a list of values.".to_string());
				return Err(FieldError::validation(None, &error_msg));
			}
		};

		if values.is_empty() && self.required {
			let error_msg = self
				.error_messages
				.get("required")
				.cloned()
				.unwrap_or_else(|| "This field is required.".to_string());
			return Err(FieldError::validation(None, &error_msg));
		}

		// Validate that all choices exist in queryset
		for value in &values {
			if let Some(value_str) = value.as_str() {
				let choice_exists = self
					.queryset
					.iter()
					.any(|instance| instance.to_choice_value() == value_str);

				if !choice_exists {
					let error_msg = self
						.error_messages
						.get("invalid_choice")
						.cloned()
						.unwrap_or_else(|| format!("'{}' is not a valid choice.", value_str));
					return Err(FieldError::validation(None, &error_msg));
				}
			}
		}

		Ok(Value::Array(values))
	}

	fn has_changed(&self, initial: Option<&Value>, data: Option<&Value>) -> bool {
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(Value::Array(a)), Some(Value::Array(b))) => {
				if a.len() != b.len() {
					return true;
				}

				let mut initial_values: Vec<_> = a.iter().map(choice_value_text).collect();
				let mut submitted_values: Vec<_> = b.iter().map(choice_value_text).collect();
				initial_values.sort_unstable();
				submitted_values.sort_unstable();
				initial_values != submitted_values
			}
			(Some(a), Some(b)) => a != b,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::FormField;
	use rstest::rstest;
	use serde_json::json;

	// Mock model for testing
	struct TestModel {
		id: i32,
		name: String,
	}

	impl FormModel for TestModel {
		fn field_names() -> Vec<String> {
			vec!["id".to_string(), "name".to_string()]
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => Some(Value::Number(self.id.into())),
				"name" => Some(Value::String(self.name.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, _name: &str, _value: Value) -> Result<(), String> {
			Ok(())
		}

		fn save(&mut self) -> Result<(), String> {
			Ok(())
		}
	}

	struct StringKeyModel {
		id: String,
		name: String,
	}

	impl FormModel for StringKeyModel {
		fn field_names() -> Vec<String> {
			vec!["id".to_string(), "name".to_string()]
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => Some(Value::String(self.id.clone())),
				"name" => Some(Value::String(self.name.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, _name: &str, _value: Value) -> Result<(), String> {
			Ok(())
		}

		fn save(&mut self) -> Result<(), String> {
			Ok(())
		}
	}

	#[test]
	fn test_model_choice_field_basic() {
		let queryset = vec![
			TestModel {
				id: 1,
				name: "Option 1".to_string(),
			},
			TestModel {
				id: 2,
				name: "Option 2".to_string(),
			},
		];

		let field = ModelChoiceField::new("choice", queryset);

		assert_eq!(field.name(), "choice");
		assert!(FormField::required(&field));
	}

	#[test]
	fn test_model_choice_field_required() {
		let field = ModelChoiceField::new("choice", Vec::<TestModel>::new());

		let result = field.clean(None);
		assert!(result.is_err());
	}

	#[test]
	fn test_model_choice_field_not_required() {
		let field = ModelChoiceField::new("choice", Vec::<TestModel>::new()).required(false);

		let result = field.clean(None);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Value::Null);
	}

	#[test]
	fn test_model_multiple_choice_field_basic() {
		let queryset = vec![
			TestModel {
				id: 1,
				name: "Option 1".to_string(),
			},
			TestModel {
				id: 2,
				name: "Option 2".to_string(),
			},
		];

		let field = ModelMultipleChoiceField::new("choices", queryset);

		assert_eq!(field.name(), "choices");
		assert!(FormField::required(&field));
	}

	#[test]
	fn test_model_multiple_choice_field_array() {
		let queryset = vec![
			TestModel {
				id: 1,
				name: "Option 1".to_string(),
			},
			TestModel {
				id: 2,
				name: "Option 2".to_string(),
			},
			TestModel {
				id: 3,
				name: "Option 3".to_string(),
			},
		];

		let field = ModelMultipleChoiceField::new("choices", queryset).required(false);

		let result = field.clean(Some(&json!(["1", "2"])));
		assert!(result.is_ok());

		if let Value::Array(arr) = result.unwrap() {
			assert_eq!(arr.len(), 2);
		} else {
			panic!("Expected array");
		}
	}

	#[test]
	fn test_model_multiple_choice_field_comma_separated() {
		let queryset = vec![
			TestModel {
				id: 1,
				name: "Option 1".to_string(),
			},
			TestModel {
				id: 2,
				name: "Option 2".to_string(),
			},
			TestModel {
				id: 3,
				name: "Option 3".to_string(),
			},
		];

		let field = ModelMultipleChoiceField::new("choices", queryset).required(false);

		let result = field.clean(Some(&json!("1,2,3")));
		assert!(result.is_ok());

		if let Value::Array(arr) = result.unwrap() {
			assert_eq!(arr.len(), 3);
		} else {
			panic!("Expected array");
		}
	}

	#[rstest]
	fn model_choice_field_validates_supported_input_shapes() {
		// Arrange
		let single = ModelChoiceField::new(
			"choice",
			vec![
				TestModel {
					id: 1,
					name: "One".to_string(),
				},
				TestModel {
					id: 2,
					name: "Two".to_string(),
				},
			],
		)
		.error_message("invalid_choice", "Unknown choice.");
		// Act and assert
		assert_eq!(single.clean(Some(&json!("1"))).unwrap(), json!("1"));
		assert_eq!(single.clean(Some(&json!(2))).unwrap(), json!("2"));
		assert_eq!(
			single.clean(Some(&json!("99"))).unwrap_err().to_string(),
			"Unknown choice.",
		);
		assert_eq!(
			single.clean(Some(&json!(["1"]))).unwrap_err().to_string(),
			"Unknown choice.",
		);
		assert_eq!(
			single.clean(None).unwrap_err().to_string(),
			"This field is required.",
		);
		assert_eq!(
			ModelChoiceField::new("choice", Vec::<TestModel>::new())
				.required(false)
				.clean(Some(&json!("")))
				.unwrap(),
			Value::Null,
		);

		let string_single = ModelChoiceField::new(
			"choice",
			vec![StringKeyModel {
				id: "alpha-01".to_string(),
				name: "Alpha".to_string(),
			}],
		);
		assert_eq!(
			string_single.clean(Some(&json!("alpha-01"))).unwrap(),
			json!("alpha-01"),
		);
	}

	#[rstest]
	fn model_multiple_choice_field_validates_supported_input_shapes() {
		// Arrange
		let multiple = ModelMultipleChoiceField::new(
			"choices",
			vec![
				TestModel {
					id: 1,
					name: "One".to_string(),
				},
				TestModel {
					id: 2,
					name: "Two".to_string(),
				},
			],
		)
		.error_message("invalid_choice", "Unknown choice.")
		.error_message("invalid_list", "Values must be a list.");

		// Act and assert
		assert_eq!(
			multiple.clean(Some(&json!("1, 2"))).unwrap(),
			json!(["1", "2"])
		);
		assert_eq!(
			multiple.clean(Some(&json!(["2", "1"]))).unwrap(),
			json!(["2", "1"])
		);
		assert_eq!(
			multiple
				.clean(Some(&json!(["1", "99"])))
				.unwrap_err()
				.to_string(),
			"Unknown choice.",
		);
		assert_eq!(
			multiple.clean(Some(&json!(2))).unwrap_err().to_string(),
			"Values must be a list.",
		);
		assert_eq!(
			multiple.clean(None).unwrap_err().to_string(),
			"This field is required.",
		);
		assert_eq!(
			ModelMultipleChoiceField::new("choices", Vec::<TestModel>::new())
				.required(false)
				.clean(Some(&json!("")))
				.unwrap(),
			json!([]),
		);
		assert!(!multiple.has_changed(Some(&json!(["1", "2"])), Some(&json!(["1", "2"]))));
		assert!(!multiple.has_changed(Some(&json!(["1", "2"])), Some(&json!(["2", "1"]))));
		assert!(!multiple.has_changed(Some(&json!([1, 2])), Some(&json!(["1", "2"]))));
		assert!(multiple.has_changed(Some(&json!(["true"])), Some(&json!([true]))));
		assert!(multiple.has_changed(Some(&json!(["null"])), Some(&json!([null]))));
		assert!(multiple.has_changed(Some(&json!(["1", "1"])), Some(&json!(["1"]))));

		let string_multiple = ModelMultipleChoiceField::new(
			"choices",
			vec![
				StringKeyModel {
					id: "alpha-01".to_string(),
					name: "Alpha".to_string(),
				},
				StringKeyModel {
					id: "beta-02".to_string(),
					name: "Beta".to_string(),
				},
			],
		);
		assert_eq!(
			string_multiple
				.clean(Some(&json!("beta-02, alpha-01")))
				.unwrap(),
			json!(["beta-02", "alpha-01"]),
		);
	}
}
