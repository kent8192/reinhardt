//! Dependency injection support for ViewSets
//!
//! This module provides the `InjectableViewSet` extension trait that enables
//! dependency injection in ViewSet methods.
//!
//! # Usage
//!
//! ```ignore
//! use reinhardt_views::viewsets::{InjectableViewSet, ModelViewSet, ViewSet};
//! use reinhardt_di::Injectable;
//! use std::sync::Arc;
//!
//! impl MyViewSet {
//!     async fn handle_list(&self, request: Request) -> Result<Response> {
//!         // Resolve dependencies from the request's DI context
//!         let db: Arc<DatabaseConnection> = self.resolve(&request).await?;
//!         let cache: CacheService = self.resolve_uncached(&request).await?;
//!
//!         // Use the dependencies
//!         let items = db.fetch_all().await?;
//!         Ok(Response::ok().with_json(&items)?)
//!     }
//! }
//! ```

use crate::ViewSet;
use async_trait::async_trait;
use reinhardt_di::{Depends, Injectable, InjectionContext};
use reinhardt_http::{Request, Result};
use std::sync::Arc;

/// Extension trait for ViewSets that enables dependency injection
///
/// This trait is automatically implemented for all types that implement `ViewSet`.
/// It provides helper methods to resolve dependencies from the request's DI context.
///
/// # Examples
///
/// ```ignore
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_views::viewsets::{InjectableViewSet, ModelViewSet, ViewSet};
/// use std::sync::Arc;
///
/// struct UserViewSet {
///     basename: String,
/// }
///
/// impl UserViewSet {
///     async fn handle_list(&self, request: Request) -> Result<Response> {
///         // Resolve with caching (default)
///         let db: Arc<DatabaseConnection> = self.resolve(&request).await?;
///
///         // Resolve without caching
///         let fresh_config: Config = self.resolve_uncached(&request).await?;
///
///         // Use dependencies...
///         Ok(Response::ok())
///     }
/// }
/// # }
/// ```
#[async_trait]
pub trait InjectableViewSet: ViewSet {
	/// Resolve a dependency from the request's DI context with caching
	///
	/// This method extracts the `InjectionContext` from the request and resolves
	/// the requested dependency type. The resolved dependency is cached for the
	/// duration of the request.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The DI context is not set on the request (router misconfiguration)
	/// - The dependency cannot be resolved (not registered, circular dependency, etc.)
	///
	/// # Examples
	///
	/// ```ignore
	/// let db: Arc<DatabaseConnection> = self.resolve(&request).await?;
	/// ```
	async fn resolve<T>(&self, request: &Request) -> Result<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let di_ctx = request
			.get_di_context::<Arc<InjectionContext>>()
			.ok_or_else(|| {
				reinhardt_core::exception::Error::Internal(
					"DI context not set. Ensure the router is configured with .with_di_context()"
						.to_string(),
				)
			})?;

		let injected = Depends::<T>::resolve(&di_ctx, true).await.map_err(|e| {
			reinhardt_core::exception::Error::Internal(format!(
				"Dependency injection failed for {}: {:?}",
				std::any::type_name::<T>(),
				e
			))
		})?;

		Ok(injected.into_inner())
	}

	/// Resolve a dependency from the request's DI context without caching
	///
	/// This method is similar to `resolve()` but creates a fresh instance
	/// of the dependency each time, bypassing the cache.
	///
	/// Use this when you need:
	/// - A fresh instance that won't share state with other resolutions
	/// - To avoid caching for dependencies with mutable state
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The DI context is not set on the request (router misconfiguration)
	/// - The dependency cannot be resolved (not registered, circular dependency, etc.)
	///
	/// # Examples
	///
	/// ```ignore
	/// let fresh_service: MyService = self.resolve_uncached(&request).await?;
	/// ```
	async fn resolve_uncached<T>(&self, request: &Request) -> Result<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let di_ctx = request
			.get_di_context::<Arc<InjectionContext>>()
			.ok_or_else(|| {
				reinhardt_core::exception::Error::Internal(
					"DI context not set. Ensure the router is configured with .with_di_context()"
						.to_string(),
				)
			})?;

		let injected = Depends::<T>::resolve(&di_ctx, false).await.map_err(|e| {
			reinhardt_core::exception::Error::Internal(format!(
				"Dependency injection failed for {}: {:?}",
				std::any::type_name::<T>(),
				e
			))
		})?;

		Ok(injected.into_inner())
	}

	/// Try to resolve a dependency, returning None if DI context is not available
	///
	/// This is useful for optional dependencies or when you want to gracefully
	/// handle the case where DI is not configured.
	///
	/// # Examples
	///
	/// ```ignore
	/// if let Some(cache) = self.try_resolve::<CacheService>(&request).await {
	///     // Use cache
	/// } else {
	///     // Fallback without cache
	/// }
	/// ```
	async fn try_resolve<T>(&self, request: &Request) -> Option<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let di_ctx = request.get_di_context::<Arc<InjectionContext>>()?;

		Depends::<T>::resolve(&di_ctx, true)
			.await
			.ok()
			.map(|injected| injected.into_inner())
	}
}

// Blanket implementation for all ViewSet types
impl<V: ViewSet> InjectableViewSet for V {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::viewsets::GenericViewSet;

	// Basic compilation test - InjectableViewSet is automatically implemented
	#[test]
	fn test_injectable_viewset_trait_is_implemented() {
		fn assert_injectable<T: InjectableViewSet>() {}

		// GenericViewSet should implement InjectableViewSet
		assert_injectable::<GenericViewSet<()>>();
	}
}
