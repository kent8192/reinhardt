//! Factory trait definitions.
//!
//! This module defines the core traits for factory-based data generation.

use std::future::Future;

use crate::error::SeedingResult;

/// Core factory trait for model data generation.
///
/// Implement this trait to define how test data is generated for a model type.
///
/// # Example
///
/// ```ignore
/// struct UserFactory {
///     username: String,
///     email: String,
/// }
///
/// impl Factory for UserFactory {
///     type Model = User;
///
///     fn build(&self) -> User {
///         User::new(self.username.clone(), self.email.clone())
///     }
///
///     async fn create(&self) -> SeedingResult<User> {
///         let mut user = self.build();
///         user.save().await?;
///         Ok(user)
///     }
///
///     async fn create_batch(&self, count: usize) -> SeedingResult<Vec<User>> {
///         let mut users = Vec::with_capacity(count);
///         for _ in 0..count {
///             users.push(self.create().await?);
///         }
///         Ok(users)
///     }
/// }
/// ```
pub trait Factory: Send + Sync {
	/// The model type this factory creates.
	type Model: Send;

	/// Builds a new model instance without persisting it.
	///
	/// This method creates an in-memory instance using the factory's
	/// configured values (defaults, fakers, sequences).
	fn build(&self) -> Self::Model;

	/// Builds multiple model instances without persisting them.
	///
	/// # Arguments
	///
	/// * `count` - Number of instances to build
	fn build_batch(&self, count: usize) -> Vec<Self::Model> {
		(0..count).map(|_| self.build()).collect()
	}

	/// Creates and persists a new model instance.
	///
	/// This method builds an instance and saves it to the database.
	fn create(&self) -> impl Future<Output = SeedingResult<Self::Model>> + Send;

	/// Creates and persists multiple model instances.
	///
	/// # Arguments
	///
	/// * `count` - Number of instances to create
	fn create_batch(
		&self,
		count: usize,
	) -> impl Future<Output = SeedingResult<Vec<Self::Model>>> + Send;
}

/// Extended factory trait with customization support.
///
/// This trait provides methods for building and creating instances
/// with custom field overrides.
pub trait FactoryExt: Factory {
	/// Builds an instance with field overrides.
	///
	/// # Arguments
	///
	/// * `customizer` - Function that modifies the built instance
	///
	/// # Example
	///
	/// ```ignore
	/// let factory = UserFactory::new();
	/// let admin = factory.build_with(|user| {
	///     user.is_admin = true;
	/// });
	/// ```
	fn build_with<F>(&self, customizer: F) -> Self::Model
	where
		F: FnOnce(&mut Self::Model);

	/// Creates an instance with field overrides.
	///
	/// # Arguments
	///
	/// * `customizer` - Function that modifies the instance before persisting
	fn create_with<F>(
		&self,
		customizer: F,
	) -> impl Future<Output = SeedingResult<Self::Model>> + Send
	where
		F: FnOnce(&mut Self::Model) + Send;
}

/// Trait for factories that can create related models.
///
/// This is useful for setting up complex test scenarios with
/// interdependent models.
pub trait RelatedFactory<R>: Factory {
	/// Creates this model with a related model.
	///
	/// # Arguments
	///
	/// * `related` - The related model instance
	fn with_related(&self, related: R) -> Self::Model;

	/// Creates this model and its related model together.
	fn create_with_related(&self) -> impl Future<Output = SeedingResult<(Self::Model, R)>> + Send;
}

/// Trait for factories that can be lazy-evaluated.
///
/// Lazy factories only generate values when actually needed,
/// which can improve performance for complex factory setups.
pub trait LazyFactory: Factory {
	/// Returns a lazy evaluator for building instances.
	fn lazy(&self) -> LazyEvaluator<Self::Model>;
}

/// Lazy evaluator for deferred factory builds.
pub struct LazyEvaluator<M> {
	builder: Box<dyn FnOnce() -> M + Send>,
}

impl<M> LazyEvaluator<M> {
	/// Creates a new lazy evaluator.
	pub fn new<F>(builder: F) -> Self
	where
		F: FnOnce() -> M + Send + 'static,
	{
		Self {
			builder: Box::new(builder),
		}
	}

