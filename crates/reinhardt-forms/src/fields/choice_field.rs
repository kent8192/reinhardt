use crate::field::{FieldError, FieldResult, FormField, Widget};

/// ChoiceField for selecting from predefined choices
pub struct ChoiceField {
	/// The field name used as the form data key.
	pub name: String,
	/// Optional human-readable label for display.
	pub label: Option<String>,
	/// Whether a selection is required.
	pub required: bool,
	/// Optional help text displayed alongside the field.
	pub help_text: Option<String>,
	/// The widget type used for rendering this field.
	pub widget: Widget,
	/// Optional initial (default) value for the field.
	pub initial: Option<serde_json::Value>,
	/// Available choices as (value, display_label) pairs.
	pub choices: Vec<(String, String)>,
}

impl ChoiceField {
	/// Create a new ChoiceField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::ChoiceField;
	///
	/// let choices = vec![("1".to_string(), "Option 1".to_string())];
	/// let field = ChoiceField::new("choice".to_string(), choices);
	/// assert_eq!(field.name, "choice");
	/// ```
	pub fn new(name: String, choices: Vec<(String, String)>) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::Select {
				choices: choices.clone(),
			},
			initial: None,
			choices,
		}
	}
}

impl FormField for ChoiceField {
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
			None if self.required => Err(FieldError::required(None)),
			None => Ok(serde_json::Value::String(String::new())),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::Invalid("Expected string".to_string()))?;

				let s = s.trim();

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::String(String::new()));
				}

				// Check if value is in choices
				let valid = self.choices.iter().any(|(value, _)| value == s);
				if !valid {
					return Err(FieldError::Validation(format!(
						"Select a valid choice. '{}' is not one of the available choices",
						s
					)));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}

/// MultipleChoiceField for selecting multiple choices
pub struct MultipleChoiceField {
	/// The field name used as the form data key.
	pub name: String,
	/// Optional human-readable label for display.
	pub label: Option<String>,
	/// Whether at least one selection is required.
	pub required: bool,
	/// Optional help text displayed alongside the field.
	pub help_text: Option<String>,
	/// The widget type used for rendering this field.
	pub widget: Widget,
	/// Optional initial (default) value for the field.
	pub initial: Option<serde_json::Value>,
	/// Available choices as (value, display_label) pairs.
	pub choices: Vec<(String, String)>,
}

impl MultipleChoiceField {
	/// Creates a new `MultipleChoiceField` with the given name and choices.
	pub fn new(name: String, choices: Vec<(String, String)>) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::Select {
				choices: choices.clone(),
			},
			initial: None,
			choices,
		}
	}
}

