use crate::field::{FieldError, FieldResult, FormField, Widget};
use chrono::NaiveDateTime;

/// DateTimeField for date and time input
pub struct DateTimeField {
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
	/// Accepted datetime format strings (strftime patterns).
	pub input_formats: Vec<String>,
}

impl DateTimeField {
	/// Create a new DateTimeField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::DateTimeField;
	///
	/// let field = DateTimeField::new("event_time".to_string());
	/// assert_eq!(field.name, "event_time");
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			input_formats: vec![
				"%Y-%m-%d %H:%M:%S".to_string(),
				"%Y-%m-%d %H:%M".to_string(),
				"%Y-%m-%dT%H:%M:%S".to_string(),
				"%Y-%m-%dT%H:%M".to_string(),
				"%m/%d/%Y %H:%M:%S".to_string(),
				"%m/%d/%Y %H:%M".to_string(),
				"%m/%d/%y %H:%M:%S".to_string(),
				"%m/%d/%y %H:%M".to_string(),
			],
		}
	}

	fn parse_datetime(&self, s: &str) -> Result<NaiveDateTime, String> {
		for fmt in &self.input_formats {
			if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
				return Ok(dt);
			}
		}
		Err("Enter a valid date/time".to_string())
	}
}

impl FormField for DateTimeField {
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
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::Invalid("Expected string".to_string()))?;

				let s = s.trim();

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				let dt = self.parse_datetime(s).map_err(FieldError::Validation)?;

				Ok(serde_json::Value::String(
					dt.format("%Y-%m-%d %H:%M:%S").to_string(),
				))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_datetimefield_valid() {
		let field = DateTimeField::new("created_at".to_string());

		assert_eq!(
			field
				.clean(Some(&serde_json::json!("2025-01-15 14:30:00")))
				.unwrap(),
			serde_json::json!("2025-01-15 14:30:00")
		);
		assert_eq!(
			field
				.clean(Some(&serde_json::json!("2025-01-15T14:30:00")))
				.unwrap(),
			serde_json::json!("2025-01-15 14:30:00")
		);
		assert_eq!(
			field
				.clean(Some(&serde_json::json!("01/15/2025 14:30:00")))
				.unwrap(),
			serde_json::json!("2025-01-15 14:30:00")
		);
	}

	#[test]
	fn test_datetimefield_invalid() {
		let field = DateTimeField::new("created_at".to_string());

		assert!(matches!(
			field.clean(Some(&serde_json::json!("not a datetime"))),
			Err(FieldError::Validation(_))
		));
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-13-01 14:30:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_datetimefield_optional() {
		let mut field = DateTimeField::new("created_at".to_string());
		field.required = false;

		assert_eq!(field.clean(None).unwrap(), serde_json::Value::Null);
		assert_eq!(
			field.clean(Some(&serde_json::json!(""))).unwrap(),
			serde_json::Value::Null
		);
	}

	#[test]
	fn test_datetimefield_required() {
		let field = DateTimeField::new("created_at".to_string());

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_datetimefield_iso_format_with_seconds() {
		let field = DateTimeField::new("created_at".to_string());

		// ISO 8601 with space separator
		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));

		// ISO 8601 with T separator
		let result = field
			.clean(Some(&serde_json::json!("2025-01-15T14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_iso_format_without_seconds() {
		let field = DateTimeField::new("created_at".to_string());

		// ISO 8601 with space separator (no seconds)
		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 14:30")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));

		// ISO 8601 with T separator (no seconds)
		let result = field
			.clean(Some(&serde_json::json!("2025-01-15T14:30")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_us_format_with_seconds() {
		let field = DateTimeField::new("created_at".to_string());

		// US format with 4-digit year
		let result = field
			.clean(Some(&serde_json::json!("01/15/2025 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));

		// US format with 2-digit year (chrono interprets as 00-99 AD)
		let result = field
			.clean(Some(&serde_json::json!("01/15/25 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("0025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_us_format_without_seconds() {
		let field = DateTimeField::new("created_at".to_string());

		// US format with 4-digit year (no seconds)
		let result = field
			.clean(Some(&serde_json::json!("01/15/2025 14:30")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));

		// US format with 2-digit year (no seconds)
		let result = field
			.clean(Some(&serde_json::json!("01/15/25 14:30")))
			.unwrap();
		assert_eq!(result, serde_json::json!("0025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_whitespace_trimming() {
		let field = DateTimeField::new("created_at".to_string());

		let result = field
			.clean(Some(&serde_json::json!("  2025-01-15 14:30:00  ")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_invalid_date() {
		let field = DateTimeField::new("created_at".to_string());

		// Invalid month (13)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-13-01 14:30:00"))),
			Err(FieldError::Validation(_))
		));

		// Invalid day (32)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-01-32 14:30:00"))),
			Err(FieldError::Validation(_))
		));

		// Feb 30 (invalid)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-02-30 14:30:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_datetimefield_invalid_time() {
		let field = DateTimeField::new("created_at".to_string());

		// Invalid hour (25)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-01-15 25:30:00"))),
			Err(FieldError::Validation(_))
		));

		// Invalid minute (61)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-01-15 14:61:00"))),
			Err(FieldError::Validation(_))
		));

		// Invalid second (61)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-01-15 14:30:61"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_datetimefield_leap_year() {
		let field = DateTimeField::new("created_at".to_string());

		// Feb 29 in leap year 2024 should be valid
		let result = field
			.clean(Some(&serde_json::json!("2024-02-29 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2024-02-29 14:30:00"));

		// Feb 29 in non-leap year 2025 should fail
		assert!(matches!(
			field.clean(Some(&serde_json::json!("2025-02-29 14:30:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_datetimefield_midnight() {
		let field = DateTimeField::new("created_at".to_string());

		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 00:00:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 00:00:00"));
	}

	#[test]
	fn test_datetimefield_end_of_day() {
		let field = DateTimeField::new("created_at".to_string());

		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 23:59:59")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 23:59:59"));
	}

	#[test]
	fn test_datetimefield_noon() {
		let field = DateTimeField::new("created_at".to_string());

		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 12:00:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 12:00:00"));
	}

	#[test]
	fn test_datetimefield_widget_type() {
		let field = DateTimeField::new("created_at".to_string());
		assert!(matches!(field.widget(), &Widget::TextInput));
	}

	#[test]
	fn test_datetimefield_custom_formats() {
		let mut field = DateTimeField::new("created_at".to_string());
		// Add custom format
		field.input_formats.push("%d-%m-%Y %H:%M:%S".to_string());

		// Custom day-first format should work
		let result = field
			.clean(Some(&serde_json::json!("15-01-2025 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));
	}

	#[test]
	fn test_datetimefield_format_precedence() {
		let field = DateTimeField::new("created_at".to_string());

		// When multiple formats could match, first matching format is used
		// "2025-01-15 14:30:00" matches "%Y-%m-%d %H:%M:%S" (first format)
		let result = field
			.clean(Some(&serde_json::json!("2025-01-15 14:30:00")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15 14:30:00"));
	}
}
