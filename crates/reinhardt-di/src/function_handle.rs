//! Function handle for fluent override API
//!
//! This module provides `FunctionHandle`, a fluent API for setting
//! and managing dependency overrides keyed by function pointer.

use std::marker::PhantomData;

use crate::InjectionContext;

/// A handle for managing function-based dependency overrides.
///
/// `FunctionHandle` provides a fluent API for setting overrides on specific
/// injectable functions. This is useful in tests where you want to mock
/// specific dependency factories while leaving others unaffected.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the borrowed `InjectionContext`
/// * `O` - The output type of the function (the dependency type)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_di::{InjectionContext, SingletonScope};
/// use std::sync::Arc;
///
/// # #[derive(Clone)]
/// # struct Database;
/// # impl Database {
/// #     fn connect(_url: &str) -> Self { Database }
/// #     fn mock() -> Self { Database }
/// # }
/// # fn create_database() -> Database { Database::connect("production://db") }
///
/// let singleton = Arc::new(SingletonScope::new());
/// let ctx = InjectionContext::builder(singleton).build();
///
/// // Set override using fluent API
/// ctx.dependency(create_database).override_with(Database::mock());
/// ```
pub struct FunctionHandle<'a, O> {
	ctx: &'a InjectionContext,
	func_ptr: usize,
	_marker: PhantomData<O>,
}

impl<'a, O: Clone + Send + Sync + 'static> FunctionHandle<'a, O> {
	/// Creates a new function handle.
	///
	/// This is typically called internally by `InjectionContext::dependency`.
	pub(crate) fn new(ctx: &'a InjectionContext, func_ptr: usize) -> Self {
		Self {
			ctx,
			func_ptr,
			_marker: PhantomData,
		}
	}

	/// Sets an override value for this function.
	///
	/// When the injectable function is invoked, this override value will be
	/// returned instead of calling the actual function implementation.
	///
	/// # Arguments
	///
	/// * `value` - The value to return when this function is called
	///
	/// # Returns
	///
	/// A reference to self for method chaining.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_di::{InjectionContext, SingletonScope};
	/// # use std::sync::Arc;
	/// # #[derive(Clone)]
	/// # struct Database;
	/// # fn create_database() -> Database { Database }
	/// # let singleton = Arc::new(SingletonScope::new());
	/// # let ctx = InjectionContext::builder(singleton).build();
	/// # let mock_database = Database;
	/// ctx.dependency(create_database)
	///    .override_with(mock_database);
	/// ```
	pub fn override_with(&self, value: O) -> &Self {
		self.ctx.overrides().set(self.func_ptr, value);
		self
	}

	/// Clears the override for this function.
	///
	/// After calling this method, the actual function implementation will be
	/// used when resolving this dependency.
	///
	/// # Returns
	///
	/// A reference to self for method chaining.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_di::{InjectionContext, SingletonScope};
	/// # use std::sync::Arc;
	/// # #[derive(Clone)]
	/// # struct Database;
	/// # fn create_database() -> Database { Database }
	/// # let singleton = Arc::new(SingletonScope::new());
	/// # let ctx = InjectionContext::builder(singleton).build();
	/// # let mock_database = Database;
	/// // Set override
	/// ctx.dependency(create_database).override_with(mock_database);
	///
	/// // Later, clear it
	/// ctx.dependency(create_database).clear_override();
	/// ```
	pub fn clear_override(&self) -> &Self {
		self.ctx.overrides().remove(self.func_ptr);
		self
	}

	/// Checks if an override exists for this function.
	///
	/// # Returns
	///
	/// `true` if an override is set, `false` otherwise.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_di::{InjectionContext, SingletonScope};
	/// # use std::sync::Arc;
	/// # #[derive(Clone)]
	/// # struct Database;
	/// # fn create_database() -> Database { Database }
	/// # let singleton = Arc::new(SingletonScope::new());
	/// # let ctx = InjectionContext::builder(singleton).build();
	/// # let mock_database = Database;
	/// assert!(!ctx.dependency(create_database).has_override());
	/// ctx.dependency(create_database).override_with(mock_database);
	/// assert!(ctx.dependency(create_database).has_override());
	/// ```
	pub fn has_override(&self) -> bool {
		self.ctx.overrides().has(self.func_ptr)
	}

	/// Gets the current override value for this function, if any.
	///
	/// # Returns
	///
	/// `Some(value)` if an override is set, `None` otherwise.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_di::{InjectionContext, SingletonScope};
	/// # use std::sync::Arc;
	/// # #[derive(Clone, PartialEq, Debug)]
	/// # struct Database;
	/// # fn create_database() -> Database { Database }
	/// # let singleton = Arc::new(SingletonScope::new());
	/// # let ctx = InjectionContext::builder(singleton).build();
	/// # let mock_database = Database;
	/// ctx.dependency(create_database).override_with(mock_database.clone());
	/// let value = ctx.dependency(create_database).get_override();
	/// assert_eq!(value, Some(mock_database));
	/// ```
	pub fn get_override(&self) -> Option<O> {
		self.ctx.overrides().get(self.func_ptr)
	}

	/// Returns the function pointer address for this handle.
	///
	/// This is primarily useful for debugging or advanced use cases.
	pub fn func_ptr(&self) -> usize {
		self.func_ptr
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use crate::{InjectionContext, SingletonScope};

	fn create_string() -> String {
		"production".to_string()
	}

	fn create_other_string() -> String {
		"other_production".to_string()
	}

	#[test]
	fn test_override_with() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton).build();

		ctx.dependency(create_string)
			.override_with("mock".to_string());

		assert!(ctx.dependency(create_string).has_override());
		assert_eq!(
			ctx.dependency(create_string).get_override(),
			Some("mock".to_string())
		);
	}

	#[test]
	fn test_clear_override() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton).build();

		ctx.dependency(create_string)
			.override_with("mock".to_string());
		assert!(ctx.dependency(create_string).has_override());

		ctx.dependency(create_string).clear_override();
		assert!(!ctx.dependency(create_string).has_override());
	}

	#[test]
	fn test_different_functions_independent() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton).build();

		ctx.dependency(create_string)
			.override_with("mock_1".to_string());
		ctx.dependency(create_other_string)
			.override_with("mock_2".to_string());

		assert_eq!(
			ctx.dependency(create_string).get_override(),
			Some("mock_1".to_string())
		);
		assert_eq!(
			ctx.dependency(create_other_string).get_override(),
			Some("mock_2".to_string())
		);
	}

	#[test]
	fn test_func_ptr() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton).build();

		let handle = ctx.dependency(create_string);
		assert_eq!(handle.func_ptr(), create_string as *const () as usize);
	}
}
