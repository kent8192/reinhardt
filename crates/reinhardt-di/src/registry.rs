//! Global dependency registry for FastAPI-style dependency injection
//!
//! This module provides a global registry that stores factory functions for creating
//! dependencies. It uses the `inventory` crate to collect registrations at compile time
//! and build a runtime registry that can be queried by type.

use crate::{DiResult, Injectable, InjectionContext};
use async_trait::async_trait;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::future::Future;
use std::marker::PhantomData;
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
	Fut: Future<Output = DiResult<T>> + Send,
	T: Any + Send + Sync + 'static,
{
	factory: F,
	_phantom: std::marker::PhantomData<fn() -> (Fut, T)>,
}

impl<F, Fut, T> AsyncFactory<F, Fut, T>
where
	F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync,
	Fut: Future<Output = DiResult<T>> + Send,
	T: Any + Send + Sync + 'static,
{
	/// Creates a new async factory from the given closure.
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
	Fut: Future<Output = DiResult<T>> + Send + 'static,
	T: Any + Send + Sync + 'static,
{
	async fn create(&self, ctx: &InjectionContext) -> DiResult<Arc<dyn Any + Send + Sync>> {
		let ctx_arc = Arc::new(ctx.clone());
		let instance = (self.factory)(ctx_arc).await?;
		Ok(Arc::new(instance))
	}
}

/// Factory that creates instances via the `Injectable` trait.
///
/// Bypasses `AsyncFactory`'s `Fut: Sync` bound by implementing `FactoryTrait`
/// directly. This is necessary because `Injectable::inject` uses `async_trait`,
/// which returns `Pin<Box<dyn Future + Send>>` (not `Sync`).
pub struct InjectableFactory<T>(PhantomData<T>);

impl<T> Default for InjectableFactory<T> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<T> InjectableFactory<T> {
	/// Create a new `InjectableFactory`.
	pub fn new() -> Self {
		Self::default()
	}
}

#[async_trait]
impl<T: Injectable + Any + Send + Sync + 'static> FactoryTrait for InjectableFactory<T> {
	async fn create(&self, ctx: &InjectionContext) -> DiResult<Arc<dyn Any + Send + Sync>> {
		// Set task-local resolve context for get_di_context() access.
		// Since we only have &InjectionContext, clone into Arc (same pattern as AsyncFactory).
		let ctx_arc = Arc::new(ctx.clone());
		let resolve_ctx = crate::resolve_context::ResolveContext {
			root: crate::resolve_context::RESOLVE_CTX
				.try_with(|outer| Arc::clone(&outer.root))
				.unwrap_or_else(|_| Arc::clone(&ctx_arc)),
			current: Arc::clone(&ctx_arc),
		};

		let value = crate::resolve_context::RESOLVE_CTX
			.scope(resolve_ctx, T::inject(ctx))
			.await?;
		Ok(Arc::new(value))
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
	/// Maps type ID to its fully-qualified type name from `std::any::type_name`.
	/// Used for framework type detection (pseudo orphan rule).
	qualified_type_names: DashMap<TypeId, &'static str>,
}

impl DependencyRegistry {
	/// Create a new empty registry
	pub fn new() -> Self {
		Self {
			factories: DashMap::new(),
			scopes: DashMap::new(),
			dependencies: DashMap::new(),
			type_names: DashMap::new(),
			qualified_type_names: DashMap::new(),
		}
	}

	/// Register a factory for a type.
	///
	/// # Panics
	///
	/// Panics if a factory for the same `TypeId` is already registered.
	/// This prevents silent overwrites that lead to non-deterministic behavior
	/// when multiple `#[injectable_factory]` or `#[injectable]` macros produce
	/// the same return type. See [#3457].
	///
	/// To check before registering (e.g. in tests), use
	/// [`is_registered`](Self::is_registered).
	///
	/// [#3457]: https://github.com/kent8192/reinhardt-web/issues/3457
	pub fn register<T: Any + Send + Sync + 'static>(
		&self,
		scope: DependencyScope,
		factory: impl FactoryTrait + 'static,
	) {
		let type_id = TypeId::of::<T>();
		let type_name = std::any::type_name::<T>();
		// Check for duplicates before inserting so that no state is mutated on the
		// error path. This avoids leaving the registry inconsistent if the panic is
		// caught (e.g. factories pointing to the new registration while scopes still
		// reflects the old one).
		if self.factories.contains_key(&type_id) {
			let short = type_name.rsplit("::").next().unwrap_or(type_name);
			panic!(
				"Duplicate DependencyRegistry registration for type `{type_name}`.\n\
\n\
Hint: reinhardt DI uses TypeId as the sole registry key. Two factories\n\
returning the same type will conflict regardless of function name or scope.\n\
Use a distinct newtype (e.g., `struct Primary{short}({short})`) for each."
			);
		}
		self.factories.insert(type_id, Box::new(factory));
		self.scopes.insert(type_id, scope);
	}

	/// Register a simple async factory function
	pub fn register_async<T, F, Fut>(&self, scope: DependencyScope, factory: F)
	where
		T: Any + Send + Sync + 'static,
		F: Fn(Arc<InjectionContext>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = DiResult<T>> + Send + 'static,
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
	/// Not intended for direct use; exposed for macro-generated code.
	#[doc(hidden)]
	pub fn register_dependencies(&self, type_id: TypeId, deps: impl AsRef<[TypeId]>) {
		self.dependencies.insert(type_id, deps.as_ref().to_vec());
	}

	/// Register a type name for debugging
	///
	/// This is typically called automatically by the registration system.
	/// Not intended for direct use; exposed for macro-generated code.
	#[doc(hidden)]
	pub fn register_type_name(&self, type_id: TypeId, type_name: &'static str) {
		self.type_names.insert(type_id, type_name);
	}

	/// Check if a type is registered by its `TypeId`.
	pub(crate) fn is_registered_by_id(&self, type_id: TypeId) -> bool {
		self.factories.contains_key(&type_id)
	}

	/// Get the scope for a type by its `TypeId`.
	pub(crate) fn get_scope_by_id(&self, type_id: TypeId) -> Option<DependencyScope> {
		self.scopes.get(&type_id).map(|entry| *entry.value())
	}

	/// Get the type name for a `TypeId`.
	pub(crate) fn get_type_name(&self, type_id: TypeId) -> Option<&'static str> {
		self.type_names.get(&type_id).map(|entry| *entry.value())
	}

	/// Register the fully-qualified type name obtained from `std::any::type_name::<T>()`.
	///
	/// Used by the pseudo orphan rule to detect framework-managed types.
	#[doc(hidden)]
	pub fn register_qualified_type_name(&self, type_id: TypeId, qualified_name: &'static str) {
		self.qualified_type_names.insert(type_id, qualified_name);
	}

	/// Get the fully-qualified type name for a given `TypeId`.
	pub fn get_qualified_type_name(&self, type_id: &TypeId) -> Option<&'static str> {
		self.qualified_type_names.get(type_id).map(|r| *r.value())
	}

	/// Iterate over all qualified type name mappings without allocating a new map.
	pub fn iter_qualified_type_names(&self) -> impl Iterator<Item = (TypeId, &'static str)> + '_ {
		self.qualified_type_names
			.iter()
			.map(|entry| (*entry.key(), *entry.value()))
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
	/// The `TypeId` of the dependency being registered.
	pub type_id: TypeId,
	/// The human-readable name of the type.
	pub type_name: &'static str,
	/// The scope (request or singleton) for this dependency.
	pub scope: DependencyScope,
	/// Direct dependencies of this type.
	pub dependencies: &'static [TypeId],
	/// A function that registers this dependency's factory with the registry.
	pub register_fn: fn(&DependencyRegistry),
}

impl DependencyRegistration {
	/// Create a new registration entry
	pub const fn new<T: Send + Sync + 'static>(
		type_name: &'static str,
		scope: DependencyScope,
		register_fn: fn(&DependencyRegistry),
	) -> Self {
		Self {
			type_id: TypeId::of::<T>(),
			type_name,
			scope,
			dependencies: &[],
			register_fn,
		}
	}

	/// Create a new registration entry with explicit dependencies
	pub const fn new_with_deps<T: Send + Sync + 'static>(
		type_name: &'static str,
		scope: DependencyScope,
		dependencies: &'static [TypeId],
		register_fn: fn(&DependencyRegistry),
	) -> Self {
		Self {
			type_id: TypeId::of::<T>(),
			type_name,
			scope,
			dependencies,
			register_fn,
		}
	}
}

