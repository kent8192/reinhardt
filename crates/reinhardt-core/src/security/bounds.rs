//! Numeric safety utilities for preventing arithmetic overflow, underflow,
//! and division-by-zero.
//!
//! Provides checked arithmetic operations that return `Result` instead of
//! panicking, and safe type conversions between numeric types.
//!
//! # Overview
//!
//! All standard arithmetic operations (addition, subtraction, multiplication,
//! division) are provided for `i64`, `u64`, and `usize` types. Each function
//! wraps Rust's built-in `checked_*()` methods and returns a
//! [`CheckedArithmeticError`] on failure instead of panicking.
//!
//! Safe type conversion functions are also provided for common numeric type
//! conversions that may lose precision or fail on out-of-range values.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::security::bounds::{checked_div_u64, checked_add_u64, safe_usize_to_u32};
//! use reinhardt_core::security::CheckedArithmeticError;
//!
//! // Safe division
//! assert_eq!(checked_div_u64(10, 2), Ok(5));
//! assert_eq!(checked_div_u64(10, 0), Err(CheckedArithmeticError::DivisionByZero));
//!
//! // Safe addition
//! assert_eq!(checked_add_u64(1, 2), Ok(3));
//! assert_eq!(checked_add_u64(u64::MAX, 1), Err(CheckedArithmeticError::Overflow));
//!
//! // Safe type conversion
//! assert_eq!(safe_usize_to_u32(100), Ok(100));
//! ```

use thiserror::Error;

/// Errors from checked arithmetic operations.
///
/// Returned by checked arithmetic functions and safe type conversions when
/// the operation cannot be performed safely.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckedArithmeticError {
	/// Arithmetic overflow occurred.
	#[error("arithmetic overflow")]
	Overflow,

	/// Arithmetic underflow occurred.
	#[error("arithmetic underflow")]
	Underflow,

	/// Division by zero attempted.
	#[error("division by zero")]
	DivisionByZero,

	/// Value out of range for target type.
	#[error("out of range: {detail}")]
	OutOfRange {
		/// Description of the conversion that failed.
		detail: String,
	},
}

// ---------------------------------------------------------------------------
// Division
// ---------------------------------------------------------------------------