impl FormField for MultipleChoiceField {
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
			None if self.required => Err(FieldError::required(None)),
			None => Ok(serde_json::json!([])),
			Some(v) => {
				let values: Vec<String> = if let Some(arr) = v.as_array() {
					arr.iter()
						.filter_map(|v| v.as_str().map(|s| s.to_string()))
						.collect()
				} else if let Some(s) = v.as_str() {
					vec![s.to_string()]
				} else {
					return Err(FieldError::Invalid("Expected array or string".to_string()));
				};

				if values.is_empty() && self.required {
					return Err(FieldError::required(None));
				}

				// Validate all values are in choices
				for val in &values {
					let valid = self.choices.iter().any(|(choice, _)| choice == val);
					if !valid {
						return Err(FieldError::Validation(format!(
							"Select a valid choice. '{}' is not one of the available choices",
							val
						)));
					}
				}

				Ok(serde_json::json!(values))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_choicefield_valid() {
		let choices = vec![
			("1".to_string(), "One".to_string()),
			("2".to_string(), "Two".to_string()),
		];
		let field = ChoiceField::new("number".to_string(), choices);

		assert_eq!(
			field.clean(Some(&serde_json::json!("1"))).unwrap(),
			serde_json::json!("1")
		);
	}

	#[test]
	fn test_choicefield_invalid() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = ChoiceField::new("number".to_string(), choices);

		assert!(matches!(
			field.clean(Some(&serde_json::json!("3"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_multiplechoicefield() {
		let choices = vec![
			("a".to_string(), "A".to_string()),
			("b".to_string(), "B".to_string()),
		];
		let field = MultipleChoiceField::new("letters".to_string(), choices);

		assert_eq!(
			field.clean(Some(&serde_json::json!(["a", "b"]))).unwrap(),
			serde_json::json!(["a", "b"])
		);

		assert!(matches!(
			field.clean(Some(&serde_json::json!(["a", "c"]))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_choicefield_required() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = ChoiceField::new("number".to_string(), choices);

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_choicefield_not_required() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let mut field = ChoiceField::new("number".to_string(), choices);
		field.required = false;

		// Not required accepts None
		assert_eq!(field.clean(None).unwrap(), serde_json::json!(""));

		// Not required accepts empty string
		assert_eq!(
			field.clean(Some(&serde_json::json!(""))).unwrap(),
			serde_json::json!("")
		);
	}

	#[test]
	fn test_choicefield_whitespace_trimming() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = ChoiceField::new("number".to_string(), choices);

		// Whitespace should be trimmed before validation
		assert_eq!(
			field.clean(Some(&serde_json::json!("  1  "))).unwrap(),
			serde_json::json!("1")
		);
	}

	#[test]
	fn test_choicefield_multiple_choices() {
		let choices = vec![
			("a".to_string(), "Alpha".to_string()),
			("b".to_string(), "Beta".to_string()),
			("c".to_string(), "Gamma".to_string()),
		];
		let field = ChoiceField::new("greek".to_string(), choices);

		// All choices should be valid
		assert!(field.clean(Some(&serde_json::json!("a"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("b"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("c"))).is_ok());

		// Non-existent choice should fail
		assert!(field.clean(Some(&serde_json::json!("d"))).is_err());
	}

	#[test]
	fn test_choicefield_widget_type() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = ChoiceField::new("number".to_string(), choices.clone());

		// Widget should be Select with choices
		match field.widget() {
			Widget::Select {
				choices: widget_choices,
			} => {
				assert_eq!(widget_choices, &choices);
			}
			_ => panic!("Expected Select widget"),
		}
	}

	#[test]
	fn test_choicefield_empty_choices() {
		let choices: Vec<(String, String)> = vec![];
		let field = ChoiceField::new("empty".to_string(), choices);

		// Any value should be invalid when choices is empty
		assert!(matches!(
			field.clean(Some(&serde_json::json!("anything"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_choicefield_case_sensitive() {
		let choices = vec![("abc".to_string(), "ABC".to_string())];
		let field = ChoiceField::new("text".to_string(), choices);

		// Exact match should work
		assert!(field.clean(Some(&serde_json::json!("abc"))).is_ok());

		// Different case should fail (choices are case-sensitive)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("ABC"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_multiplechoicefield_required() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = MultipleChoiceField::new("numbers".to_string(), choices);

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty array
		assert!(field.clean(Some(&serde_json::json!([]))).is_err());
	}

	#[test]
	fn test_multiplechoicefield_not_required() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let mut field = MultipleChoiceField::new("numbers".to_string(), choices);
		field.required = false;

		// Not required accepts None
		assert_eq!(field.clean(None).unwrap(), serde_json::json!([]));

		// Not required accepts empty array
		assert_eq!(
			field.clean(Some(&serde_json::json!([]))).unwrap(),
			serde_json::json!([])
		);
	}

	#[test]
	fn test_multiplechoicefield_single_value() {
		let choices = vec![
			("a".to_string(), "A".to_string()),
			("b".to_string(), "B".to_string()),
		];
		let field = MultipleChoiceField::new("letters".to_string(), choices);

		// Single value as string should work
		assert_eq!(
			field.clean(Some(&serde_json::json!("a"))).unwrap(),
			serde_json::json!(["a"])
		);

		// Invalid single value should fail
		assert!(matches!(
			field.clean(Some(&serde_json::json!("z"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_multiplechoicefield_multiple_values() {
		let choices = vec![
			("1".to_string(), "One".to_string()),
			("2".to_string(), "Two".to_string()),
			("3".to_string(), "Three".to_string()),
		];
		let field = MultipleChoiceField::new("numbers".to_string(), choices);

		// Valid multiple values
		assert_eq!(
			field.clean(Some(&serde_json::json!(["1", "2"]))).unwrap(),
			serde_json::json!(["1", "2"])
		);

		assert_eq!(
			field
				.clean(Some(&serde_json::json!(["1", "2", "3"])))
				.unwrap(),
			serde_json::json!(["1", "2", "3"])
		);

		// One invalid value should fail entire validation
		assert!(matches!(
			field.clean(Some(&serde_json::json!(["1", "2", "4"]))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_multiplechoicefield_duplicate_values() {
		let choices = vec![
			("a".to_string(), "A".to_string()),
			("b".to_string(), "B".to_string()),
		];
		let field = MultipleChoiceField::new("letters".to_string(), choices);

		// Duplicates should be accepted (validation doesn't remove them)
		let result = field
			.clean(Some(&serde_json::json!(["a", "a", "b"])))
			.unwrap();
		assert_eq!(result, serde_json::json!(["a", "a", "b"]));
	}

	#[test]
	fn test_multiplechoicefield_widget_type() {
		let choices = vec![("1".to_string(), "One".to_string())];
		let field = MultipleChoiceField::new("numbers".to_string(), choices.clone());

		// Widget should be Select with choices
		match field.widget() {
			Widget::Select {
				choices: widget_choices,
			} => {
				assert_eq!(widget_choices, &choices);
			}
			_ => panic!("Expected Select widget"),
		}
	}
}
