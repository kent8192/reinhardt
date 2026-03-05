use crate::field::{FieldError, FieldResult, FormField, Widget};
use chrono::{Datelike, NaiveDate};

/// DateField for date input
pub struct DateField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub input_formats: Vec<String>,
	pub localize: bool,
	pub locale: Option<String>,
}

impl DateField {
	/// Create a new DateField with the given name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::DateField;
	///
	/// let field = DateField::new("birth_date".to_string());
	/// assert_eq!(field.name, "birth_date");
	/// assert!(field.required);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::DateInput,
			initial: None,
			input_formats: vec![
				"%Y-%m-%d".to_string(),  // 2025-01-15
				"%m/%d/%Y".to_string(),  // 01/15/2025
				"%b %d %Y".to_string(),  // Jan 15 2025
				"%b %d, %Y".to_string(), // Jan 15, 2025
				"%d %b %Y".to_string(),  // 15 Jan 2025
				"%d %b, %Y".to_string(), // 15 Jan, 2025
				"%B %d %Y".to_string(),  // January 15 2025
				"%B %d, %Y".to_string(), // January 15, 2025
				"%d %B %Y".to_string(),  // 15 January 2025
				"%d %B, %Y".to_string(), // 15 January, 2025
			],
			localize: false,
			locale: None,
		}
	}
	/// Enable localization for this field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::DateField;
	///
	/// let field = DateField::new("date".to_string()).with_localize(true);
	/// assert!(field.localize);
	/// ```
	pub fn with_localize(mut self, localize: bool) -> Self {
		self.localize = localize;
		self
	}
	/// Set the locale for this field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::DateField;
	///
	/// let field = DateField::new("date".to_string()).with_locale("en_US".to_string());
	/// assert_eq!(field.locale, Some("en_US".to_string()));
	/// ```
	pub fn with_locale(mut self, locale: String) -> Self {
		self.locale = Some(locale);
		self
	}

	fn parse_date(&self, s: &str) -> Result<NaiveDate, String> {
		for format in &self.input_formats {
			if let Ok(date) = NaiveDate::parse_from_str(s, format) {
				// Reject dates with years outside the 4-digit range (1000-9999)
				// to prevent ambiguous 2-digit year interpretations.
				let year = date.year();
				if !(1000..=9999).contains(&year) {
					continue;
				}
				return Ok(date);
			}
		}
		Err("Enter a valid date with a 4-digit year".to_string())
	}
}

impl FormField for DateField {
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

				let date = self.parse_date(s).map_err(FieldError::Validation)?;

