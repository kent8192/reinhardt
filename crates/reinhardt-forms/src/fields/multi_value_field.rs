use crate::field::{FieldError, FieldResult, FormField, Widget};
use chrono::NaiveDateTime;

/// MultiValueField combines multiple fields into one
pub struct MultiValueField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub fields: Vec<Box<dyn FormField>>,
	pub require_all_fields: bool,
}

impl MultiValueField {
	/// Create a new MultiValueField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::MultiValueField;
	/// use reinhardt_forms::fields::{CharField, IntegerField};
	/// use reinhardt_forms::FormField;
	/// use serde_json::json;
	///
	/// // Create a multi-value field combining name and age
	/// let fields: Vec<Box<dyn FormField>> = vec![
	///     Box::new(CharField::new("name".to_string())),
	///     Box::new(IntegerField::new("age".to_string())),
	/// ];
	///
	/// let field = MultiValueField::new("person".to_string(), fields);
	///
	/// // Valid: both values provided
	/// let result = field.clean(Some(&json!(["John Doe", 30])));
	/// assert!(result.is_ok());
	/// ```
	pub fn new(name: String, fields: Vec<Box<dyn FormField>>) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			fields,
			require_all_fields: true,
		}
	}
	pub fn compress(&self, values: Vec<serde_json::Value>) -> FieldResult<serde_json::Value> {
		// Default implementation: return array of values
		Ok(serde_json::Value::Array(values))
	}
}

impl FormField for MultiValueField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				let values = v
					.as_array()
					.ok_or_else(|| FieldError::invalid(None, "Expected array"))?;

				if values.is_empty() && self.required {
					return Err(FieldError::required(None));
				}

				if values.len() != self.fields.len() {
					return Err(FieldError::invalid(
						None,
						&format!("Expected {} values", self.fields.len()),
					));
				}

				let mut cleaned_values = Vec::new();
				for (idx, field) in self.fields.iter().enumerate() {
					let field_value = values.get(idx);

					match field.clean(field_value) {
						Ok(cleaned) => {
							if cleaned.is_null() && self.require_all_fields {
								return Err(FieldError::validation(
									None,
									"All fields are required",
								));
							}
							cleaned_values.push(cleaned);
						}
						Err(e) => return Err(e),
					}
				}

				self.compress(cleaned_values)
			}
		}
	}
}

/// SplitDateTimeField splits datetime input into separate date and time fields
pub struct SplitDateTimeField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub input_date_formats: Vec<String>,
	pub input_time_formats: Vec<String>,
}

impl SplitDateTimeField {
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			input_date_formats: vec![
				"%Y-%m-%d".to_string(),
				"%m/%d/%Y".to_string(),
				"%m/%d/%y".to_string(),
			],
			input_time_formats: vec![
				"%H:%M:%S".to_string(),
				"%H:%M".to_string(),
				"%I:%M:%S %p".to_string(),
				"%I:%M %p".to_string(),
			],
		}
	}

	fn parse_date(&self, s: &str) -> Result<chrono::NaiveDate, String> {
		for fmt in &self.input_date_formats {
			if let Ok(date) = chrono::NaiveDate::parse_from_str(s, fmt) {
				return Ok(date);
			}
		}
		Err("Enter a valid date".to_string())
	}

	fn parse_time(&self, s: &str) -> Result<chrono::NaiveTime, String> {
		for fmt in &self.input_time_formats {
			if let Ok(time) = chrono::NaiveTime::parse_from_str(s, fmt) {
				return Ok(time);
			}
		}
		Err("Enter a valid time".to_string())
	}
}

impl FormField for SplitDateTimeField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				// Expect an array with [date_string, time_string]
				let parts = v
					.as_array()
					.ok_or_else(|| FieldError::invalid(None, "Expected array of [date, time]"))?;

				if parts.len() != 2 {
					return Err(FieldError::invalid(None, "Expected [date, time]"));
				}

				let date_str = parts[0]
					.as_str()
					.ok_or_else(|| FieldError::invalid(None, "Date must be a string"))?;

				let time_str = parts[1]
					.as_str()
					.ok_or_else(|| FieldError::invalid(None, "Time must be a string"))?;

				if date_str.trim().is_empty() || time_str.trim().is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				let date = self
					.parse_date(date_str.trim())
					.map_err(|e| FieldError::validation(None, &e))?;

				let time = self
					.parse_time(time_str.trim())
					.map_err(|e| FieldError::validation(None, &e))?;

				let datetime = NaiveDateTime::new(date, time);

				Ok(serde_json::Value::String(
					datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
				))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fields::{CharField, IntegerField};

	#[test]
	fn test_multi_value_field() {
		let fields: Vec<Box<dyn FormField>> = vec![
			Box::new(CharField::new("first".to_string())),
			Box::new(IntegerField::new("second".to_string())),
		];

		let field = MultiValueField::new("combined".to_string(), fields);

		let value = serde_json::json!(["hello", 42]);
		let result = field.clean(Some(&value));
		assert!(result.is_ok());

		// Test with wrong number of values
		let wrong_value = serde_json::json!(["hello"]);
		assert!(matches!(
			field.clean(Some(&wrong_value)),
			Err(FieldError::Invalid(_))
		));
	}

	#[test]
	fn test_split_datetime_field() {
		let field = SplitDateTimeField::new("when".to_string());

		let value = serde_json::json!(["2025-01-15", "14:30:00"]);
		let result = field.clean(Some(&value)).unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));

		// Test with different formats
		let value2 = serde_json::json!(["01/15/2025", "02:30 PM"]);
		let result2 = field.clean(Some(&value2)).unwrap();
		assert_eq!(result2, serde_json::json!("2025-01-15 14:30:00"));
	}

	#[test]
	fn test_split_datetime_field_invalid() {
		let field = SplitDateTimeField::new("when".to_string());

		// Wrong format
		let value = serde_json::json!(["not-a-date", "14:30:00"]);
		assert!(matches!(
			field.clean(Some(&value)),
			Err(FieldError::Validation(_))
		));

		// Wrong structure
		let value2 = serde_json::json!(["2025-01-15"]);
		assert!(matches!(
			field.clean(Some(&value2)),
			Err(FieldError::Invalid(_))
		));
	}
}
