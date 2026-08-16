//! Numeric validators

use super::{ValidationError, ValidationResult, Validator};
use std::fmt::Display;

/// Minimum value validator
pub struct MinValueValidator<T> {
	min: T,
	message: Option<String>,
}

impl<T> MinValueValidator<T> {
	/// Creates a new MinValueValidator with the specified minimum value.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MinValueValidator, Validator};
	///
	/// let validator = MinValueValidator::new(10);
	/// assert!(validator.validate(&15).is_ok());
	/// assert!(validator.validate(&5).is_err());
	/// ```
	pub fn new(min: T) -> Self {
		Self { min, message: None }
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MinValueValidator, Validator};
	///
	/// let validator = MinValueValidator::new(10).with_message("Value must be at least 10");
	/// let result = validator.validate(&5);
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl<T: PartialOrd + Display + Clone> Validator<T> for MinValueValidator<T> {
	fn validate(&self, value: &T) -> ValidationResult<()> {
		if value >= &self.min {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooSmall {
				value: value.to_string(),
				min: self.min.to_string(),
			})
		}
	}
}

/// Maximum value validator
pub struct MaxValueValidator<T> {
	max: T,
	message: Option<String>,
}

impl<T> MaxValueValidator<T> {
	/// Creates a new MaxValueValidator with the specified maximum value.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MaxValueValidator, Validator};
	///
	/// let validator = MaxValueValidator::new(20);
	/// assert!(validator.validate(&15).is_ok());
	/// assert!(validator.validate(&25).is_err());
	/// ```
	pub fn new(max: T) -> Self {
		Self { max, message: None }
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MaxValueValidator, Validator};
	///
	/// let validator = MaxValueValidator::new(20).with_message("Value must be at most 20");
	/// let result = validator.validate(&25);
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl<T: PartialOrd + Display + Clone> Validator<T> for MaxValueValidator<T> {
	fn validate(&self, value: &T) -> ValidationResult<()> {
		if value <= &self.max {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooLarge {
				value: value.to_string(),
				max: self.max.to_string(),
			})
		}
	}
}

/// Range validator
pub struct RangeValidator<T> {
	min: T,
	max: T,
	message: Option<String>,
}

impl<T> RangeValidator<T> {
	/// Creates a new RangeValidator with the specified minimum and maximum values.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{RangeValidator, Validator};
	///
	/// let validator = RangeValidator::new(10, 20);
	/// assert!(validator.validate(&15).is_ok());
	/// assert!(validator.validate(&5).is_err());
	/// assert!(validator.validate(&25).is_err());
	/// ```
	pub fn new(min: T, max: T) -> Self {
		Self {
			min,
			max,
			message: None,
		}
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{RangeValidator, Validator};
	///
	/// let validator = RangeValidator::new(10, 20).with_message("Value must be between 10 and 20");
	/// let result = validator.validate(&5);
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl<T: PartialOrd + Display + Clone> Validator<T> for RangeValidator<T> {
	fn validate(&self, value: &T) -> ValidationResult<()> {
		if value < &self.min {
			if let Some(ref msg) = self.message {
				Err(ValidationError::Custom(msg.clone()))
			} else {
				Err(ValidationError::TooSmall {
					value: value.to_string(),
					min: self.min.to_string(),
				})
			}
		} else if value > &self.max {
			if let Some(ref msg) = self.message {
				Err(ValidationError::Custom(msg.clone()))
			} else {
				Err(ValidationError::TooLarge {
					value: value.to_string(),
					max: self.max.to_string(),
				})
			}
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Tests based on Django validators/tests.py - TestValidatorEquality::test_basic_equality
	#[test]
	fn test_min_value_validator_integers() {
		let validator = MinValueValidator::new(10);
		assert!(validator.validate(&10).is_ok());
		assert!(validator.validate(&15).is_ok());
		assert!(validator.validate(&100).is_ok());
		assert!(validator.validate(&5).is_err());
		assert!(validator.validate(&0).is_err());
		assert!(validator.validate(&-10).is_err());
	}

	#[test]
	fn test_min_value_validator_floats() {
		let validator = MinValueValidator::new(10.5);
		assert!(validator.validate(&10.5).is_ok());
		assert!(validator.validate(&15.5).is_ok());
		assert!(validator.validate(&10.4).is_err());
		assert!(validator.validate(&5.0).is_err());
	}

	#[test]
	fn test_min_value_validator_error_message() {
		let validator = MinValueValidator::new(44);
		match validator.validate(&10) {
			Err(ValidationError::TooSmall { value, min }) => {
				assert_eq!(value, "10");
				assert_eq!(min, "44");
			}
			_ => panic!("Expected TooSmall error"),
		}
	}

	#[test]
	fn test_max_value_validator_integers() {
		let validator = MaxValueValidator::new(20);
		assert!(validator.validate(&20).is_ok());
		assert!(validator.validate(&15).is_ok());
		assert!(validator.validate(&0).is_ok());
		assert!(validator.validate(&-10).is_ok());
		assert!(validator.validate(&25).is_err());
		assert!(validator.validate(&100).is_err());
	}

	#[test]
	fn test_max_value_validator_floats() {
		let validator = MaxValueValidator::new(20.5);
		assert!(validator.validate(&20.5).is_ok());
		assert!(validator.validate(&15.5).is_ok());
		assert!(validator.validate(&20.6).is_err());
		assert!(validator.validate(&100.0).is_err());
	}

	#[test]
	fn test_max_value_validator_error_message() {
		let validator = MaxValueValidator::new(44);
		match validator.validate(&100) {
			Err(ValidationError::TooLarge { value, max }) => {
				assert_eq!(value, "100");
				assert_eq!(max, "44");
			}
			_ => panic!("Expected TooLarge error"),
		}
	}

	#[test]
	fn test_range_validator_within_range() {
		let validator = RangeValidator::new(10, 20);
		assert!(validator.validate(&10).is_ok());
		assert!(validator.validate(&15).is_ok());
		assert!(validator.validate(&20).is_ok());
	}

	#[test]
	fn test_range_validator_below_range() {
		let validator = RangeValidator::new(10, 20);
		let result = validator.validate(&5);
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooSmall { value, min }) => {
				assert_eq!(value, "5");
				assert_eq!(min, "10");
			}
			_ => panic!("Expected TooSmall error"),
		}
	}

	#[test]
	fn test_range_validator_above_range() {
		let validator = RangeValidator::new(10, 20);
		let result = validator.validate(&25);
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooLarge { value, max }) => {
				assert_eq!(value, "25");
				assert_eq!(max, "20");
			}
			_ => panic!("Expected TooLarge error"),
		}
	}

