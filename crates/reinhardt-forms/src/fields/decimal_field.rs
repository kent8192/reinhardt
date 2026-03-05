use crate::field::{FieldError, FieldResult, FormField, Widget};
use std::str::FromStr;

/// DecimalField for decimal number input with digit and precision validation.
///
/// **Precision Note**: This field stores values internally as `f64` (IEEE 754),
/// which provides approximately 15-17 significant decimal digits of precision.
/// All digit count and decimal place validations are performed on the **string
/// representation** before conversion to `f64`, ensuring accurate constraint
/// enforcement even for values that cannot be exactly represented in binary
/// floating-point.
///
/// For applications requiring exact decimal arithmetic (e.g., financial
/// calculations), consider using `rust_decimal::Decimal` in your application
/// layer after form validation.
pub struct DecimalField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_value: Option<f64>,
	pub min_value: Option<f64>,
	pub max_digits: Option<usize>,
	pub decimal_places: Option<usize>,
	pub localize: bool,
	pub locale: Option<String>,
	pub use_thousands_separator: bool,
}

impl DecimalField {
	/// Create a new DecimalField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::DecimalField;
	///
	/// let field = DecimalField::new("price".to_string());
	/// assert_eq!(field.name, "price");
	/// assert!(field.required);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::NumberInput,
			initial: None,
			max_value: None,
			min_value: None,
			max_digits: None,
			decimal_places: None,
			localize: false,
			locale: None,
			use_thousands_separator: false,
		}
	}
	pub fn with_localize(mut self, localize: bool) -> Self {
		self.localize = localize;
		self
	}
	pub fn with_locale(mut self, locale: String) -> Self {
		self.locale = Some(locale);
		self
	}
	pub fn with_thousands_separator(mut self, use_separator: bool) -> Self {
		self.use_thousands_separator = use_separator;
		self
	}

	fn validate_decimal(&self, s: &str) -> Result<f64, String> {
		let num = f64::from_str(s).map_err(|_| "Enter a number".to_string())?;

		if !num.is_finite() {
			return Err("Enter a valid number".to_string());
		}

		// Reject leading zeros (e.g., "007", "00.5") to avoid ambiguous input.
		// Allowed patterns: "0", "0.5", "-0", "-0.5"
		let integer_part = s.trim_start_matches('-');
		let digits_before_dot = integer_part.split('.').next().unwrap_or(integer_part);
		if digits_before_dot.len() > 1 && digits_before_dot.starts_with('0') {
			return Err("Enter a number without leading zeros".to_string());
		}

		// Check total digits
		if let Some(max_digits) = self.max_digits {
			let parts: Vec<&str> = s.split('.').collect();
			let total_digits =
				parts[0].trim_start_matches('-').len() + parts.get(1).map(|p| p.len()).unwrap_or(0);
			if total_digits > max_digits {
				return Err(format!(
					"Ensure that there are no more than {} digits in total",
					max_digits
				));
			}
		}

		// Check decimal places
		if let Some(decimal_places) = self.decimal_places {
			let parts: Vec<&str> = s.split('.').collect();
			if let Some(decimals) = parts.get(1)
				&& decimals.len() > decimal_places
			{
				return Err(format!(
					"Ensure that there are no more than {} decimal places",
					decimal_places
				));
			}
		}

		Ok(num)
	}
}

impl FormField for DecimalField {
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
				// Convert to string representation for precise validation,
				// then parse to f64 for range checks. This ensures digit count
				// and decimal place validation is done on the original string
				// rather than on a potentially imprecise f64 representation.
				let (num, str_repr) = if let Some(s) = v.as_str() {
					let s = s.trim();

					if s.is_empty() {
						if self.required {
							return Err(FieldError::required(None));
						}
						return Ok(serde_json::Value::Null);
					}

					let n = self.validate_decimal(s).map_err(FieldError::Validation)?;
					(n, s.to_string())
				} else if let Some(f) = v.as_f64() {
					if !f.is_finite() {
						return Err(FieldError::Validation("Enter a valid number".to_string()));
					}
					(f, format!("{}", f))
				} else if let Some(i) = v.as_i64() {
					(i as f64, format!("{}", i))
				} else {
					return Err(FieldError::Invalid("Expected number or string".to_string()));
				};

