//! Factory registry for dynamic factory discovery.
//!
//! This module provides a global registry for factories, enabling
//! lookup by model identifier or type.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;
use parking_lot::RwLock;

use super::traits::Factory;
// crate::error::SeedingError is available but not currently used

/// Type-erased factory wrapper.
pub trait AnyFactory: Send + Sync {
	/// Returns the model identifier.
	fn model_id(&self) -> &str;

	/// Returns the factory as an Any reference for downcasting.
	fn as_any(&self) -> &dyn Any;
}

/// Wrapper to store factories with type information.
struct FactoryEntry<F: Factory + 'static> {
	model_id: String,
	factory: F,
}

impl<F: Factory + 'static> AnyFactory for FactoryEntry<F> {
	fn model_id(&self) -> &str {
		&self.model_id
	}

	fn as_any(&self) -> &dyn Any {
		&self.factory
	}
}

/// Global factory registry.
static FACTORY_REGISTRY: Lazy<RwLock<HashMap<String, Arc<dyn AnyFactory>>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// Type ID to model ID mapping.
static TYPE_FACTORY_MAP: Lazy<RwLock<HashMap<TypeId, String>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// Registers a factory in the global registry.
///
/// # Arguments
///
/// * `model_id` - Model identifier (e.g., "auth.User")
/// * `factory` - Factory instance
///
/// # Example
///
/// ```ignore
/// register_factory("auth.User", UserFactory::new());
/// ```
pub fn register_factory<F: Factory + 'static>(model_id: impl Into<String>, factory: F) {
	let model_id = model_id.into();
	let entry = FactoryEntry {
		model_id: model_id.clone(),
		factory,
	};
	FACTORY_REGISTRY.write().insert(model_id, Arc::new(entry));
}

/// Registers a factory with type information.
///
/// This allows retrieval by model type in addition to model ID.
///
/// # Type Parameters
///
/// * `M` - The model type
/// * `F` - The factory type
pub fn register_factory_for_type<M: 'static, F: Factory<Model = M> + 'static>(
	model_id: impl Into<String>,
	factory: F,
) {
	let model_id = model_id.into();
	TYPE_FACTORY_MAP
		.write()
		.insert(TypeId::of::<M>(), model_id.clone());
	register_factory(model_id, factory);
}

/// Gets a factory by model identifier.
///
/// # Arguments
///
/// * `model_id` - Model identifier
///
/// # Returns
///
/// Returns the factory if registered.
pub fn get_factory(model_id: &str) -> Option<Arc<dyn AnyFactory>> {
	FACTORY_REGISTRY.read().get(model_id).cloned()
}

/// Gets a factory by model type.
///
/// # Type Parameters
///
/// * `M` - The model type
pub fn get_factory_for_type<M: 'static>() -> Option<Arc<dyn AnyFactory>> {
	let model_id = TYPE_FACTORY_MAP.read().get(&TypeId::of::<M>()).cloned()?;
	get_factory(&model_id)
}

// Note: get_typed_factory is not implemented due to lifetime constraints.
// Use get_factory() and work with the AnyFactory trait instead.

/// Checks if a factory is registered for the model identifier.
pub fn has_factory(model_id: &str) -> bool {
	FACTORY_REGISTRY.read().contains_key(model_id)
}

/// Returns all registered model identifiers.
pub fn factory_model_ids() -> Vec<String> {
	FACTORY_REGISTRY.read().keys().cloned().collect()
}

/// Clears all registered factories.
///
/// This is primarily useful for testing.
pub fn clear_factories() {
	FACTORY_REGISTRY.write().clear();
	TYPE_FACTORY_MAP.write().clear();
}

/// Returns the number of registered factories.
pub fn factory_count() -> usize {
	FACTORY_REGISTRY.read().len()
}

/// Factory registry handle for scoped operations.
#[derive(Debug, Default)]
pub struct FactoryRegistry;

impl FactoryRegistry {
	/// Creates a new registry handle.
	pub fn new() -> Self {
		Self
	}

	/// Gets a factory by model identifier.
	pub fn get(&self, model_id: &str) -> Option<Arc<dyn AnyFactory>> {
		get_factory(model_id)
	}

	/// Gets a factory by model type.
	pub fn get_for_type<M: 'static>(&self) -> Option<Arc<dyn AnyFactory>> {
		get_factory_for_type::<M>()
	}

	/// Checks if a factory is registered.
	pub fn has(&self, model_id: &str) -> bool {
		has_factory(model_id)
	}

	/// Returns all registered model IDs.
	pub fn model_ids(&self) -> Vec<String> {
		factory_model_ids()
	}

	/// Returns the number of registered factories.
	pub fn len(&self) -> usize {
		factory_count()
	}

	/// Returns true if no factories are registered.
	pub fn is_empty(&self) -> bool {
		factory_count() == 0
	}

	/// Clears all factories (primarily for testing).
	pub fn clear(&self) {
		clear_factories();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::error::SeedingResult;
	use rstest::rstest;

	// Test model - name field used in factory build()
	#[derive(Debug, Clone)]
	#[allow(dead_code)]
	struct TestModel {
		name: String,
	}

	// Test factory
	struct TestFactory {
		name: String,
	}

	impl TestFactory {
		fn new(name: &str) -> Self {
			Self {
				name: name.to_string(),
			}
		}
	}

	impl Factory for TestFactory {
		type Model = TestModel;

		fn build(&self) -> TestModel {
			TestModel {
				name: self.name.clone(),
			}
		}

		async fn create(&self) -> SeedingResult<TestModel> {
			Ok(self.build())
		}

		async fn create_batch(&self, count: usize) -> SeedingResult<Vec<TestModel>> {
			Ok(self.build_batch(count))
		}
	}

	#[rstest]
	fn test_register_and_get_factory() {
		clear_factories();

		register_factory("test.Model", TestFactory::new("test"));

		assert!(has_factory("test.Model"));
		assert!(!has_factory("other.Model"));

		let factory = get_factory("test.Model").unwrap();
		assert_eq!(factory.model_id(), "test.Model");
	}

	#[rstest]
	fn test_register_for_type() {
		clear_factories();

		register_factory_for_type::<TestModel, _>("typed.Model", TestFactory::new("typed"));

		let factory = get_factory_for_type::<TestModel>().unwrap();
		assert_eq!(factory.model_id(), "typed.Model");
	}

	#[rstest]
	fn test_factory_model_ids() {
		clear_factories();

		register_factory("app1.Model", TestFactory::new("1"));
		register_factory("app2.Model", TestFactory::new("2"));

		let ids = factory_model_ids();
		assert_eq!(ids.len(), 2);
		assert!(ids.contains(&"app1.Model".to_string()));
		assert!(ids.contains(&"app2.Model".to_string()));
	}

	#[rstest]
	fn test_factory_registry_handle() {
		clear_factories();

		let registry = FactoryRegistry::new();
		assert!(registry.is_empty());

		register_factory("handle.Model", TestFactory::new("handle"));

		assert!(!registry.is_empty());
		assert_eq!(registry.len(), 1);
		assert!(registry.has("handle.Model"));
		assert!(registry.get("handle.Model").is_some());
	}

	#[rstest]
	fn test_clear_factories() {
		clear_factories();

		register_factory("clear.Model", TestFactory::new("clear"));
		assert!(has_factory("clear.Model"));

		clear_factories();
		assert!(!has_factory("clear.Model"));
	}
}
