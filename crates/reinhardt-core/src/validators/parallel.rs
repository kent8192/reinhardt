//! # Parallel Validation
//!
//! This module provides parallel validation capabilities using rayon.
//!
//! ## Features
//!
//! - Parallel validation of multiple values with a single validator
//! - Parallel validation of a single value with multiple validators
//! - Configurable thread pool and batch processing
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::validators::parallel::{validate_all_parallel, ParallelValidationResult};
//! use reinhardt_core::validators::string::MinLengthValidator;
//! use reinhardt_core::validators::Validator;
//!
//! let validator = MinLengthValidator::new(3);
//! let values: Vec<String> = vec!["hello", "hi", "world", "ab"]
//!     .into_iter().map(String::from).collect();
//!
//! let results = validate_all_parallel(&validator, &values);
//! assert_eq!(results.valid_count(), 2);
//! assert_eq!(results.invalid_count(), 2);
//! ```

use super::Validator;
use super::errors::{ValidationError, ValidationResult};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Result of parallel validation for multiple values.
#[derive(Debug, Clone)]
pub struct ParallelValidationResult<T: Clone> {
	/// Values that passed validation
	pub valid: Vec<T>,
	/// Values that failed validation along with their errors
	pub invalid: Vec<(T, ValidationError)>,
}

impl<T: Clone> Default for ParallelValidationResult<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Clone> ParallelValidationResult<T> {
	/// Creates a new empty result.
	pub fn new() -> Self {
		Self {
			valid: Vec::new(),
			invalid: Vec::new(),
		}
	}

	/// Returns the number of valid values.
	pub fn valid_count(&self) -> usize {
		self.valid.len()
	}

	/// Returns the number of invalid values.
	pub fn invalid_count(&self) -> usize {
		self.invalid.len()
	}

	/// Returns true if all values are valid.
	pub fn all_valid(&self) -> bool {
		self.invalid.is_empty()
	}

	/// Returns true if any value is valid.
	pub fn any_valid(&self) -> bool {
		!self.valid.is_empty()
	}

	/// Returns true if all values are invalid.
	pub fn all_invalid(&self) -> bool {
		self.valid.is_empty()
	}

	/// Returns the total number of values.
	pub fn total(&self) -> usize {
		self.valid.len() + self.invalid.len()
	}

	/// Returns the validation success rate (0.0 to 1.0).
	pub fn success_rate(&self) -> f64 {
		let total = self.total();
		if total == 0 {
			1.0
		} else {
			self.valid.len() as f64 / total as f64
		}
	}

	/// Returns all errors.
	pub fn errors(&self) -> Vec<&ValidationError> {
		self.invalid.iter().map(|(_, e)| e).collect()
	}

	/// Returns the first error if any.
	pub fn first_error(&self) -> Option<&ValidationError> {
		self.invalid.first().map(|(_, e)| e)
	}
}

/// Result of parallel validation for multiple validators on a single value.
#[derive(Debug, Clone)]
pub struct MultiValidatorResult {
	/// Validators that passed (by index)
	pub passed: Vec<usize>,
	/// Validators that failed with their errors (by index)
	pub failed: Vec<(usize, ValidationError)>,
}

impl Default for MultiValidatorResult {
	fn default() -> Self {
		Self::new()
	}
}

impl MultiValidatorResult {
	/// Creates a new empty result.
	pub fn new() -> Self {
		Self {
			passed: Vec::new(),
			failed: Vec::new(),
		}
	}

	/// Returns true if all validators passed.
	pub fn all_passed(&self) -> bool {
		self.failed.is_empty()
	}

	/// Returns true if any validator passed.
	pub fn any_passed(&self) -> bool {
		!self.passed.is_empty()
	}

	/// Returns true if all validators failed.
	pub fn all_failed(&self) -> bool {
		self.passed.is_empty()
	}

	/// Returns the number of passed validators.
	pub fn passed_count(&self) -> usize {
		self.passed.len()
	}