				// Return in ISO 8601 format
				Ok(serde_json::json!(date.format("%Y-%m-%d").to_string()))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_date_field_required() {
		let field = DateField::new("date".to_string());

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_date_field_not_required() {
		let mut field = DateField::new("date".to_string());
		field.required = false;

		// Not required accepts None
		assert_eq!(field.clean(None).unwrap(), serde_json::Value::Null);

		// Not required accepts empty string
		assert_eq!(
			field.clean(Some(&serde_json::json!(""))).unwrap(),
			serde_json::Value::Null
		);
	}

	#[test]
	fn test_date_field_iso_format() {
		let field = DateField::new("date".to_string());

		// Standard ISO 8601 format (YYYY-MM-DD)
		let result = field.clean(Some(&serde_json::json!("2025-01-15"))).unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));
	}

	#[test]
	fn test_date_field_us_format() {
		let field = DateField::new("date".to_string());

		// US format (MM/DD/YYYY)
		let result = field.clean(Some(&serde_json::json!("01/15/2025"))).unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));

		// 2-digit year format (MM/DD/YY) is rejected to avoid ambiguity
		assert!(field.clean(Some(&serde_json::json!("01/15/25"))).is_err());
	}

	#[test]
	fn test_date_field_month_name_formats() {
		let field = DateField::new("date".to_string());

		// Abbreviated month (Jan 15 2025)
		let result = field
			.clean(Some(&serde_json::json!("Jan 15 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));

		// Abbreviated month with comma (Jan 15, 2025)
		let result = field
			.clean(Some(&serde_json::json!("Jan 15, 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));

		// Full month name (January 15 2025)
		let result = field
			.clean(Some(&serde_json::json!("January 15 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));

		// Full month with comma (January 15, 2025)
		let result = field
			.clean(Some(&serde_json::json!("January 15, 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));
	}

	#[test]
	fn test_date_field_day_first_formats() {
		let field = DateField::new("date".to_string());

		// Day first with abbreviated month (15 Jan 2025)
		let result = field
			.clean(Some(&serde_json::json!("15 Jan 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));

		// Day first with full month (15 January 2025)
		let result = field
			.clean(Some(&serde_json::json!("15 January 2025")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));
	}

	#[test]
	fn test_date_field_invalid_format() {
		let field = DateField::new("date".to_string());

		// Invalid date format
		assert!(field.clean(Some(&serde_json::json!("not a date"))).is_err());
		assert!(field.clean(Some(&serde_json::json!("2025/13/01"))).is_err());
		assert!(field.clean(Some(&serde_json::json!("2025-00-01"))).is_err());
	}

	#[test]
	fn test_date_field_whitespace_trimming() {
		let field = DateField::new("date".to_string());

		// Should trim whitespace
		let result = field
			.clean(Some(&serde_json::json!("  2025-01-15  ")))
			.unwrap();
		assert_eq!(result, serde_json::json!("2025-01-15"));
	}

	#[test]
	fn test_date_field_invalid_dates() {
		let field = DateField::new("date".to_string());

		// Invalid month
		assert!(field.clean(Some(&serde_json::json!("2025-13-01"))).is_err());

		// Invalid day
		assert!(field.clean(Some(&serde_json::json!("2025-01-32"))).is_err());

		// February 30th doesn't exist
		assert!(field.clean(Some(&serde_json::json!("2025-02-30"))).is_err());
	}

	#[test]
	fn test_date_field_leap_year() {
		let field = DateField::new("date".to_string());

		// Feb 29 in leap year (2024)
		let result = field.clean(Some(&serde_json::json!("2024-02-29"))).unwrap();
		assert_eq!(result, serde_json::json!("2024-02-29"));

		// Feb 29 in non-leap year (2025)
		assert!(field.clean(Some(&serde_json::json!("2025-02-29"))).is_err());
	}

	#[test]
	fn test_date_field_localize() {
		let field = DateField::new("date".to_string()).with_localize(true);
		assert!(field.localize);
	}

	#[test]
	fn test_date_field_locale() {
		let field = DateField::new("date".to_string()).with_locale("en_US".to_string());
		assert_eq!(field.locale, Some("en_US".to_string()));
	}

	#[test]
	fn test_date_field_widget() {
		let field = DateField::new("date".to_string());
		assert!(matches!(field.widget(), &Widget::DateInput));
	}

	#[test]
	fn test_date_field_name() {
		let field = DateField::new("birth_date".to_string());
		assert_eq!(field.name(), "birth_date");
	}

	#[rstest]
	#[case("01/15/25")]
	#[case("12/31/99")]
	#[case("06/15/00")]
	fn test_date_field_rejects_two_digit_years(#[case] input: &str) {
		// Arrange
		let field = DateField::new("date".to_string());

		// Act
		let result = field.clean(Some(&serde_json::json!(input)));

		// Assert
		assert!(
			result.is_err(),
			"Expected 2-digit year input '{}' to be rejected, got: {:?}",
			input,
			result,
		);
	}

	#[rstest]
	#[case("01/15/2025", "2025-01-15")]
	#[case("12/31/1999", "1999-12-31")]
	#[case("2024-02-29", "2024-02-29")]
	fn test_date_field_accepts_four_digit_years(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		let field = DateField::new("date".to_string());

		// Act
		let result = field.clean(Some(&serde_json::json!(input)));

		// Assert
		assert_eq!(
			result.unwrap(),
			serde_json::json!(expected),
			"Expected 4-digit year input '{}' to parse as '{}'",
			input,
			expected,
		);
	}
}
