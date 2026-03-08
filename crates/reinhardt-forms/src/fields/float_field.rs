use crate::field::{FieldError, FieldResult, FormField, Widget};

/// FloatField for floating-point number input
pub struct FloatField {
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
	pub max_value: Option<f64>,
	/// Minimum allowed value.
	pub min_value: Option<f64>,
}

impl FloatField {
	/// Create a new FloatField with the given name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::FloatField;
	///
	/// let field = FloatField::new("price".to_string());
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
		}
	}
}

impl FormField for FloatField {
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
				// Parse float from either number or string
				let num = if let Some(f) = v.as_f64() {
					f
				} else if let Some(s) = v.as_str() {
					// Trim whitespace
					let s = s.trim();

					// Return None/error for empty string
					if s.is_empty() {
						if self.required {
							return Err(FieldError::required(None));
						}
						return Ok(serde_json::Value::Null);
					}

					// Parse string to float
					s.parse::<f64>()
						.map_err(|_| FieldError::Invalid("Enter a number".to_string()))?
				} else if let Some(i) = v.as_i64() {
					// Convert integer to float
					i as f64
				} else {
					return Err(FieldError::Invalid("Expected number or string".to_string()));
				};

				// Check for special values
				if !num.is_finite() {
					return Err(FieldError::Invalid("Enter a valid number".to_string()));
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

	#[test]
	fn test_floatfield_basic() {
		let field = FloatField::new("price".to_string());

		assert_eq!(
			field.clean(Some(&serde_json::json!(3.15))).unwrap(),
			serde_json::json!(3.15)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!(42))).unwrap(),
			serde_json::json!(42.0)
		);
	}

	#[test]
	fn test_floatfield_string_parsing() {
		let mut field = FloatField::new("value".to_string());
		field.required = false;

		assert_eq!(
			field.clean(Some(&serde_json::json!("3.15"))).unwrap(),
			serde_json::json!(3.15)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("  -2.5  "))).unwrap(),
			serde_json::json!(-2.5)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("42"))).unwrap(),
			serde_json::json!(42.0)
		);
	}

	#[test]
	fn test_floatfield_range() {
		let mut field = FloatField::new("score".to_string());
		field.min_value = Some(0.0);
		field.max_value = Some(100.0);

		assert!(field.clean(Some(&serde_json::json!(50.0))).is_ok());
		assert!(field.clean(Some(&serde_json::json!(0.0))).is_ok());
		assert!(field.clean(Some(&serde_json::json!(100.0))).is_ok());

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
	fn test_floatfield_invalid() {
		let field = FloatField::new("value".to_string());

		assert!(matches!(
			field.clean(Some(&serde_json::json!("abc"))),
			Err(FieldError::Invalid(_))
		));
	}

	#[test]
	fn test_floatfield_required() {
		let field = FloatField::new("value".to_string());

		// Required field rejects None
		assert!(field.clean(None).is_err());

		// Required field rejects empty string
		assert!(field.clean(Some(&serde_json::json!(""))).is_err());
	}

	#[test]
	fn test_floatfield_not_required() {
		let mut field = FloatField::new("value".to_string());
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
	fn test_floatfield_negative_numbers() {
		let mut field = FloatField::new("value".to_string());
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
	fn test_floatfield_scientific_notation() {
		let mut field = FloatField::new("value".to_string());
		field.required = false;

		// Scientific notation as string
		assert_eq!(
			field.clean(Some(&serde_json::json!("1.5e2"))).unwrap(),
			serde_json::json!(150.0)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("1e-3"))).unwrap(),
			serde_json::json!(0.001)
		);
	}

	#[test]
	fn test_floatfield_infinity() {
		let field = FloatField::new("value".to_string());

		// Infinity is rejected
		assert!(
			field
				.clean(Some(&serde_json::json!(f64::INFINITY)))
				.is_err()
		);
		assert!(
			field
				.clean(Some(&serde_json::json!(f64::NEG_INFINITY)))
				.is_err()
		);
	}

	#[test]
	fn test_floatfield_nan() {
		let field = FloatField::new("value".to_string());

		// NaN is rejected
		assert!(field.clean(Some(&serde_json::json!(f64::NAN))).is_err());
	}

	#[test]
	fn test_floatfield_very_small_numbers() {
		let mut field = FloatField::new("value".to_string());
		field.required = false;

		// Very small numbers
		assert_eq!(
			field.clean(Some(&serde_json::json!(0.000001))).unwrap(),
			serde_json::json!(0.000001)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("0.000001"))).unwrap(),
			serde_json::json!(0.000001)
		);
	}

	#[test]
	fn test_floatfield_very_large_numbers() {
		let mut field = FloatField::new("value".to_string());
		field.required = false;

		// Very large numbers
		assert_eq!(
			field.clean(Some(&serde_json::json!(1000000000.0))).unwrap(),
			serde_json::json!(1000000000.0)
		);
	}

	#[test]
	fn test_floatfield_max_value_boundary() {
		let mut field = FloatField::new("value".to_string());
		field.max_value = Some(100.0);

		// Exactly at boundary
		assert!(field.clean(Some(&serde_json::json!(100.0))).is_ok());

		// Just over boundary
		assert!(field.clean(Some(&serde_json::json!(100.001))).is_err());
	}

	#[test]
	fn test_floatfield_min_value_boundary() {
		let mut field = FloatField::new("value".to_string());
		field.min_value = Some(-100.0);

		// Exactly at boundary
		assert!(field.clean(Some(&serde_json::json!(-100.0))).is_ok());

		// Just under boundary
		assert!(field.clean(Some(&serde_json::json!(-100.001))).is_err());
	}

	#[test]
	fn test_floatfield_zero() {
		let field = FloatField::new("value".to_string());

		assert_eq!(
			field.clean(Some(&serde_json::json!(0.0))).unwrap(),
			serde_json::json!(0.0)
		);
		assert_eq!(
			field.clean(Some(&serde_json::json!("0.0"))).unwrap(),
			serde_json::json!(0.0)
		);
	}

	#[test]
	fn test_floatfield_widget() {
		let field = FloatField::new("value".to_string());
		assert!(matches!(field.widget(), &Widget::NumberInput));
	}
}