	/// Returns the number of failed validators.
	pub fn failed_count(&self) -> usize {
		self.failed.len()
	}

	/// Returns all errors.
	pub fn errors(&self) -> Vec<&ValidationError> {
		self.failed.iter().map(|(_, e)| e).collect()
	}

	/// Returns the first error if any.
	pub fn first_error(&self) -> Option<&ValidationError> {
		self.failed.first().map(|(_, e)| e)
	}

	/// Returns an aggregated result as `ValidationResult`.
	pub fn to_result(&self) -> ValidationResult<()> {
		if self.all_passed() {
			Ok(())
		} else {
			let error_messages: Vec<String> =
				self.failed.iter().map(|(_, e)| e.to_string()).collect();
			Err(ValidationError::AllValidatorsFailed {
				errors: error_messages.join("; "),
			})
		}
	}
}

/// Options for parallel validation.
#[derive(Debug, Clone)]
pub struct ParallelValidationOptions {
	/// Minimum batch size for parallel processing.
	/// Values collections smaller than this will be processed sequentially.
	pub min_batch_size: usize,
	/// Whether to stop on first error (for all_valid checks).
	pub fail_fast: bool,
}

impl Default for ParallelValidationOptions {
	fn default() -> Self {
		Self {
			min_batch_size: 10,
			fail_fast: false,
		}
	}
}

impl ParallelValidationOptions {
	/// Creates new options with default values.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the minimum batch size.
	pub fn min_batch_size(mut self, size: usize) -> Self {
		self.min_batch_size = size;
		self
	}

	/// Enables fail-fast mode.
	pub fn fail_fast(mut self, fail_fast: bool) -> Self {
		self.fail_fast = fail_fast;
		self
	}
}

/// Validates multiple values in parallel using a single validator.
///
/// # Arguments
///
/// * `validator` - The validator to use
/// * `values` - The values to validate
///
/// # Returns
///
/// A `ParallelValidationResult` containing valid and invalid values.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::validate_all_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world"]
///     .into_iter().map(String::from).collect();
///
/// let results = validate_all_parallel(&validator, &values);
/// assert_eq!(results.valid_count(), 2);
/// ```
pub fn validate_all_parallel<T, V>(validator: &V, values: &[T]) -> ParallelValidationResult<T>
where
	T: Clone + Sync + Send,
	V: Validator<T> + Sync,
{
	validate_all_parallel_with_options(validator, values, &ParallelValidationOptions::default())
}

/// Validates multiple values in parallel with custom options.
pub fn validate_all_parallel_with_options<T, V>(
	validator: &V,
	values: &[T],
	options: &ParallelValidationOptions,
) -> ParallelValidationResult<T>
where
	T: Clone + Sync + Send,
	V: Validator<T> + Sync,
{
	if values.len() < options.min_batch_size {
		// Process sequentially for small collections
		let mut result = ParallelValidationResult::new();
		for value in values {
			match validator.validate(value) {
				Ok(()) => result.valid.push(value.clone()),
				Err(e) => result.invalid.push((value.clone(), e)),
			}
		}
		return result;
	}

	let results: Vec<_> = values
		.par_iter()
		.map(|value| {
			let validation = validator.validate(value);
			(value.clone(), validation)
		})
		.collect();

	let mut result = ParallelValidationResult::new();
	for (value, validation) in results {
		match validation {
			Ok(()) => result.valid.push(value),
			Err(e) => result.invalid.push((value, e)),
		}
	}
	result
}

