//! Global dependency registry for FastAPI-style dependency injection
//!
//! This module provides a global registry that stores factory functions for creating
//! dependencies. It uses the `inventory` crate to collect registrations at compile time
//! and build a runtime registry that can be queried by type.

use crate::{DiResult, InjectionContext};
use async_trait::async_trait;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::future::Future;
use std::sync::{Arc, OnceLock};

/// Scope for dependency injection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DependencyScope {
	/// Single instance shared across the entire application
	#[default]
	Singleton,
	/// New instance per request, cached within the request
	Request,
	/// New instance every time, never cached
	Transient,
}

/// Factory trait for creating dependencies
///
/// Factories are async functions that can resolve dependencies from an InjectionContext
/// and return a type-erased `Arc<dyn Any>`.
#[async_trait]
pub trait FactoryTrait: Send + Sync {
	/// Create an instance of the dependency
	async fn create(&self, ctx: &InjectionContext) -> DiResult<Arc<dyn Any + Send + Sync>>;
}

/// Wrapper for async factory functions
pub struct AsyncFactory<F, Fut, T>
where
	F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync,
	Fut: Future<Output = DiResult<T>> + Send + Sync,
	T: Any + Send + Sync + 'static,
{
	factory: F,
	_phantom: std::marker::PhantomData<fn() -> (Fut, T)>,
}

impl<F, Fut, T> AsyncFactory<F, Fut, T>
where
	F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync,
	Fut: Future<Output = DiResult<T>> + Send + Sync,
	T: Any + Send + Sync + 'static,
{
	pub fn new(factory: F) -> Self {
		Self {
			factory,
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<F, Fut, T> FactoryTrait for AsyncFactory<F, Fut, T>
where
	F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync,
	Fut: Future<Output = DiResult<T>> + Send + Sync + 'static,
	T: Any + Send + Sync + 'static,
{
	async fn create(&self, ctx: &InjectionContext) -> DiResult<Arc<dyn Any + Send + Sync>> {
		let ctx_arc = Arc::new(ctx.clone());
		let instance = (self.factory)(ctx_arc).await?;
		Ok(Arc::new(instance))
	}
}

/// Type-erased factory function
type BoxedFactory = Box<dyn FactoryTrait>;

/// Global dependency registry
///
/// Stores factory functions for each type, along with their scope information.
/// Uses DashMap for thread-safe concurrent access without blocking.
pub struct DependencyRegistry {
	factories: DashMap<TypeId, BoxedFactory>,
	scopes: DashMap<TypeId, DependencyScope>,
	/// Maps type ID to its direct dependencies
	dependencies: DashMap<TypeId, Vec<TypeId>>,
	/// Maps type ID to its type name for debugging
	type_names: DashMap<TypeId, &'static str>,
}

impl DependencyRegistry {
	/// Create a new empty registry
	pub fn new() -> Self {
		Self {
			factories: DashMap::new(),
			scopes: DashMap::new(),
			dependencies: DashMap::new(),
			type_names: DashMap::new(),
		}
	}

	/// Register a factory for a type
	pub fn register<T: Any + Send + Sync + 'static>(
		&self,
		scope: DependencyScope,
		factory: impl FactoryTrait + 'static,
	) {
		let type_id = TypeId::of::<T>();
		self.factories.insert(type_id, Box::new(factory));
		self.scopes.insert(type_id, scope);
	}

	/// Register a simple async factory function
	pub fn register_async<T, F, Fut>(&self, scope: DependencyScope, factory: F)
	where
		T: Any + Send + Sync + 'static,
		F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = DiResult<T>> + Send + Sync + 'static,
	{
		self.register::<T>(scope, AsyncFactory::new(factory));
	}

	/// Get the scope for a type
	pub fn get_scope<T: Any + 'static>(&self) -> Option<DependencyScope> {
		let type_id = TypeId::of::<T>();
		self.scopes.get(&type_id).map(|entry| *entry.value())
	}

	/// Check if a type is registered
	pub fn is_registered<T: Any + 'static>(&self) -> bool {
		let type_id = TypeId::of::<T>();
		self.factories.contains_key(&type_id)
	}

	/// Get the number of registered dependencies
	pub fn len(&self) -> usize {
		self.factories.len()
	}

	/// Check if the registry is empty
	pub fn is_empty(&self) -> bool {
		self.factories.is_empty()
	}

	/// Create an instance using the registered factory
	pub async fn create<T: Any + Send + Sync + 'static>(
		&self,
		ctx: &InjectionContext,
	) -> DiResult<Arc<T>> {
		let type_id = TypeId::of::<T>();

		let factory = self.factories.get(&type_id).ok_or_else(|| {
			crate::DiError::DependencyNotRegistered {
				type_name: std::any::type_name::<T>().to_string(),
			}
		})?;

		let any_arc = factory.create(ctx).await?;

		any_arc
			.downcast::<T>()
			.map_err(|_| crate::DiError::Internal {
				message: format!(
					"Failed to downcast dependency: expected {}, got different type",
					std::any::type_name::<T>()
				),
			})
	}

	/// Get the direct dependencies of a type
	///
	/// Returns a vector of TypeIds representing the types that the given type directly depends on.
	pub fn get_dependencies(&self, type_id: TypeId) -> Vec<TypeId> {
		self.dependencies
			.get(&type_id)
			.map(|deps| deps.value().clone())
			.unwrap_or_default()
	}

	/// Get all dependencies in the registry
	///
	/// Returns a HashMap mapping each type to its direct dependencies.
	pub fn get_all_dependencies(&self) -> std::collections::HashMap<TypeId, Vec<TypeId>> {
		self.dependencies
			.iter()
			.map(|entry| (*entry.key(), entry.value().clone()))
			.collect()
	}

	/// Get all type names in the registry
	///
	/// Returns a HashMap mapping TypeIds to their human-readable type names.
	pub fn get_type_names(&self) -> std::collections::HashMap<TypeId, &'static str> {
		self.type_names
			.iter()
			.map(|entry| (*entry.key(), *entry.value()))
			.collect()
	}

	/// Register dependencies for a type
	///
	/// This is typically called automatically by the registration system.
	pub(crate) fn register_dependencies(&self, type_id: TypeId, deps: Vec<TypeId>) {
		self.dependencies.insert(type_id, deps);
	}

	/// Register a type name for debugging
	///
	/// This is typically called automatically by the registration system.
	pub(crate) fn register_type_name(&self, type_id: TypeId, type_name: &'static str) {
		self.type_names.insert(type_id, type_name);
	}
}