				// Validate digit/decimal constraints on string representation
				// when value was provided as a number (string inputs are already
				// validated in validate_decimal)
				if !v.is_string() {
					if let Some(max_digits) = self.max_digits {
						let parts: Vec<&str> = str_repr.split('.').collect();
						let total_digits = parts[0].trim_start_matches('-').len()
							+ parts.get(1).map(|p| p.len()).unwrap_or(0);
						if total_digits > max_digits {
							return Err(FieldError::Validation(format!(
								"Ensure that there are no more than {} digits in total",
								max_digits
							)));
						}
					}

					if let Some(decimal_places) = self.decimal_places {
						let parts: Vec<&str> = str_repr.split('.').collect();
						if let Some(decimals) = parts.get(1)
							&& decimals.len() > decimal_places
						{
							return Err(FieldError::Validation(format!(
								"Ensure that there are no more than {} decimal places",
								decimal_places
							)));
						}
					}
				}

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

				Ok(serde_json::json!(num))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_decimalfield_basic() {
		let field = DecimalField::new("amount".to_string());

		assert_eq!(
			field.clean(Some(&serde_json::json!(3.15))).unwrap(),
			serde_json::json!(3.15)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("3.15"))).unwrap(),
			serde_json::json!(3.15)
		);
	}

	#[test]
	fn test_decimalfield_max_digits() {
		let mut field = DecimalField::new("amount".to_string());
		field.max_digits = Some(5);
		field.decimal_places = Some(2);

		assert!(field.clean(Some(&serde_json::json!("123.45"))).is_ok());
		assert!(matches!(
			field.clean(Some(&serde_json::json!("1234.567"))),
			Err(FieldError::Validation(_))
		));
		assert!(matches!(
			field.clean(Some(&serde_json::json!("123.456"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_decimalfield_range() {
		let mut field = DecimalField::new("amount".to_string());
		field.min_value = Some(0.0);
		field.max_value = Some(100.0);

		assert!(field.clean(Some(&serde_json::json!(50.0))).is_ok());
		assert!(matches!(
			field.clean(Some(&serde_json::json!(-1.0))),
			Err(FieldError::Validation(_))
		));
		assert!(matches!(
			field.clean(Some(&serde_json::json!(101.0))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_decimalfield_required() {
		let field = DecimalField::new("price".to_string());

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_decimalfield_not_required() {
		let mut field = DecimalField::new("price".to_string());
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
	fn test_decimalfield_integer_input() {
		let field = DecimalField::new("amount".to_string());

		// Integer as number
		assert_eq!(
			field.clean(Some(&serde_json::json!(42))).unwrap(),
			serde_json::json!(42.0)
		);

		// Integer as string
		assert_eq!(
			field.clean(Some(&serde_json::json!("42"))).unwrap(),
			serde_json::json!(42.0)
		);
	}

	#[test]
	fn test_decimalfield_negative_numbers() {
		let mut field = DecimalField::new("amount".to_string());
		field.required = false;

		assert_eq!(
			field.clean(Some(&serde_json::json!(-3.15))).unwrap(),
			serde_json::json!(-3.15)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("-3.15"))).unwrap(),
			serde_json::json!(-3.15)
		);
	}

	#[test]
	fn test_decimalfield_whitespace_trimming() {
		let field = DecimalField::new("amount".to_string());

		assert_eq!(
			field.clean(Some(&serde_json::json!("  3.15  "))).unwrap(),
			serde_json::json!(3.15)
		);
	}

	#[test]
	fn test_decimalfield_invalid_input() {
		let field = DecimalField::new("amount".to_string());

		// Non-numeric string
		assert!(field.clean(Some(&serde_json::json!("abc"))).is_err());

		// Multiple decimal points
		assert!(field.clean(Some(&serde_json::json!("3.15.15"))).is_err());
	}

	#[test]
	fn test_decimalfield_infinity_nan() {
		let field = DecimalField::new("amount".to_string());

		// Infinity is rejected
		assert!(
			field
				.clean(Some(&serde_json::json!(f64::INFINITY)))
				.is_err()
		);

		// NaN is rejected
		assert!(field.clean(Some(&serde_json::json!(f64::NAN))).is_err());
	}

	#[test]
	fn test_decimalfield_max_digits_exact() {
		let mut field = DecimalField::new("amount".to_string());
		field.max_digits = Some(5);

		// Exactly 5 digits should pass
		assert!(field.clean(Some(&serde_json::json!("12345"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("123.45"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("12.345"))).is_ok());
	}

	#[test]
	fn test_decimalfield_decimal_places_exact() {
		let mut field = DecimalField::new("amount".to_string());
		field.decimal_places = Some(2);

		// Exactly 2 decimal places should pass
		assert!(field.clean(Some(&serde_json::json!("123.45"))).is_ok());

		// 1 decimal place should pass (less than max)
		assert!(field.clean(Some(&serde_json::json!("123.4"))).is_ok());

		// 3 decimal places should fail
		assert!(field.clean(Some(&serde_json::json!("123.456"))).is_err());
	}

	#[test]
	fn test_decimalfield_max_value_exact() {
		let mut field = DecimalField::new("amount".to_string());
		field.max_value = Some(100.0);

		// Exactly max value should pass
		assert!(field.clean(Some(&serde_json::json!(100.0))).is_ok());

		// Just above max should fail
		assert!(field.clean(Some(&serde_json::json!(100.1))).is_err());
	}

	#[test]
	fn test_decimalfield_min_value_exact() {
		let mut field = DecimalField::new("amount".to_string());
		field.min_value = Some(0.0);

		// Exactly min value should pass
		assert!(field.clean(Some(&serde_json::json!(0.0))).is_ok());

		// Just below min should fail
		assert!(field.clean(Some(&serde_json::json!(-0.1))).is_err());
	}

	#[test]
	fn test_decimalfield_combined_constraints() {
		let mut field = DecimalField::new("amount".to_string());
		field.min_value = Some(0.0);
		field.max_value = Some(999.99);
		field.max_digits = Some(5);
		field.decimal_places = Some(2);

		// Valid values
		assert!(field.clean(Some(&serde_json::json!("0.00"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("123.45"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("999.99"))).is_ok());

		// Exceeds max value
		assert!(field.clean(Some(&serde_json::json!("1000.00"))).is_err());

		// Below min value
		assert!(field.clean(Some(&serde_json::json!("-0.01"))).is_err());

		// Too many decimal places
		assert!(field.clean(Some(&serde_json::json!("123.456"))).is_err());

		// Too many total digits
		assert!(field.clean(Some(&serde_json::json!("1234.56"))).is_err());
	}

	#[test]
	fn test_decimalfield_localize_option() {
		let field = DecimalField::new("amount".to_string()).with_localize(true);
		assert!(field.localize);
	}

	#[test]
	fn test_decimalfield_locale_option() {
		let field = DecimalField::new("amount".to_string()).with_locale("en_US".to_string());
		assert_eq!(field.locale, Some("en_US".to_string()));
	}

	#[test]
	fn test_decimalfield_thousands_separator() {
		let field = DecimalField::new("amount".to_string()).with_thousands_separator(true);
		assert!(field.use_thousands_separator);
	}

	#[test]
	fn test_decimalfield_widget() {
		let field = DecimalField::new("amount".to_string());
		assert!(matches!(field.widget(), &Widget::NumberInput));
	}

	#[rstest]
	#[case("007", true)]
	#[case("00.5", true)]
	#[case("00", true)]
	#[case("01", true)]
	#[case("0123.45", true)]
	#[case("0", false)]
	#[case("0.5", false)]
	#[case("7", false)]
	#[case("123", false)]
	#[case("10.5", false)]
	#[case("-0", false)]
	#[case("-0.5", false)]
	#[case("-007", true)]
	fn test_decimalfield_leading_zeros(#[case] input: &str, #[case] should_reject: bool) {
		// Arrange
		let field = DecimalField::new("amount".to_string());

		// Act
		let result = field.clean(Some(&serde_json::json!(input)));

		// Assert
		if should_reject {
			assert!(
				matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("leading zeros")),
				"Expected leading zeros rejection for input '{}', got: {:?}",
				input,
				result,
			);
		} else {
			assert!(
				result.is_ok(),
				"Expected valid input '{}' to succeed, got: {:?}",
				input,
				result,
			);
		}
	}
}