/// Checks if all values are valid in parallel.
///
/// This is more efficient than `validate_all_parallel` when you only need
/// to know if all values pass validation, especially with fail-fast enabled.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::all_valid_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let valid_values: Vec<String> = vec!["hello", "world", "rust"]
///     .into_iter().map(String::from).collect();
/// let mixed_values: Vec<String> = vec!["hello", "hi", "world"]
///     .into_iter().map(String::from).collect();
///
/// assert!(all_valid_parallel(&validator, &valid_values));
/// assert!(!all_valid_parallel(&validator, &mixed_values));
/// ```
pub fn all_valid_parallel<T, V>(validator: &V, values: &[T]) -> bool
where
	T: Sync,
	V: Validator<T> + Sync,
{
	all_valid_parallel_with_options(validator, values, &ParallelValidationOptions::default())
}

/// Checks if all values are valid in parallel with custom options.
pub fn all_valid_parallel_with_options<T, V>(
	validator: &V,
	values: &[T],
	options: &ParallelValidationOptions,
) -> bool
where
	T: Sync,
	V: Validator<T> + Sync,
{
	if values.len() < options.min_batch_size {
		// Process sequentially for small collections
		return values.iter().all(|v| validator.validate(v).is_ok());
	}

	values.par_iter().all(|v| validator.validate(v).is_ok())
}

/// Checks if any value is valid in parallel.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::any_valid_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hi", "a", "hello"]
///     .into_iter().map(String::from).collect();
///
/// assert!(any_valid_parallel(&validator, &values));
/// ```
pub fn any_valid_parallel<T, V>(validator: &V, values: &[T]) -> bool
where
	T: Sync,
	V: Validator<T> + Sync,
{
	if values.len() < 10 {
		return values.iter().any(|v| validator.validate(v).is_ok());
	}

	values.par_iter().any(|v| validator.validate(v).is_ok())
}

/// Returns the first error found during parallel validation.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::find_first_error_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world"]
///     .into_iter().map(String::from).collect();
///
/// let error = find_first_error_parallel(&validator, &values);
/// assert!(error.is_some());
/// ```
pub fn find_first_error_parallel<T, V>(validator: &V, values: &[T]) -> Option<ValidationError>
where
	T: Sync,
	V: Validator<T> + Sync,
{
	if values.len() < 10 {
		return values.iter().find_map(|v| validator.validate(v).err());
	}

	values
		.par_iter()
		.find_map_any(|v| validator.validate(v).err())
}

/// Counts how many values pass validation in parallel.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::count_valid_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world", "a"]
///     .into_iter().map(String::from).collect();
///
/// assert_eq!(count_valid_parallel(&validator, &values), 2);
/// ```
pub fn count_valid_parallel<T, V>(validator: &V, values: &[T]) -> usize
where
	T: Sync,
	V: Validator<T> + Sync,
{
	if values.len() < 10 {
		return values
			.iter()
			.filter(|v| validator.validate(v).is_ok())
			.count();
	}

	let count = AtomicUsize::new(0);
	values.par_iter().for_each(|v| {
		if validator.validate(v).is_ok() {
			count.fetch_add(1, Ordering::Relaxed);
		}
	});
	count.load(Ordering::Relaxed)
}

/// Validates a single value with multiple validators in parallel.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::validate_with_multiple;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// // Use validators of the same concrete type
/// let validators = vec![
///     MinLengthValidator::new(3),
///     MinLengthValidator::new(5),
/// ];
///
/// let result = validate_with_multiple(&validators, "hello");
/// assert!(result.all_passed());
/// ```
pub fn validate_with_multiple<T, V>(validators: &[V], value: &T) -> MultiValidatorResult
where
	T: Sync + ?Sized,
	V: Validator<T> + Sync,
{
	if validators.len() < 10 {
		// Process sequentially for small numbers of validators
		let mut result = MultiValidatorResult::new();
		for (i, validator) in validators.iter().enumerate() {
			match validator.validate(value) {
				Ok(()) => result.passed.push(i),
				Err(e) => result.failed.push((i, e)),
			}
		}
		return result;
	}

	let results: Vec<_> = validators
		.par_iter()
		.enumerate()
		.map(|(i, validator)| (i, validator.validate(value)))
		.collect();

	let mut result = MultiValidatorResult::new();
	for (i, validation) in results {
		match validation {
			Ok(()) => result.passed.push(i),
			Err(e) => result.failed.push((i, e)),
		}
	}
	result
}

