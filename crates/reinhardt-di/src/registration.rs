//! Deferred DI registration list for propagating singleton registrations
//! across module boundaries.
//!
//! When route configuration functions (e.g., `routes()`) need to register
//! singletons for DI, the registrations cannot be applied immediately because
//! the server's [`SingletonScope`] does not exist yet. Instead, registrations
//! are captured as deferred closures and applied later during server startup.
//!
//! # Example
//!
//! ```
//! use reinhardt_di::{DiRegistrationList, SingletonScope};
//! use std::sync::Arc;
//!
//! // In routes() configuration
//! let mut registrations = DiRegistrationList::new();
//! registrations.register_arc(Arc::new(42i32));
//!
//! // Later, during server startup
//! let scope = SingletonScope::new();
//! registrations.apply_to(&scope);
//!
//! assert_eq!(*scope.get::<i32>().unwrap(), 42);
//! ```

use std::any::Any;
use std::sync::Arc;

use crate::SingletonScope;

/// Type alias for a boxed registration closure.
type Registration = Box<dyn FnOnce(&SingletonScope) + Send + Sync>;

/// A deferred list of DI singleton registrations.
///
/// Captures type-erased registration closures that are applied to a
/// [`SingletonScope`] at a later point (typically during server startup).
/// This bridges the gap between synchronous route configuration and
/// the server's DI scope lifecycle.
pub struct DiRegistrationList {
	registrations: Vec<Registration>,
}

impl DiRegistrationList {
	/// Creates an empty registration list.
	pub fn new() -> Self {
		Self {
			registrations: Vec::new(),
		}
	}

	/// Captures a pre-wrapped `Arc<T>` for deferred singleton registration.
	///
	/// When [`apply_to`](Self::apply_to) is called, the value will be
	/// registered via [`SingletonScope::set_arc`].
	pub fn register_arc<T: Any + Send + Sync + 'static>(&mut self, value: Arc<T>) {
		self.registrations
			.push(Box::new(move |scope: &SingletonScope| {
				scope.set_arc(value);
			}));
	}

	/// Captures a value for deferred singleton registration.
	///
	/// When [`apply_to`](Self::apply_to) is called, the value will be
	/// registered via [`SingletonScope::set`].
	pub fn register<T: Any + Send + Sync + 'static>(&mut self, value: T) {
		self.registrations
			.push(Box::new(move |scope: &SingletonScope| {
				scope.set(value);
			}));
	}

	/// Applies all deferred registrations to the given scope.
	///
	/// Consumes the list, executing each captured closure against the scope.
	/// Registrations are applied in the order they were added.
	pub fn apply_to(self, scope: &SingletonScope) {
		for registration in self.registrations {
			registration(scope);
		}
	}

	/// Merges another registration list into this one.
	///
	/// All registrations from `other` are appended to this list.
	pub fn merge(&mut self, other: DiRegistrationList) {
		self.registrations.extend(other.registrations);
	}

	/// Returns `true` if this list contains no registrations.
	pub fn is_empty(&self) -> bool {
		self.registrations.is_empty()
	}

	/// Returns the number of pending registrations.
	pub fn len(&self) -> usize {
		self.registrations.len()
	}
}

impl Default for DiRegistrationList {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for DiRegistrationList {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DiRegistrationList")
			.field("count", &self.registrations.len())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_empty_list() {
		let list = DiRegistrationList::new();
		assert!(list.is_empty());
		assert_eq!(list.len(), 0);
	}

	#[test]
	fn test_register_arc_and_apply() {
		let mut list = DiRegistrationList::new();
		list.register_arc(Arc::new(42i32));
		list.register_arc(Arc::new("hello".to_string()));

		assert_eq!(list.len(), 2);

		let scope = SingletonScope::new();
		list.apply_to(&scope);

		assert_eq!(*scope.get::<i32>().unwrap(), 42);
		assert_eq!(*scope.get::<String>().unwrap(), "hello");
	}

	#[test]
	fn test_register_value_and_apply() {
		let mut list = DiRegistrationList::new();
		list.register(100u64);

		let scope = SingletonScope::new();
		list.apply_to(&scope);

		assert_eq!(*scope.get::<u64>().unwrap(), 100);
	}

	#[test]
	fn test_merge() {
		let mut list_a = DiRegistrationList::new();
		list_a.register_arc(Arc::new(1i32));

		let mut list_b = DiRegistrationList::new();
		list_b.register_arc(Arc::new("merged".to_string()));

		list_a.merge(list_b);
		assert_eq!(list_a.len(), 2);

		let scope = SingletonScope::new();
		list_a.apply_to(&scope);

		assert_eq!(*scope.get::<i32>().unwrap(), 1);
		assert_eq!(*scope.get::<String>().unwrap(), "merged");
	}

	#[test]
	fn test_apply_empty_is_noop() {
		let list = DiRegistrationList::new();
		let scope = SingletonScope::new();
		list.apply_to(&scope);
		// No panic, no side effects
		assert!(scope.get::<i32>().is_none());
	}
}
