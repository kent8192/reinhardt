use crate::field::{FieldError, FieldResult, FormField, Widget};
use chrono::NaiveTime;

/// TimeField for time input
pub struct TimeField {
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
	/// Accepted time format strings (strftime patterns).
	pub input_formats: Vec<String>,
}

impl TimeField {
	/// Create a new TimeField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::TimeField;
	///
	/// let field = TimeField::new("start_time".to_string());
	/// assert_eq!(field.name, "start_time");
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
				"%H:%M:%S".to_string(),
				"%H:%M".to_string(),
				"%I:%M:%S %p".to_string(),
				"%I:%M %p".to_string(),
			],
		}
	}

	fn parse_time(&self, s: &str) -> Result<NaiveTime, String> {
		for fmt in &self.input_formats {
			if let Ok(time) = NaiveTime::parse_from_str(s, fmt) {
				return Ok(time);
			}
		}
		Err("Enter a valid time".to_string())
	}
}

impl FormField for TimeField {
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

				let time = self.parse_time(s).map_err(FieldError::Validation)?;

				Ok(serde_json::Value::String(
					time.format("%H:%M:%S").to_string(),
				))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_timefield_valid() {
		let field = TimeField::new("start_time".to_string());

		assert_eq!(
			field.clean(Some(&serde_json::json!("14:30:00"))).unwrap(),
			serde_json::json!("14:30:00")
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("14:30"))).unwrap(),
			serde_json::json!("14:30:00")
		);
		assert_eq!(
			field
				.clean(Some(&serde_json::json!("02:30:00 PM")))
				.unwrap(),
			serde_json::json!("14:30:00")
		);
	}

	#[test]
	fn test_timefield_invalid() {
		let field = TimeField::new("start_time".to_string());

		assert!(matches!(
			field.clean(Some(&serde_json::json!("not a time"))),
			Err(FieldError::Validation(_))
		));
		assert!(matches!(
			field.clean(Some(&serde_json::json!("25:00:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_timefield_optional() {
		let mut field = TimeField::new("start_time".to_string());
		field.required = false;

		assert_eq!(field.clean(None).unwrap(), serde_json::Value::Null);
		assert_eq!(
			field.clean(Some(&serde_json::json!(""))).unwrap(),
			serde_json::Value::Null
		);
	}

	#[test]
	fn test_timefield_required() {
		let field = TimeField::new("start_time".to_string());

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_timefield_24hour_format_with_seconds() {
		let field = TimeField::new("start_time".to_string());

		let result = field.clean(Some(&serde_json::json!("14:30:00"))).unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));

		let result = field.clean(Some(&serde_json::json!("09:15:30"))).unwrap();
		assert_eq!(result, serde_json::json!("09:15:30"));
	}

	#[test]
	fn test_timefield_24hour_format_without_seconds() {
		let field = TimeField::new("start_time".to_string());

		// Without seconds defaults to :00
		let result = field.clean(Some(&serde_json::json!("14:30"))).unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));

		let result = field.clean(Some(&serde_json::json!("09:15"))).unwrap();
		assert_eq!(result, serde_json::json!("09:15:00"));
	}

	#[test]
	fn test_timefield_12hour_format_pm() {
		let field = TimeField::new("start_time".to_string());

		// PM times
		let result = field
			.clean(Some(&serde_json::json!("02:30:00 PM")))
			.unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));

		let result = field.clean(Some(&serde_json::json!("02:30 PM"))).unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));

		let result = field
			.clean(Some(&serde_json::json!("11:59:59 PM")))
			.unwrap();
		assert_eq!(result, serde_json::json!("23:59:59"));
	}

	#[test]
	fn test_timefield_12hour_format_am() {
		let field = TimeField::new("start_time".to_string());

		// AM times
		let result = field
			.clean(Some(&serde_json::json!("09:30:00 AM")))
			.unwrap();
		assert_eq!(result, serde_json::json!("09:30:00"));

		let result = field.clean(Some(&serde_json::json!("09:30 AM"))).unwrap();
		assert_eq!(result, serde_json::json!("09:30:00"));

		let result = field
			.clean(Some(&serde_json::json!("11:59:59 AM")))
			.unwrap();
		assert_eq!(result, serde_json::json!("11:59:59"));
	}

	#[test]
	fn test_timefield_midnight() {
		let field = TimeField::new("start_time".to_string());

		// Midnight as 00:00:00
		let result = field.clean(Some(&serde_json::json!("00:00:00"))).unwrap();
		assert_eq!(result, serde_json::json!("00:00:00"));

		// Midnight as 12:00 AM
		let result = field.clean(Some(&serde_json::json!("12:00 AM"))).unwrap();
		assert_eq!(result, serde_json::json!("00:00:00"));
	}

	#[test]
	fn test_timefield_noon() {
		let field = TimeField::new("start_time".to_string());

		// Noon as 12:00:00
		let result = field.clean(Some(&serde_json::json!("12:00:00"))).unwrap();
		assert_eq!(result, serde_json::json!("12:00:00"));

		// Noon as 12:00 PM
		let result = field.clean(Some(&serde_json::json!("12:00 PM"))).unwrap();
		assert_eq!(result, serde_json::json!("12:00:00"));
	}

	#[test]
	fn test_timefield_end_of_day() {
		let field = TimeField::new("start_time".to_string());

		let result = field.clean(Some(&serde_json::json!("23:59:59"))).unwrap();
		assert_eq!(result, serde_json::json!("23:59:59"));
	}

	#[test]
	fn test_timefield_whitespace_trimming() {
		let field = TimeField::new("start_time".to_string());

		let result = field
			.clean(Some(&serde_json::json!("  14:30:00  ")))
			.unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));
	}

	#[test]
	fn test_timefield_invalid_hour() {
		let field = TimeField::new("start_time".to_string());

		// Hour 25 is invalid
		assert!(matches!(
			field.clean(Some(&serde_json::json!("25:00:00"))),
			Err(FieldError::Validation(_))
		));

		// Hour 24 is invalid (valid range is 00-23)
		assert!(matches!(
			field.clean(Some(&serde_json::json!("24:00:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_timefield_invalid_minute() {
		let field = TimeField::new("start_time".to_string());

		// Minute 60 is invalid
		assert!(matches!(
			field.clean(Some(&serde_json::json!("14:60:00"))),
			Err(FieldError::Validation(_))
		));

		// Minute 99 is invalid
		assert!(matches!(
			field.clean(Some(&serde_json::json!("14:99:00"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_timefield_invalid_second() {
		let field = TimeField::new("start_time".to_string());

		// Second 99 is invalid
		assert!(matches!(
			field.clean(Some(&serde_json::json!("14:30:99"))),
			Err(FieldError::Validation(_))
		));

		// Note: Second 60 is accepted by chrono as leap second
		// So we test with a clearly invalid value like 99 instead
	}

	#[test]
	fn test_timefield_invalid_format() {
		let field = TimeField::new("start_time".to_string());

		// Missing colon
		assert!(matches!(
			field.clean(Some(&serde_json::json!("1430"))),
			Err(FieldError::Validation(_))
		));

		// Invalid text
		assert!(matches!(
			field.clean(Some(&serde_json::json!("not a time"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_timefield_widget_type() {
		let field = TimeField::new("start_time".to_string());
		assert!(matches!(field.widget(), &Widget::TextInput));
	}

	#[test]
	fn test_timefield_custom_format() {
		let mut field = TimeField::new("start_time".to_string());
		// Replace with custom 24-hour format using period
		field.input_formats.clear();
		field.input_formats.push("%H.%M.%S".to_string());

		// Custom format with periods should work
		let result = field.clean(Some(&serde_json::json!("14.30.00"))).unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));
	}

	#[test]
	fn test_timefield_format_precedence() {
		let field = TimeField::new("start_time".to_string());

		// When multiple formats could match, first matching format is used
		// "14:30:00" matches "%H:%M:%S" (first format)
		let result = field.clean(Some(&serde_json::json!("14:30:00"))).unwrap();
		assert_eq!(result, serde_json::json!("14:30:00"));
	}
}