/// Validates a single value with multiple boxed validators in parallel.
///
/// This is useful when you have validators of different types.
pub fn validate_with_multiple_boxed<T>(
	validators: &[Box<dyn Validator<T> + Sync>],
	value: &T,
) -> MultiValidatorResult
where
	T: Sync + ?Sized,
{
	if validators.len() < 10 {
		let mut result = MultiValidatorResult::new();
		for (i, validator) in validators.iter().enumerate() {
			match validator.validate(value) {
				Ok(()) => result.passed.push(i),
				Err(e) => result.failed.push((i, e)),
			}
		}
		return result;
	}

	let results: Vec<_> = validators
		.par_iter()
		.enumerate()
		.map(|(i, validator)| (i, validator.validate(value)))
		.collect();

	let mut result = MultiValidatorResult::new();
	for (i, validation) in results {
		match validation {
			Ok(()) => result.passed.push(i),
			Err(e) => result.failed.push((i, e)),
		}
	}
	result
}

/// Filters values by validation in parallel, returning only valid values.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::filter_valid_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world", "a"]
///     .into_iter().map(String::from).collect();
///
/// let valid = filter_valid_parallel(&validator, &values);
/// assert_eq!(valid.len(), 2);
/// ```
pub fn filter_valid_parallel<T, V>(validator: &V, values: &[T]) -> Vec<T>
where
	T: Clone + Sync + Send,
	V: Validator<T> + Sync,
{
	if values.len() < 10 {
		return values
			.iter()
			.filter(|v| validator.validate(v).is_ok())
			.cloned()
			.collect();
	}

	values
		.par_iter()
		.filter(|v| validator.validate(v).is_ok())
		.cloned()
		.collect()
}

/// Filters values by validation in parallel, returning only invalid values with errors.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::filter_invalid_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world", "a"]
///     .into_iter().map(String::from).collect();
///
/// let invalid = filter_invalid_parallel(&validator, &values);
/// assert_eq!(invalid.len(), 2);
/// ```
pub fn filter_invalid_parallel<T, V>(validator: &V, values: &[T]) -> Vec<(T, ValidationError)>
where
	T: Clone + Sync + Send,
	V: Validator<T> + Sync,
{
	if values.len() < 10 {
		return values
			.iter()
			.filter_map(|v| validator.validate(v).err().map(|e| (v.clone(), e)))
			.collect();
	}

	values
		.par_iter()
		.filter_map(|v| validator.validate(v).err().map(|e| (v.clone(), e)))
		.collect()
}

