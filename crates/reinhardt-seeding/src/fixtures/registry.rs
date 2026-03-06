//! Model registry for fixture loading.
//!
//! This module provides a global registry for model loaders that handle
//! the conversion of fixture records to database operations.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use parking_lot::RwLock;

use super::FixtureRecord;
use crate::error::{SeedingError, SeedingResult};

/// Trait for loading fixture records into the database.
///
/// Implement this trait for each model type that should support fixture loading.
#[async_trait]
pub trait ModelLoader: Send + Sync {
	/// Returns the model identifier (e.g., "auth.User").
	fn model_id(&self) -> &str;

	/// Loads a single fixture record into the database.
	///
	/// # Arguments
	///
	/// * `record` - The fixture record to load
	///
	/// # Returns
	///
	/// Returns the primary key of the inserted record on success.
	async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value>;

	/// Loads multiple fixture records into the database.
	///
	/// The default implementation loads records sequentially.
	/// Override this for batch loading optimization.
	///
	/// # Arguments
	///
	/// * `records` - The fixture records to load
	///
	/// # Returns
	///
	/// Returns the count of successfully loaded records.
	async fn load_records(&self, records: &[FixtureRecord]) -> SeedingResult<usize> {
		let mut count = 0;
		for record in records {
			self.load_record(record).await?;
			count += 1;
		}
		Ok(count)
	}

	/// Returns true if this loader supports batch operations.
	fn supports_batch(&self) -> bool {
		false
	}
}

/// Global registry for model loaders.
static MODEL_REGISTRY: Lazy<RwLock<HashMap<String, Arc<dyn ModelLoader>>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// Type ID registry for generic type resolution.
static TYPE_ID_REGISTRY: Lazy<RwLock<HashMap<TypeId, String>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// Registers a model loader in the global registry.
///
/// # Arguments
///
/// * `loader` - The model loader to register
///
/// # Example
///
/// ```ignore
/// struct UserLoader;
///
/// #[async_trait]
/// impl ModelLoader for UserLoader {
///     fn model_id(&self) -> &str { "auth.User" }
///     async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value> {
///         // Load user from fixture
///         todo!()
///     }
/// }
///
/// register_model_loader(UserLoader);
/// ```
pub fn register_model_loader<L: ModelLoader + 'static>(loader: L) {
	let model_id = loader.model_id().to_string();
	MODEL_REGISTRY.write().insert(model_id, Arc::new(loader));
}

/// Registers a model loader with type information for generic resolution.
///
/// # Type Parameters
///
/// * `M` - The model type this loader handles
/// * `L` - The loader implementation type
#[allow(dead_code)]
pub(crate) fn register_model_loader_for_type<M: 'static, L: ModelLoader + 'static>(loader: L) {
	let model_id = loader.model_id().to_string();
	TYPE_ID_REGISTRY
		.write()
		.insert(TypeId::of::<M>(), model_id.clone());
	MODEL_REGISTRY.write().insert(model_id, Arc::new(loader));
}

/// Model registry providing access to registered loaders.
#[derive(Debug, Default)]
pub struct ModelRegistry;

impl ModelRegistry {
	/// Creates a new model registry reference.
	pub fn new() -> Self {
		Self
	}

	/// Gets a loader for the specified model identifier.
	///
	/// # Arguments
	///
	/// * `model_id` - Model identifier (e.g., "auth.User")
	///
	/// # Returns
	///
	/// Returns the loader if registered, `None` otherwise.
	pub fn get_loader(&self, model_id: &str) -> Option<Arc<dyn ModelLoader>> {
		MODEL_REGISTRY.read().get(model_id).cloned()
	}