impl Default for DependencyRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Global singleton registry instance
static GLOBAL_REGISTRY: OnceLock<Arc<DependencyRegistry>> = OnceLock::new();

/// Get the global registry instance
pub fn global_registry() -> &'static Arc<DependencyRegistry> {
	GLOBAL_REGISTRY.get_or_init(|| {
		let registry = Arc::new(DependencyRegistry::new());
		initialize_registry(&registry);
		registry
	})
}

/// Resets the global dependency registry for test isolation.
///
/// This replaces the `GLOBAL_REGISTRY` `OnceLock` with a fresh instance so
/// that the next call to `global_registry()` will re-initialize it.
///
/// # Safety
///
/// This function replaces a static `OnceLock` value using `std::ptr::write`.
/// It is only safe to call from a single-threaded test context (e.g., with
/// `#[serial]`) where no other thread is concurrently reading the registry.
#[cfg(test)]
pub fn reset_global_registry() {
	// SAFETY: We replace the OnceLock in-place with a fresh instance.
	// This is safe only when called from a single-threaded test context
	// (enforced by #[serial]) where no concurrent readers exist.
	unsafe {
		let ptr = std::ptr::addr_of!(GLOBAL_REGISTRY) as *mut OnceLock<Arc<DependencyRegistry>>;
		std::ptr::write(ptr, OnceLock::new());
	}
}

/// Registration entry for inventory collection
pub struct DependencyRegistration {
	pub type_id: TypeId,
	pub type_name: &'static str,
	pub scope: DependencyScope,
	/// Direct dependencies of this type
	pub dependencies: Vec<TypeId>,
	pub register_fn: Box<dyn Fn(&DependencyRegistry) + Send + Sync>,
}

impl DependencyRegistration {
	/// Create a new registration entry
	pub fn new<T, F, Fut>(type_name: &'static str, scope: DependencyScope, factory: F) -> Self
	where
		T: Any + Send + Sync + 'static,
		F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync + 'static + Clone,
		Fut: Future<Output = DiResult<T>> + Send + Sync + 'static,
	{
		let type_id = TypeId::of::<T>();
		let register_fn = Box::new(move |registry: &DependencyRegistry| {
			let factory = factory.clone();
			registry.register_async::<T, _, _>(scope, factory);
			// Register type name for debugging
			registry.register_type_name(type_id, type_name);
		});

		Self {
			type_id,
			type_name,
			scope,
			dependencies: Vec::new(), // Dependencies will be populated by macros in the future
			register_fn,
		}
	}

	/// Create a new registration entry with explicit dependencies
	pub fn new_with_deps<T, F, Fut>(
		type_name: &'static str,
		scope: DependencyScope,
		dependencies: Vec<TypeId>,
		factory: F,
	) -> Self
	where
		T: Any + Send + Sync + 'static,
		F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync + 'static + Clone,
		Fut: Future<Output = DiResult<T>> + Send + Sync + 'static,
	{
		let type_id = TypeId::of::<T>();
		let deps_clone = dependencies.clone();
		let register_fn = Box::new(move |registry: &DependencyRegistry| {
			let factory = factory.clone();
			registry.register_async::<T, _, _>(scope, factory);
			// Register type name and dependencies
			registry.register_type_name(type_id, type_name);
			registry.register_dependencies(type_id, deps_clone.clone());
		});

		Self {
			type_id,
			type_name,
			scope,
			dependencies,
			register_fn,
		}
	}
}

// Collect all dependency registrations at compile time
inventory::collect!(DependencyRegistration);

/// Initialize the registry with all collected registrations
fn initialize_registry(registry: &DependencyRegistry) {
	for registration in inventory::iter::<DependencyRegistration> {
		(registration.register_fn)(registry);
	}
}

/// Helper macro for submitting registrations to inventory
///
/// This is used internally by the `#[injectable]` and `#[injectable_factory]` macros.
#[macro_export]
macro_rules! submit_registration {
	($registration:expr) => {
		$crate::inventory::submit! {
			$registration
		}
	};
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::scope::SingletonScope;

	#[derive(Clone)]
	struct TestService {
		value: i32,
	}

	#[tokio::test]
	async fn test_registry_basic() {
		let registry = DependencyRegistry::new();

		registry.register_async::<TestService, _, _>(DependencyScope::Singleton, |_ctx| async {
			Ok(TestService { value: 42 })
		});

		assert!(registry.is_registered::<TestService>());
		assert_eq!(
			registry.get_scope::<TestService>(),
			Some(DependencyScope::Singleton)
		);

		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		let service = registry.create::<TestService>(&ctx).await.unwrap();
		assert_eq!(service.value, 42);
	}

	#[tokio::test]
	async fn test_registry_not_registered() {
		let registry = DependencyRegistry::new();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		let result = registry.create::<TestService>(&ctx).await;
		assert!(result.is_err());
	}
}