/// Partitions values into valid and invalid groups in parallel.
///
/// This is equivalent to `validate_all_parallel` but returns a tuple instead of a struct.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::parallel::partition_parallel;
/// use reinhardt_core::validators::string::MinLengthValidator;
/// use reinhardt_core::validators::Validator;
///
/// let validator = MinLengthValidator::new(3);
/// let values: Vec<String> = vec!["hello", "hi", "world", "a"]
///     .into_iter().map(String::from).collect();
///
/// let (valid, invalid) = partition_parallel(&validator, &values);
/// assert_eq!(valid.len(), 2);
/// assert_eq!(invalid.len(), 2);
/// ```
pub fn partition_parallel<T, V>(validator: &V, values: &[T]) -> (Vec<T>, Vec<(T, ValidationError)>)
where
	T: Clone + Sync + Send,
	V: Validator<T> + Sync,
{
	let result = validate_all_parallel(validator, values);
	(result.valid, result.invalid)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::validators::string::MinLengthValidator;

	#[test]
	fn test_parallel_validation_result_new() {
		let result: ParallelValidationResult<String> = ParallelValidationResult::new();
		assert!(result.valid.is_empty());
		assert!(result.invalid.is_empty());
		assert!(result.all_valid());
		assert!(!result.any_valid());
		assert!(result.all_invalid());
	}

	#[test]
	fn test_parallel_validation_result_counts() {
		let mut result: ParallelValidationResult<String> = ParallelValidationResult::new();
		result.valid.push("hello".to_string());
		result.valid.push("world".to_string());
		result.invalid.push((
			"hi".to_string(),
			ValidationError::Custom("test".to_string()),
		));

		assert_eq!(result.valid_count(), 2);
		assert_eq!(result.invalid_count(), 1);
		assert_eq!(result.total(), 3);
		assert!(!result.all_valid());
		assert!(result.any_valid());
		assert!(!result.all_invalid());
	}

	#[test]
	fn test_parallel_validation_result_success_rate() {
		let mut result: ParallelValidationResult<String> = ParallelValidationResult::new();
		result.valid.push("hello".to_string());
		result.valid.push("world".to_string());
		result.invalid.push((
			"hi".to_string(),
			ValidationError::Custom("test".to_string()),
		));

		let rate = result.success_rate();
		assert!((rate - 0.6666).abs() < 0.01);
	}

	#[test]
	fn test_parallel_validation_result_empty_success_rate() {
		let result: ParallelValidationResult<String> = ParallelValidationResult::new();
		assert_eq!(result.success_rate(), 1.0);
	}

	#[test]
	fn test_parallel_validation_result_errors() {
		let mut result: ParallelValidationResult<String> = ParallelValidationResult::new();
		result.invalid.push((
			"a".to_string(),
			ValidationError::Custom("error1".to_string()),
		));
		result.invalid.push((
			"b".to_string(),
			ValidationError::Custom("error2".to_string()),
		));

		let errors = result.errors();
		assert_eq!(errors.len(), 2);
		assert!(result.first_error().is_some());
	}

	#[test]
	fn test_multi_validator_result_new() {
		let result = MultiValidatorResult::new();
		assert!(result.passed.is_empty());
		assert!(result.failed.is_empty());
		assert!(result.all_passed());
		assert!(!result.any_passed());
		assert!(result.all_failed());
	}

	#[test]
	fn test_multi_validator_result_counts() {
		let mut result = MultiValidatorResult::new();
		result.passed.push(0);
		result.passed.push(1);
		result
			.failed
			.push((2, ValidationError::Custom("test".to_string())));

		assert_eq!(result.passed_count(), 2);
		assert_eq!(result.failed_count(), 1);
		assert!(!result.all_passed());
		assert!(result.any_passed());
		assert!(!result.all_failed());
	}

	#[test]
	fn test_multi_validator_result_to_result() {
		let mut result = MultiValidatorResult::new();
		result.passed.push(0);
		assert!(result.to_result().is_ok());

		result
			.failed
			.push((1, ValidationError::Custom("error".to_string())));
		assert!(result.to_result().is_err());
	}

	#[test]
	fn test_parallel_validation_options_builder() {
		let options = ParallelValidationOptions::new()
			.min_batch_size(100)
			.fail_fast(true);

		assert_eq!(options.min_batch_size, 100);
		assert!(options.fail_fast);
	}

	#[test]
	fn test_validate_all_parallel_small() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi", "world"]
			.into_iter()
			.map(String::from)
			.collect();

		let result = validate_all_parallel(&validator, &values);
		assert_eq!(result.valid_count(), 2);
		assert_eq!(result.invalid_count(), 1);
	}

	#[test]
	fn test_validate_all_parallel_large() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = (0..100)
			.map(|i| {
				if i % 2 == 0 {
					"hello".to_string()
				} else {
					"hi".to_string()
				}
			})
			.collect();

		let result = validate_all_parallel(&validator, &values);
		assert_eq!(result.valid_count(), 50);
		assert_eq!(result.invalid_count(), 50);
	}

	#[test]
	fn test_all_valid_parallel() {
		let validator = MinLengthValidator::new(3);

		let valid_values: Vec<String> = vec!["hello", "world", "rust"]
			.into_iter()
			.map(String::from)
			.collect();
		assert!(all_valid_parallel(&validator, &valid_values));

		let mixed_values: Vec<String> = vec!["hello", "hi", "world"]
			.into_iter()
			.map(String::from)
			.collect();
		assert!(!all_valid_parallel(&validator, &mixed_values));
	}

	#[test]
	fn test_any_valid_parallel() {
		let validator = MinLengthValidator::new(3);

		let values: Vec<String> = vec!["hi", "a", "hello"]
			.into_iter()
			.map(String::from)
			.collect();
		assert!(any_valid_parallel(&validator, &values));

		let all_invalid: Vec<String> = vec!["hi", "a", "b"].into_iter().map(String::from).collect();
		assert!(!any_valid_parallel(&validator, &all_invalid));
	}

	#[test]
	fn test_find_first_error_parallel() {
		let validator = MinLengthValidator::new(3);

		let values: Vec<String> = vec!["hello", "world", "rust"]
			.into_iter()
			.map(String::from)
			.collect();
		assert!(find_first_error_parallel(&validator, &values).is_none());

		let mixed: Vec<String> = vec!["hello", "hi", "world"]
			.into_iter()
			.map(String::from)
			.collect();
		assert!(find_first_error_parallel(&validator, &mixed).is_some());
	}

	#[test]
	fn test_count_valid_parallel() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi", "world", "a"]
			.into_iter()
			.map(String::from)
			.collect();

		assert_eq!(count_valid_parallel(&validator, &values), 2);
	}

	#[test]
	fn test_validate_with_multiple() {
		let validators: Vec<MinLengthValidator> =
			vec![MinLengthValidator::new(3), MinLengthValidator::new(5)];

		let result = validate_with_multiple(&validators, "hello");
		assert!(result.all_passed());

		let result2 = validate_with_multiple(&validators, "hi");
		assert!(result2.all_failed());

		let result3 = validate_with_multiple(&validators, "hey");
		assert_eq!(result3.passed_count(), 1);
		assert_eq!(result3.failed_count(), 1);
	}

	#[test]
	fn test_filter_valid_parallel() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi", "world", "a"]
			.into_iter()
			.map(String::from)
			.collect();

		let valid = filter_valid_parallel(&validator, &values);
		assert_eq!(valid.len(), 2);
		assert!(valid.contains(&"hello".to_string()));
		assert!(valid.contains(&"world".to_string()));
	}

	#[test]
	fn test_filter_invalid_parallel() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi", "world", "a"]
			.into_iter()
			.map(String::from)
			.collect();

		let invalid = filter_invalid_parallel(&validator, &values);
		assert_eq!(invalid.len(), 2);
	}

	#[test]
	fn test_partition_parallel() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi", "world", "a"]
			.into_iter()
			.map(String::from)
			.collect();

		let (valid, invalid) = partition_parallel(&validator, &values);
		assert_eq!(valid.len(), 2);
		assert_eq!(invalid.len(), 2);
	}

	#[test]
	fn test_validate_with_options() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "hi"].into_iter().map(String::from).collect();
		let options = ParallelValidationOptions::new().min_batch_size(1);

		let result = validate_all_parallel_with_options(&validator, &values, &options);
		assert_eq!(result.valid_count(), 1);
		assert_eq!(result.invalid_count(), 1);
	}

	#[test]
	fn test_all_valid_parallel_with_options() {
		let validator = MinLengthValidator::new(3);
		let values: Vec<String> = vec!["hello", "world"]
			.into_iter()
			.map(String::from)
			.collect();
		let options = ParallelValidationOptions::new().min_batch_size(1);

		assert!(all_valid_parallel_with_options(
			&validator, &values, &options
		));
	}
}