// Collect all dependency registrations at compile time
inventory::collect!(DependencyRegistration);

/// Const-constructible registration entry for `#[injectable]` structs with `#[scope]`.
///
/// Unlike `DependencyRegistration` which uses `Box<dyn Fn>` (non-const),
/// this struct stores a plain function pointer so it can be used in
/// `inventory::submit!` which requires const-evaluable expressions.
pub struct InjectableRegistration {
	/// A function that registers this type's factory with the registry.
	pub register_fn: fn(&DependencyRegistry),
}

impl InjectableRegistration {
	/// Create a new `InjectableRegistration` with a function pointer.
	pub const fn new(register_fn: fn(&DependencyRegistry)) -> Self {
		Self { register_fn }
	}
}

inventory::collect!(InjectableRegistration);

/// Initialize the registry with all collected registrations
fn initialize_registry(registry: &DependencyRegistry) {
	for registration in inventory::iter::<DependencyRegistration> {
		(registration.register_fn)(registry);
	}
	for registration in inventory::iter::<InjectableRegistration> {
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
	use rstest::*;

	#[derive(Clone)]
	struct TestService {
		value: i32,
	}

	#[rstest]
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

	#[rstest]
	#[tokio::test]
	async fn test_registry_not_registered() {
		let registry = DependencyRegistry::new();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		let result = registry.create::<TestService>(&ctx).await;
		assert!(result.is_err());
	}

	// Fixes #3457
	#[rstest]
	fn test_duplicate_registration_panics() {
		let registry = DependencyRegistry::new();

		registry.register_async::<TestService, _, _>(DependencyScope::Singleton, |_ctx| async {
			Ok(TestService { value: 1 })
		});

		// Act
		let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			registry.register_async::<TestService, _, _>(DependencyScope::Request, |_ctx| async {
				Ok(TestService { value: 2 })
			});
		}));

		// Assert
		let err = result.expect_err("expected panic on duplicate registration");
		let msg = err
			.downcast_ref::<String>()
			.map(|s| s.as_str())
			.or_else(|| err.downcast_ref::<&str>().copied())
			.expect("panic payload should be a string");
		assert!(
			msg.contains("Duplicate DependencyRegistry registration"),
			"missing duplicate prefix: {msg}"
		);
		assert!(
			msg.contains("TestService"),
			"missing type name in panic message: {msg}"
		);
		assert!(
			msg.contains("newtype"),
			"missing newtype hint in panic message: {msg}"
		);
	}

	// Fixes #3457 — is_registered guard prevents panic (test helper pattern)
	#[rstest]
	fn test_is_registered_guard_allows_skip() {
		let registry = DependencyRegistry::new();

		registry.register_async::<TestService, _, _>(DependencyScope::Singleton, |_ctx| async {
			Ok(TestService { value: 1 })
		});

		// Second registration guarded — no panic
		if !registry.is_registered::<TestService>() {
			registry.register_async::<TestService, _, _>(DependencyScope::Request, |_ctx| async {
				Ok(TestService { value: 2 })
			});
		}

		assert!(registry.is_registered::<TestService>());
	}
}
