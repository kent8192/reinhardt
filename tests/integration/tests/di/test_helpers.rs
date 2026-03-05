//! Test helpers for DI integration tests
//!
//! This module provides utilities for testing Injectable types that are not
//! registered in the global registry. It wraps Injectable::inject() with
//! circular dependency detection.

use reinhardt_di::{
	DiError, DiResult, Injectable, InjectionContext, begin_resolution, register_type_name,
	with_cycle_detection_scope,
};
use std::any::TypeId;
use std::sync::Arc;

/// Resolve a type using Injectable::inject() with circular dependency detection
///
/// This helper function bypasses the global registry requirement and directly
/// calls Injectable::inject(), while still providing circular dependency detection.
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_di::{Injectable, InjectionContext, DiResult, SingletonScope};
/// # use std::sync::Arc;
/// # async fn example() -> DiResult<()> {
/// struct MyService;
///
/// #[async_trait::async_trait]
/// impl Injectable for MyService {
///     async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(MyService)
///     }
/// }
///
/// # let singleton_scope = Arc::new(SingletonScope::new());
/// let ctx = InjectionContext::builder(singleton_scope).build();
/// let service = resolve_injectable::<MyService>(&ctx).await?;
/// # Ok(())
/// # }
/// ```
pub(crate) async fn resolve_injectable<T>(ctx: &InjectionContext) -> DiResult<Arc<T>>
where
	T: Injectable + Send + Sync + 'static,
{
	with_cycle_detection_scope(async {
		let type_id = TypeId::of::<T>();
		let type_name = std::any::type_name::<T>();

		// Register type name for better error messages
		register_type_name::<T>(type_name);

		// Begin circular dependency detection
		let _guard = begin_resolution(type_id, type_name)
			.map_err(|e| DiError::CircularDependency(e.to_string()))?;

		// Call Injectable::inject()
		let instance = T::inject(ctx).await?;

		Ok(Arc::new(instance))
	})
	.await
}