	/// Gets a loader for the specified model type.
	///
	/// # Type Parameters
	///
	/// * `M` - The model type
	pub fn get_loader_for_type<M: 'static>(&self) -> Option<Arc<dyn ModelLoader>> {
		let type_id = TypeId::of::<M>();
		let model_id = TYPE_ID_REGISTRY.read().get(&type_id).cloned()?;
		self.get_loader(&model_id)
	}

	/// Checks if a loader is registered for the model identifier.
	pub fn has_loader(&self, model_id: &str) -> bool {
		MODEL_REGISTRY.read().contains_key(model_id)
	}

	/// Returns all registered model identifiers.
	pub fn model_ids(&self) -> Vec<String> {
		MODEL_REGISTRY.read().keys().cloned().collect()
	}

	/// Clears all registered loaders.
	///
	/// This is primarily useful for testing.
	pub fn clear(&self) {
		MODEL_REGISTRY.write().clear();
		TYPE_ID_REGISTRY.write().clear();
	}

	/// Returns the number of registered loaders.
	pub fn len(&self) -> usize {
		MODEL_REGISTRY.read().len()
	}

	/// Returns true if no loaders are registered.
	pub fn is_empty(&self) -> bool {
		MODEL_REGISTRY.read().is_empty()
	}

	/// Loads a fixture record using the appropriate loader.
	///
	/// # Arguments
	///
	/// * `record` - The fixture record to load
	///
	/// # Returns
	///
	/// Returns the primary key of the inserted record.
	///
	/// # Errors
	///
	/// Returns an error if no loader is registered for the model.
	pub async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value> {
		let loader = self
			.get_loader(&record.model)
			.ok_or_else(|| SeedingError::ModelNotFound(record.model.clone()))?;
		loader.load_record(record).await
	}

	/// Loads multiple fixture records for the same model.
	///
	/// # Arguments
	///
	/// * `model_id` - Model identifier
	/// * `records` - Fixture records to load
	///
	/// # Returns
	///
	/// Returns the count of successfully loaded records.
	pub async fn load_records(
		&self,
		model_id: &str,
		records: &[FixtureRecord],
	) -> SeedingResult<usize> {
		let loader = self
			.get_loader(model_id)
			.ok_or_else(|| SeedingError::ModelNotFound(model_id.to_string()))?;
		loader.load_records(records).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	struct TestLoader {
		model_id: String,
	}

	impl TestLoader {
		fn new(model_id: &str) -> Self {
			Self {
				model_id: model_id.to_string(),
			}
		}
	}

	#[async_trait]
	impl ModelLoader for TestLoader {
		fn model_id(&self) -> &str {
			&self.model_id
		}

		async fn load_record(&self, record: &FixtureRecord) -> SeedingResult<serde_json::Value> {
			// Return the pk or a generated one
			Ok(record.pk.clone().unwrap_or(json!(1)))
		}
	}

	#[rstest]
	fn test_register_and_get_loader() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(TestLoader::new("test.Model"));

		assert!(registry.has_loader("test.Model"));
		assert!(!registry.has_loader("test.Other"));

		let loader = registry.get_loader("test.Model").unwrap();
		assert_eq!(loader.model_id(), "test.Model");
	}

	#[rstest]
	fn test_model_ids() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(TestLoader::new("app1.Model"));
		register_model_loader(TestLoader::new("app2.Model"));

		let ids = registry.model_ids();
		assert_eq!(ids.len(), 2);
		assert!(ids.contains(&"app1.Model".to_string()));
		assert!(ids.contains(&"app2.Model".to_string()));
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_record() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader(TestLoader::new("test.User"));

		let record = FixtureRecord::with_pk("test.User", json!(42), json!({"name": "test"}));
		let pk = registry.load_record(&record).await.unwrap();
		assert_eq!(pk, json!(42));
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_record_model_not_found() {
		let registry = ModelRegistry::new();
		registry.clear();

		let record = FixtureRecord::new("unknown.Model", json!({}));
		let result = registry.load_record(&record).await;
		assert!(matches!(result, Err(SeedingError::ModelNotFound(_))));
	}

	struct TypedModel;

	#[rstest]
	fn test_register_for_type() {
		let registry = ModelRegistry::new();
		registry.clear();

		register_model_loader_for_type::<TypedModel, _>(TestLoader::new("typed.Model"));

		let loader = registry.get_loader_for_type::<TypedModel>().unwrap();
		assert_eq!(loader.model_id(), "typed.Model");
	}
}