/// Safe division for `i64` that returns an error instead of panicking on zero.
///
/// Also detects overflow from `i64::MIN / -1`.
pub fn checked_div_i64(a: i64, b: i64) -> Result<i64, CheckedArithmeticError> {
	if b == 0 {
		return Err(CheckedArithmeticError::DivisionByZero);
	}
	a.checked_div(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe division for `u64` that returns an error on division by zero.
pub fn checked_div_u64(a: u64, b: u64) -> Result<u64, CheckedArithmeticError> {
	if b == 0 {
		return Err(CheckedArithmeticError::DivisionByZero);
	}
	// u64 division cannot overflow, but use checked_div for consistency.
	a.checked_div(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe division for `usize` that returns an error on division by zero.
pub fn checked_div_usize(a: usize, b: usize) -> Result<usize, CheckedArithmeticError> {
	if b == 0 {
		return Err(CheckedArithmeticError::DivisionByZero);
	}
	a.checked_div(b).ok_or(CheckedArithmeticError::Overflow)
}

// ---------------------------------------------------------------------------
// Multiplication
// ---------------------------------------------------------------------------

/// Safe multiplication for `u64` with overflow detection.
pub fn checked_mul_u64(a: u64, b: u64) -> Result<u64, CheckedArithmeticError> {
	a.checked_mul(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe multiplication for `usize` with overflow detection.
pub fn checked_mul_usize(a: usize, b: usize) -> Result<usize, CheckedArithmeticError> {
	a.checked_mul(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe multiplication for `i64` with overflow detection.
///
/// Returns [`CheckedArithmeticError::Overflow`] for both positive overflow
/// and negative overflow (e.g. `i64::MIN * 2`).
pub fn checked_mul_i64(a: i64, b: i64) -> Result<i64, CheckedArithmeticError> {
	a.checked_mul(b).ok_or(CheckedArithmeticError::Overflow)
}

// ---------------------------------------------------------------------------
// Addition
// ---------------------------------------------------------------------------

/// Safe addition for `u64` with overflow detection.
pub fn checked_add_u64(a: u64, b: u64) -> Result<u64, CheckedArithmeticError> {
	a.checked_add(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe addition for `usize` with overflow detection.
pub fn checked_add_usize(a: usize, b: usize) -> Result<usize, CheckedArithmeticError> {
	a.checked_add(b).ok_or(CheckedArithmeticError::Overflow)
}

/// Safe addition for `i64` with overflow detection.
///
/// Returns [`CheckedArithmeticError::Overflow`] when adding two large
/// positive values, or [`CheckedArithmeticError::Underflow`] when adding
/// two large negative values.
pub fn checked_add_i64(a: i64, b: i64) -> Result<i64, CheckedArithmeticError> {
	match a.checked_add(b) {
		Some(result) => Ok(result),
		None => {
			// If both operands are negative (or one is very negative), it's underflow.
			// Otherwise it's overflow.
			if a < 0 && b < 0 {
				Err(CheckedArithmeticError::Underflow)
			} else {
				Err(CheckedArithmeticError::Overflow)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Subtraction
// ---------------------------------------------------------------------------

/// Safe subtraction for `u64` with underflow detection.
pub fn checked_sub_u64(a: u64, b: u64) -> Result<u64, CheckedArithmeticError> {
	a.checked_sub(b).ok_or(CheckedArithmeticError::Underflow)
}

/// Safe subtraction for `usize` with underflow detection.
pub fn checked_sub_usize(a: usize, b: usize) -> Result<usize, CheckedArithmeticError> {
	a.checked_sub(b).ok_or(CheckedArithmeticError::Underflow)
}

/// Safe subtraction for `i64` with overflow/underflow detection.
///
/// Returns [`CheckedArithmeticError::Overflow`] when subtracting a large
/// negative from a large positive, or [`CheckedArithmeticError::Underflow`]
/// when subtracting a large positive from a large negative.
pub fn checked_sub_i64(a: i64, b: i64) -> Result<i64, CheckedArithmeticError> {
	match a.checked_sub(b) {
		Some(result) => Ok(result),
		None => {
			// a - b overflows: if a >= 0 and b < 0, it's positive overflow.
			// If a < 0 and b > 0, it's negative underflow.
			if a >= 0 && b < 0 {
				Err(CheckedArithmeticError::Overflow)
			} else {
				Err(CheckedArithmeticError::Underflow)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Type conversions
// ---------------------------------------------------------------------------

/// Safe conversion from `usize` to `u32`.
///
/// Returns [`CheckedArithmeticError::OutOfRange`] if the value exceeds
/// `u32::MAX`.
pub fn safe_usize_to_u32(val: usize) -> Result<u32, CheckedArithmeticError> {
	u32::try_from(val).map_err(|_| CheckedArithmeticError::OutOfRange {
		detail: format!("usize value {val} exceeds u32::MAX ({})", u32::MAX),
	})
}

/// Safe conversion from `u64` to `usize`.
///
/// On 64-bit platforms this is infallible, but on 32-bit platforms values
/// above `usize::MAX` will fail.
pub fn safe_u64_to_usize(val: u64) -> Result<usize, CheckedArithmeticError> {
	usize::try_from(val).map_err(|_| CheckedArithmeticError::OutOfRange {
		detail: format!("u64 value {val} exceeds usize::MAX ({})", usize::MAX),
	})
}

/// Safe conversion from `i64` to `usize`.
///
/// Rejects negative values and values exceeding `usize::MAX`.
pub fn safe_i64_to_usize(val: i64) -> Result<usize, CheckedArithmeticError> {
	usize::try_from(val).map_err(|_| {
		if val < 0 {
			CheckedArithmeticError::OutOfRange {
				detail: format!("negative i64 value {val} cannot be converted to usize"),
			}
		} else {
			CheckedArithmeticError::OutOfRange {
				detail: format!("i64 value {val} exceeds usize::MAX ({})", usize::MAX),
			}
		}
	})
}

/// Safe conversion from `usize` to `u64`.
///
/// On 64-bit platforms this is infallible. On 32-bit platforms the
/// conversion always succeeds because `usize` is at most 32 bits.
pub fn safe_usize_to_u64(val: usize) -> Result<u64, CheckedArithmeticError> {
	u64::try_from(val).map_err(|_| CheckedArithmeticError::OutOfRange {
		detail: format!("usize value {val} exceeds u64::MAX ({})", u64::MAX),
	})
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	// -----------------------------------------------------------------------
	// Division tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::normal(10, 2, Ok(5))]
	#[case::exact(100, 10, Ok(10))]
	#[case::truncated(7, 2, Ok(3))]
	#[case::negative_dividend(-10, 2, Ok(-5))]
	#[case::negative_divisor(10, -2, Ok(-5))]
	#[case::both_negative(-10, -2, Ok(5))]
	#[case::div_by_zero(10, 0, Err(CheckedArithmeticError::DivisionByZero))]
	#[case::zero_div_by_zero(0, 0, Err(CheckedArithmeticError::DivisionByZero))]
	#[case::min_div_minus_one(i64::MIN, -1, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_div_i64(
		#[case] a: i64,
		#[case] b: i64,
		#[case] expected: Result<i64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_div_i64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(10, 2, Ok(5))]
	#[case::exact(100, 10, Ok(10))]
	#[case::div_by_zero(10, 0, Err(CheckedArithmeticError::DivisionByZero))]
	#[case::zero_div_by_zero(0, 0, Err(CheckedArithmeticError::DivisionByZero))]
	#[case::max_div_one(u64::MAX, 1, Ok(u64::MAX))]
	fn test_checked_div_u64(
		#[case] a: u64,
		#[case] b: u64,
		#[case] expected: Result<u64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_div_u64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(10, 2, Ok(5))]
	#[case::div_by_zero(10, 0, Err(CheckedArithmeticError::DivisionByZero))]
	#[case::max_div_one(usize::MAX, 1, Ok(usize::MAX))]
	fn test_checked_div_usize(
		#[case] a: usize,
		#[case] b: usize,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_div_usize(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	// -----------------------------------------------------------------------
	// Multiplication tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::normal(3, 4, Ok(12))]
	#[case::zero_left(0, 100, Ok(0))]
	#[case::zero_right(100, 0, Ok(0))]
	#[case::one(1, u64::MAX, Ok(u64::MAX))]
	#[case::overflow(u64::MAX, 2, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_mul_u64(
		#[case] a: u64,
		#[case] b: u64,
		#[case] expected: Result<u64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_mul_u64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(3, 4, Ok(12))]
	#[case::zero(0, 100, Ok(0))]
	#[case::overflow(usize::MAX, 2, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_mul_usize(
		#[case] a: usize,
		#[case] b: usize,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_mul_usize(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(3, 4, Ok(12))]
	#[case::negative(-3, 4, Ok(-12))]
	#[case::both_negative(-3, -4, Ok(12))]
	#[case::zero(0, i64::MAX, Ok(0))]
	#[case::overflow(i64::MAX, 2, Err(CheckedArithmeticError::Overflow))]
	#[case::negative_overflow(i64::MIN, 2, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_mul_i64(
		#[case] a: i64,
		#[case] b: i64,
		#[case] expected: Result<i64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_mul_i64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	// -----------------------------------------------------------------------
	// Addition tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::normal(1, 2, Ok(3))]
	#[case::zero(0, 0, Ok(0))]
	#[case::max_plus_zero(u64::MAX, 0, Ok(u64::MAX))]
	#[case::overflow(u64::MAX, 1, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_add_u64(
		#[case] a: u64,
		#[case] b: u64,
		#[case] expected: Result<u64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_add_u64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(1, 2, Ok(3))]
	#[case::zero(0, 0, Ok(0))]
	#[case::overflow(usize::MAX, 1, Err(CheckedArithmeticError::Overflow))]
	fn test_checked_add_usize(
		#[case] a: usize,
		#[case] b: usize,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_add_usize(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(1, 2, Ok(3))]
	#[case::negative(-1, -2, Ok(-3))]
	#[case::mixed(1, -2, Ok(-1))]
	#[case::zero(0, 0, Ok(0))]
	#[case::positive_overflow(i64::MAX, 1, Err(CheckedArithmeticError::Overflow))]
	#[case::negative_underflow(i64::MIN, -1, Err(CheckedArithmeticError::Underflow))]
	fn test_checked_add_i64(
		#[case] a: i64,
		#[case] b: i64,
		#[case] expected: Result<i64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_add_i64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	// -----------------------------------------------------------------------
	// Subtraction tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::normal(5, 3, Ok(2))]
	#[case::zero(0, 0, Ok(0))]
	#[case::same(10, 10, Ok(0))]
	#[case::underflow(0, 1, Err(CheckedArithmeticError::Underflow))]
	#[case::large_underflow(0, u64::MAX, Err(CheckedArithmeticError::Underflow))]
	fn test_checked_sub_u64(
		#[case] a: u64,
		#[case] b: u64,
		#[case] expected: Result<u64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_sub_u64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(5, 3, Ok(2))]
	#[case::zero(0, 0, Ok(0))]
	#[case::underflow(0, 1, Err(CheckedArithmeticError::Underflow))]
	fn test_checked_sub_usize(
		#[case] a: usize,
		#[case] b: usize,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_sub_usize(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::normal(5, 3, Ok(2))]
	#[case::negative_result(3, 5, Ok(-2))]
	#[case::both_negative(-3, -5, Ok(2))]
	#[case::zero(0, 0, Ok(0))]
	#[case::overflow(i64::MAX, -1, Err(CheckedArithmeticError::Overflow))]
	#[case::underflow(i64::MIN, 1, Err(CheckedArithmeticError::Underflow))]
	fn test_checked_sub_i64(
		#[case] a: i64,
		#[case] b: i64,
		#[case] expected: Result<i64, CheckedArithmeticError>,
	) {
		// Act
		let result = checked_sub_i64(a, b);

		// Assert
		assert_eq!(result, expected);
	}

	// -----------------------------------------------------------------------
	// Type conversion tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::zero(0, Ok(0))]
	#[case::normal(100, Ok(100))]
	#[case::max_u32(u32::MAX as usize, Ok(u32::MAX))]
	fn test_safe_usize_to_u32_success(
		#[case] val: usize,
		#[case] expected: Result<u32, CheckedArithmeticError>,
	) {
		// Act
		let result = safe_usize_to_u32(val);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_safe_usize_to_u32_overflow() {
		// Arrange
		let val = u32::MAX as usize + 1;

		// Act
		let result = safe_usize_to_u32(val);

		// Assert
		assert!(matches!(
			result,
			Err(CheckedArithmeticError::OutOfRange { .. })
		));
	}

	#[rstest]
	#[case::zero(0, Ok(0))]
	#[case::normal(100, Ok(100))]
	fn test_safe_u64_to_usize_success(
		#[case] val: u64,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = safe_u64_to_usize(val);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::zero(0, Ok(0))]
	#[case::normal(100, Ok(100))]
	#[case::max_positive(i64::MAX, Ok(i64::MAX as usize))]
	fn test_safe_i64_to_usize_success(
		#[case] val: i64,
		#[case] expected: Result<usize, CheckedArithmeticError>,
	) {
		// Act
		let result = safe_i64_to_usize(val);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::negative(-1)]
	#[case::min(i64::MIN)]
	fn test_safe_i64_to_usize_negative(#[case] val: i64) {
		// Act
		let result = safe_i64_to_usize(val);

		// Assert
		assert!(matches!(
			result,
			Err(CheckedArithmeticError::OutOfRange { .. })
		));
	}

	#[rstest]
	#[case::zero(0, Ok(0))]
	#[case::normal(100, Ok(100))]
	fn test_safe_usize_to_u64(
		#[case] val: usize,
		#[case] expected: Result<u64, CheckedArithmeticError>,
	) {
		// Act
		let result = safe_usize_to_u64(val);

		// Assert
		assert_eq!(result, expected);
	}

	// -----------------------------------------------------------------------
	// Error display tests
	// -----------------------------------------------------------------------

	#[rstest]
	#[case::overflow(CheckedArithmeticError::Overflow, "arithmetic overflow")]
	#[case::underflow(CheckedArithmeticError::Underflow, "arithmetic underflow")]
	#[case::div_by_zero(CheckedArithmeticError::DivisionByZero, "division by zero")]
	#[case::out_of_range(
		CheckedArithmeticError::OutOfRange { detail: "test detail".to_string() },
		"out of range: test detail"
	)]
	fn test_error_display(#[case] error: CheckedArithmeticError, #[case] expected: &str) {
		// Act
		let display = error.to_string();

		// Assert
		assert_eq!(display, expected);
	}
}
