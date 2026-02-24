//! Override registry for dependency injection
//!
//! This module provides a registry for storing override values that take precedence
//! over normal dependency resolution. Overrides are keyed by function pointer addresses,
//! allowing specific injectable functions to be mocked in tests.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// Override registry for dependency injection.
///
/// Stores override values keyed by function pointer addresses (usize).
/// This allows specific injectable functions to be mocked while leaving
/// other functions with the same return type unaffected.
///
/// # Thread Safety
///
/// The registry uses `RwLock` internally, making it safe to use from multiple threads.
///
/// # Examples
///
/// ```rust
/// use reinhardt_di::OverrideRegistry;
///
/// let registry = OverrideRegistry::new();
///
/// // Use function pointer as key
/// fn create_database() -> String { "production".to_string() }
/// let func_ptr = create_database as usize;
///
/// // Set override
/// registry.set(func_ptr, "test".to_string());
///
/// // Get override
/// let value: Option<String> = registry.get(func_ptr);
/// assert_eq!(value, Some("test".to_string()));
/// ```
#[derive(Default)]
pub struct OverrideRegistry {
	/// Function pointer address → Override value
	overrides: RwLock<HashMap<usize, Arc<dyn Any + Send + Sync>>>,
}

impl OverrideRegistry {
	/// Creates a new empty override registry.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// ```
	pub fn new() -> Self {
		Self {
			overrides: RwLock::new(HashMap::new()),
		}
	}

	/// Sets an override value for the given function pointer.
	///
	/// # Arguments
	///
	/// * `func_ptr` - The function pointer address as usize
	/// * `value` - The override value to store
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// fn my_factory() -> i32 { 42 }
	///
	/// registry.set(my_factory as usize, 100i32);
	/// ```
	pub fn set<O: Clone + Send + Sync + 'static>(&self, func_ptr: usize, value: O) {
		self.overrides
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(func_ptr, Arc::new(value));
	}

	/// Gets an override value for the given function pointer.
	///
	/// Returns `None` if no override is set or if the type doesn't match.
	///
	/// # Arguments
	///
	/// * `func_ptr` - The function pointer address as usize
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// fn my_factory() -> i32 { 42 }
	///
	/// registry.set(my_factory as usize, 100i32);
	/// let value: Option<i32> = registry.get(my_factory as usize);
	/// assert_eq!(value, Some(100));
	/// ```
	pub fn get<O: Clone + 'static>(&self, func_ptr: usize) -> Option<O> {
		self.overrides
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(&func_ptr)
			.and_then(|arc| arc.downcast_ref::<O>().cloned())
	}

	/// Removes an override for the given function pointer.
	///
	/// # Arguments
	///
	/// * `func_ptr` - The function pointer address as usize
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// fn my_factory() -> i32 { 42 }
	///
	/// registry.set(my_factory as usize, 100i32);
	/// registry.remove(my_factory as usize);
	/// let value: Option<i32> = registry.get(my_factory as usize);
	/// assert!(value.is_none());
	/// ```
	pub fn remove(&self, func_ptr: usize) {
		self.overrides
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&func_ptr);
	}

	/// Clears all overrides from the registry.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// fn factory1() -> i32 { 1 }
	/// fn factory2() -> i32 { 2 }
	///
	/// registry.set(factory1 as usize, 10i32);
	/// registry.set(factory2 as usize, 20i32);
	/// registry.clear();
	///
	/// let v1: Option<i32> = registry.get(factory1 as usize);
	/// let v2: Option<i32> = registry.get(factory2 as usize);
	/// assert!(v1.is_none());
	/// assert!(v2.is_none());
	/// ```
	pub fn clear(&self) {
		self.overrides
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.clear();
	}

	/// Checks if an override exists for the given function pointer.
	///
	/// # Arguments
	///
	/// * `func_ptr` - The function pointer address as usize
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// fn my_factory() -> i32 { 42 }
	///
	/// assert!(!registry.has(my_factory as usize));
	/// registry.set(my_factory as usize, 100i32);
	/// assert!(registry.has(my_factory as usize));
	/// ```
	pub fn has(&self, func_ptr: usize) -> bool {
		self.overrides
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.contains_key(&func_ptr)
	}

	/// Returns the number of overrides in the registry.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// assert_eq!(registry.len(), 0);
	///
	/// fn my_factory() -> i32 { 42 }
	/// registry.set(my_factory as usize, 100i32);
	/// assert_eq!(registry.len(), 1);
	/// ```
	pub fn len(&self) -> usize {
		self.overrides
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.len()
	}

	/// Returns true if the registry contains no overrides.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::OverrideRegistry;
	///
	/// let registry = OverrideRegistry::new();
	/// assert!(registry.is_empty());
	///
	/// fn my_factory() -> i32 { 42 }
	/// registry.set(my_factory as usize, 100i32);
	/// assert!(!registry.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.overrides
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn factory_a() -> String {
		"production_a".to_string()
	}

	fn factory_b() -> String {
		"production_b".to_string()
	}

	#[test]
	fn test_set_and_get_override() {
		let registry = OverrideRegistry::new();

		registry.set(factory_a as usize, "mock_a".to_string());

		let value: Option<String> = registry.get(factory_a as usize);
		assert_eq!(value, Some("mock_a".to_string()));
	}

	#[test]
	fn test_different_functions_same_return_type() {
		let registry = OverrideRegistry::new();

		registry.set(factory_a as usize, "mock_a".to_string());
		registry.set(factory_b as usize, "mock_b".to_string());

		let value_a: Option<String> = registry.get(factory_a as usize);
		let value_b: Option<String> = registry.get(factory_b as usize);

		assert_eq!(value_a, Some("mock_a".to_string()));
		assert_eq!(value_b, Some("mock_b".to_string()));
	}

	#[test]
	fn test_get_nonexistent_returns_none() {
		let registry = OverrideRegistry::new();

		let value: Option<String> = registry.get(factory_a as usize);
		assert!(value.is_none());
	}

	#[test]
	fn test_remove_override() {
		let registry = OverrideRegistry::new();

		registry.set(factory_a as usize, "mock_a".to_string());
		assert!(registry.has(factory_a as usize));

		registry.remove(factory_a as usize);
		assert!(!registry.has(factory_a as usize));

		let value: Option<String> = registry.get(factory_a as usize);
		assert!(value.is_none());
	}

	#[test]
	fn test_clear_overrides() {
		let registry = OverrideRegistry::new();

		registry.set(factory_a as usize, "mock_a".to_string());
		registry.set(factory_b as usize, "mock_b".to_string());
		assert_eq!(registry.len(), 2);

		registry.clear();
		assert!(registry.is_empty());
	}

	#[test]
	fn test_type_mismatch_returns_none() {
		let registry = OverrideRegistry::new();

		fn int_factory() -> i32 {
			42
		}

		registry.set(int_factory as usize, 100i32);

		// Try to get as wrong type
		let value: Option<String> = registry.get(int_factory as usize);
		assert!(value.is_none());

		// Correct type works
		let value: Option<i32> = registry.get(int_factory as usize);
		assert_eq!(value, Some(100));
	}
}
