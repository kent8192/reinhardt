//! Lightweight DI container for mock handler test contexts.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Lightweight type-keyed container for injecting test dependencies
/// into server_fn mock handlers.
#[derive(Default)]
pub struct TestContext {
	values: HashMap<TypeId, Box<dyn Any>>,
}

impl TestContext {
	/// Create an empty context.
	pub fn new() -> Self {
		Self::default()
	}

	/// Insert a value keyed by its type. Consumes self for builder pattern.
	pub fn insert<T: 'static>(mut self, value: T) -> Self {
		self.values.insert(TypeId::of::<T>(), Box::new(value));
		self
	}

	/// Retrieve a reference to a stored value.
	///
	/// # Panics
	///
	/// Panics if the type was not previously inserted.
	pub fn get<T: 'static>(&self) -> &T {
		self.try_get::<T>().unwrap_or_else(|| {
			panic!(
				"TestContext: type not found: {}",
				std::any::type_name::<T>()
			)
		})
	}

	/// Try to retrieve a reference to a stored value.
	/// Returns `None` if the type was not previously inserted.
	pub fn try_get<T: 'static>(&self) -> Option<&T> {
		self.values
			.get(&TypeId::of::<T>())
			.and_then(|v| v.downcast_ref::<T>())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	struct MockDb {
		users: Vec<String>,
	}

	struct MockCache {
		ttl: u32,
	}

	#[rstest]
	fn insert_and_get() {
		let ctx = TestContext::new().insert(MockDb {
			users: vec!["Alice".into()],
		});
		let db = ctx.get::<MockDb>();
		assert_eq!(db.users.len(), 1);
		assert_eq!(db.users[0], "Alice");
	}

	#[rstest]
	fn multiple_types() {
		let ctx = TestContext::new()
			.insert(MockDb { users: vec![] })
			.insert(MockCache { ttl: 300 });
		assert_eq!(ctx.get::<MockCache>().ttl, 300);
		assert!(ctx.get::<MockDb>().users.is_empty());
	}

	#[rstest]
	fn try_get_returns_none_for_missing() {
		let ctx = TestContext::new();
		assert!(ctx.try_get::<MockDb>().is_none());
	}

	#[rstest]
	#[should_panic(expected = "TestContext: type not found")]
	fn get_panics_for_missing() {
		let ctx = TestContext::new();
		ctx.get::<MockDb>();
	}
}