	/// Evaluates the lazy factory and returns the built instance.
	pub fn evaluate(self) -> M {
		(self.builder)()
	}
}

/// Trait for subfactories that can be embedded in other factories.
pub trait SubFactory: Send + Sync {
	/// The model type this subfactory creates.
	type Model: Send;

	/// Builds and returns the subfactory's model.
	fn build(&self) -> Self::Model;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// Test model
	#[derive(Debug, Clone, PartialEq)]
	struct TestModel {
		id: Option<i64>,
		name: String,
		active: bool,
	}

	impl TestModel {
		fn new(name: &str) -> Self {
			Self {
				id: None,
				name: name.to_string(),
				active: true,
			}
		}
	}

	// Test factory
	struct TestFactory {
		name: String,
		active: bool,
	}

	impl TestFactory {
		fn new() -> Self {
			Self {
				name: "test".to_string(),
				active: true,
			}
		}
	}

	impl Factory for TestFactory {
		type Model = TestModel;

		fn build(&self) -> TestModel {
			TestModel {
				id: None,
				name: self.name.clone(),
				active: self.active,
			}
		}

		async fn create(&self) -> SeedingResult<TestModel> {
			let mut model = self.build();
			model.id = Some(1); // Simulate database insert
			Ok(model)
		}

		async fn create_batch(&self, count: usize) -> SeedingResult<Vec<TestModel>> {
			let mut models = Vec::with_capacity(count);
			for i in 0..count {
				let mut model = self.build();
				model.id = Some(i as i64 + 1);
				models.push(model);
			}
			Ok(models)
		}
	}

	impl FactoryExt for TestFactory {
		fn build_with<F>(&self, customizer: F) -> TestModel
		where
			F: FnOnce(&mut TestModel),
		{
			let mut model = self.build();
			customizer(&mut model);
			model
		}

		async fn create_with<F>(&self, customizer: F) -> SeedingResult<TestModel>
		where
			F: FnOnce(&mut TestModel) + Send,
		{
			let mut model = self.build();
			customizer(&mut model);
			model.id = Some(1);
			Ok(model)
		}
	}

	#[rstest]
	fn test_factory_build() {
		let factory = TestFactory::new();
		let model = factory.build();
		assert_eq!(model.name, "test");
		assert!(model.active);
		assert!(model.id.is_none());
	}

	#[rstest]
	fn test_factory_build_batch() {
		let factory = TestFactory::new();
		let models = factory.build_batch(5);
		assert_eq!(models.len(), 5);
	}

	#[rstest]
	#[tokio::test]
	async fn test_factory_create() {
		let factory = TestFactory::new();
		let model = factory.create().await.unwrap();
		assert_eq!(model.id, Some(1));
	}

	#[rstest]
	#[tokio::test]
	async fn test_factory_create_batch() {
		let factory = TestFactory::new();
		let models = factory.create_batch(3).await.unwrap();
		assert_eq!(models.len(), 3);
		assert_eq!(models[0].id, Some(1));
		assert_eq!(models[1].id, Some(2));
		assert_eq!(models[2].id, Some(3));
	}

	#[rstest]
	fn test_factory_ext_build_with() {
		let factory = TestFactory::new();
		let model = factory.build_with(|m| {
			m.name = "custom".to_string();
			m.active = false;
		});
		assert_eq!(model.name, "custom");
		assert!(!model.active);
	}

	#[rstest]
	#[tokio::test]
	async fn test_factory_ext_create_with() {
		let factory = TestFactory::new();
		let model = factory
			.create_with(|m| {
				m.name = "custom".to_string();
			})
			.await
			.unwrap();
		assert_eq!(model.name, "custom");
		assert!(model.id.is_some());
	}

	#[rstest]
	fn test_lazy_evaluator() {
		let lazy = LazyEvaluator::new(|| TestModel::new("lazy"));
		let model = lazy.evaluate();
		assert_eq!(model.name, "lazy");
	}
}
