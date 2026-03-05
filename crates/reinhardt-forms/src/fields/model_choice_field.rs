//! ModelChoiceField and ModelMultipleChoiceField for ORM integration

use crate::Widget;
use crate::field::{FieldError, FieldResult, FormField};
use crate::model_form::FormModel;
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

/// A field for selecting a single model instance from a queryset
///
/// This field displays model instances as choices in a select widget.
pub struct ModelChoiceField<T: FormModel> {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
	pub queryset: Vec<T>,
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
	pub fn empty_label(mut self, label: Option<String>) -> Self {
		self.empty_label = label;
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
pub struct ModelMultipleChoiceField<T: FormModel> {
	pub name: String,
	pub required: bool,
	pub error_messages: HashMap<String, String>,
	pub widget: Widget,
	pub help_text: String,
	pub initial: Option<Value>,
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
				a.iter().zip(b.iter()).any(|(x, y)| x != y)
			}
			(Some(a), Some(b)) => a != b,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::FormField;
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
}