	#[test]
	fn test_range_validator_custom_message() {
		let validator = RangeValidator::new(10, 20).with_message("Value is out of range");

		assert_eq!(
			validator.validate(&5),
			Err(ValidationError::Custom("Value is out of range".to_owned()))
		);
		assert_eq!(
			validator.validate(&25),
			Err(ValidationError::Custom("Value is out of range".to_owned()))
		);
	}

	#[test]
	fn test_range_validator_floats() {
		let validator = RangeValidator::new(0.0, 1.0);
		assert!(validator.validate(&0.0).is_ok());
		assert!(validator.validate(&0.5).is_ok());
		assert!(validator.validate(&1.0).is_ok());
		assert!(validator.validate(&-0.1).is_err());
		assert!(validator.validate(&1.1).is_err());
	}

	#[test]
	fn test_range_validator_negative_range() {
		let validator = RangeValidator::new(-100, -50);
		assert!(validator.validate(&-75).is_ok());
		assert!(validator.validate(&-100).is_ok());
		assert!(validator.validate(&-50).is_ok());
		assert!(validator.validate(&-101).is_err());
		assert!(validator.validate(&-49).is_err());
		assert!(validator.validate(&0).is_err());
	}

	#[test]
	fn test_validator_boundary_conditions() {
		// Test boundary conditions for min value
		let min_validator = MinValueValidator::new(0);
		assert!(min_validator.validate(&0).is_ok());
		assert!(min_validator.validate(&-1).is_err());

		// Test boundary conditions for max value
		let max_validator = MaxValueValidator::new(100);
		assert!(max_validator.validate(&100).is_ok());
		assert!(max_validator.validate(&101).is_err());
	}

	// Test with different numeric types
	#[test]
	fn test_validators_with_i32() {
		let validator = RangeValidator::new(i32::MIN + 1000, i32::MAX - 1000);
		assert!(validator.validate(&0).is_ok());
		assert!(validator.validate(&(i32::MIN + 1000)).is_ok());
		assert!(validator.validate(&(i32::MAX - 1000)).is_ok());
	}

	#[test]
	fn test_validators_with_u32() {
		let validator = RangeValidator::new(0u32, 1000u32);
		assert!(validator.validate(&500u32).is_ok());
		assert!(validator.validate(&0u32).is_ok());
		assert!(validator.validate(&1000u32).is_ok());
		assert!(validator.validate(&1001u32).is_err());
	}

	#[test]
	fn test_validators_with_f64() {
		let validator = RangeValidator::new(0.0f64, 1.0f64);
		assert!(validator.validate(&0.5f64).is_ok());
		assert!(validator.validate(&0.0f64).is_ok());
		assert!(validator.validate(&1.0f64).is_ok());
		assert!(validator.validate(&1.1f64).is_err());
		assert!(validator.validate(&-0.1f64).is_err());
	}
}
