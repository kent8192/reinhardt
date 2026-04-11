//! Depends wrapper for dependency injection
//!
//! FastAPI-inspired dependency injection wrapper that provides:
//! - Automatic dependency resolution
//! - Circular dependency detection
//! - Caching control via `use_cache` parameter
//! - Type-safe dependency injection with metadata
//!
//! ## Examples
//!
//! ```rust,no_run
//! use reinhardt_di::{Depends, DiResult, InjectionContext, SingletonScope, global_registry, DependencyScope};
//! use std::sync::Arc;
//!
//! #[derive(Default)]
//! struct Config {
//!     database_url: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Register Config in the global registry (normally done via #[injectable] or #[injectable_factory])
//! let registry = global_registry();
//! registry.register_async::<Config, _, _>(DependencyScope::Request, |_ctx| async {
//!     Ok(Config::default())
//! });
//!
//! let singleton = Arc::new(SingletonScope::new());
//! let ctx = InjectionContext::builder(singleton).build();
//!
//! // Basic usage - with caching (default)
//! let config = Depends::<Config>::builder().resolve(&ctx).await?;
//!
//! // Without caching flag - caching behavior is determined by the registered
//! // DependencyScope (Singleton/Request/Transient), not by this flag.
//! let config = Depends::<Config>::builder_no_cache().resolve(&ctx).await?;
//! # Ok(())
//! # }
//! ```

use crate::injected::DependencyScope;
use crate::{DiResult, context::InjectionContext, injected::InjectionMetadata};
use std::ops::Deref;
use std::sync::Arc;

/// Dependency injection wrapper similar to FastAPI's Depends.
///
/// Provides automatic dependency resolution with optional caching
/// and circular dependency detection.
///
/// `T` does not need to implement `Injectable` — resolution goes through
/// the global dependency registry, so any type registered via
/// `#[injectable_factory]` or `#[injectable]` can be used.
#[derive(Debug)]
pub struct Depends<T: Send + Sync + 'static> {
	inner: Arc<T>,
	metadata: InjectionMetadata,
}

impl<T: Send + Sync + 'static> Depends<T> {
	/// Create a new DependsBuilder with caching enabled (default behavior).
	///
	/// Similar to FastAPI's `Depends(dependency, use_cache=True)`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable, InjectionContext, DiResult};
	/// # use async_trait::async_trait;
	///
	/// #[derive(Clone, Default)]
	/// struct Config {
	///     value: String,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	///
	/// let builder = Depends::<Config>::builder();
	/// ```
	pub fn builder() -> DependsBuilder<T> {
		DependsBuilder {
			use_cache: true,
			_phantom: std::marker::PhantomData,
		}
	}
	/// Create a new DependsBuilder with caching disabled.
	///
	/// Similar to FastAPI's `Depends(dependency, use_cache=False)`.
	/// Each call will create a new instance.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable, InjectionContext, DiResult};
	/// # use async_trait::async_trait;
	///
	/// #[derive(Clone, Default)]
	/// struct RequestData {
	///     id: u32,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for RequestData {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(RequestData::default())
	/// #     }
	/// # }
	///
	/// let builder = Depends::<RequestData>::builder_no_cache();
	/// ```
	pub fn builder_no_cache() -> DependsBuilder<T> {
		DependsBuilder {
			use_cache: false,
			_phantom: std::marker::PhantomData,
		}
	}
	/// Resolve the dependency from the injection context.
	///
	/// This method delegates to `ctx.resolve::<T>()` which:
	/// 1. Detects circular dependencies
	/// 2. Checks scope caches (singleton/request)
	/// 3. Calls the registered factory if not cached
	///
	/// Caching behavior is determined by the registered `DependencyScope`,
	/// not by the `use_cache` parameter.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, InjectionContext, SingletonScope, Injectable, DiResult};
	/// # use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// #[derive(Clone, Default)]
	/// struct Config {
	///     value: String,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	///
	/// # async fn example() -> DiResult<()> {
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// let result = Depends::<Config>::resolve(&ctx, true).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn resolve(ctx: &InjectionContext, use_cache: bool) -> DiResult<Self> {
		// Resolve via the global dependency registry.
		// This does not require T: Injectable — any type registered via
		// #[injectable_factory] or #[injectable] can be resolved.
		// Cycle detection and caching are handled by ctx.resolve() based on
		// the registered DependencyScope (Singleton/Request/Transient).
		// The `use_cache` parameter is retained for API compatibility but
		// does not affect resolution — use Transient scope for uncached behavior.
		let arc = ctx.resolve::<T>().await?;

		Ok(Self {
			inner: arc,
			metadata: InjectionMetadata {
				scope: DependencyScope::Request,
				cached: use_cache,
			},
		})
	}
	/// Create a Depends from an existing value (for testing).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable, InjectionContext, DiResult};
	/// # use async_trait::async_trait;
	///
	/// #[derive(Clone, Default)]
	/// struct Config {
	///     value: String,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	///
	/// let config = Config { value: "test".to_string() };
	/// let depends = Depends::from_value(config);
	/// assert_eq!(depends.value, "test");
	/// ```
	pub fn from_value(value: T) -> Self {
		Self {
			inner: Arc::new(value),
			metadata: InjectionMetadata {
				scope: DependencyScope::Request,
				cached: false,
			},
		}
	}

	/// Get Arc reference
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable};
	/// use std::sync::Arc;
	///
	/// # #[derive(Clone, Default)]
	/// # struct Config;
	/// #
	/// # #[async_trait::async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	/// #
	/// let depends = Depends::from_value(Config::default());
	/// let arc: &Arc<Config> = depends.as_arc();
	/// ```
	pub fn as_arc(&self) -> &Arc<T> {
		&self.inner
	}

	/// Get injection metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable};
	///
	/// # #[derive(Clone, Default)]
	/// # struct Config;
	/// #
	/// # #[async_trait::async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	/// #
	/// let depends = Depends::from_value(Config::default());
	/// let metadata = depends.metadata();
	/// assert!(!metadata.cached);
	/// ```
	pub fn metadata(&self) -> &InjectionMetadata {
		&self.metadata
	}

	/// Attempt to unwrap the inner `Arc`, returning `T` if this is the only
	/// strong reference. Returns `Err(Self)` if other references exist.
	///
	/// This mirrors [`Arc::try_unwrap`] semantics. Unlike
	/// [`into_inner`](Depends::into_inner), this method does **not** require
	/// `T: Clone`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::Depends;
	///
	/// // Success: single owner
	/// let depends = Depends::from_value(42u32);
	/// let value = depends.try_unwrap().unwrap();
	/// assert_eq!(value, 42);
	///
	/// // Failure: multiple owners
	/// let depends = Depends::from_value(42u32);
	/// let _clone = depends.clone();
	/// let err = depends.try_unwrap().unwrap_err();
	/// assert_eq!(*err, 42); // still accessible via Deref
	/// ```
	pub fn try_unwrap(self) -> Result<T, Self> {
		match Arc::try_unwrap(self.inner) {
			Ok(val) => Ok(val),
			Err(arc) => Err(Self {
				inner: arc,
				metadata: self.metadata,
			}),
		}
	}
}

impl<T: Clone + Send + Sync + 'static> Depends<T> {
	/// Extract the inner value from the Depends wrapper.
	///
	/// This method tries to unwrap the Arc. If the Arc has multiple strong references,
	/// it clones the inner value instead.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, Injectable, InjectionContext, DiResult};
	/// # use async_trait::async_trait;
	///
	/// #[derive(Clone, Default)]
	/// struct Config {
	///     value: String,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	///
	/// let config = Config { value: "test".to_string() };
	/// let depends = Depends::from_value(config);
	/// let inner = depends.into_inner();
	/// assert_eq!(inner.value, "test");
	/// ```
	pub fn into_inner(self) -> T {
		Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone())
	}
}

/// Builder for Depends to support FastAPI-style API.
pub struct DependsBuilder<T: Send + Sync + 'static> {
	use_cache: bool,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> DependsBuilder<T> {
	/// Resolve the dependency.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Depends, InjectionContext, SingletonScope, Injectable, DiResult};
	/// # use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// #[derive(Clone, Default)]
	/// struct Config {
	///     value: String,
	/// }
	///
	/// # #[async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	///
	/// # async fn example() -> DiResult<()> {
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// let builder = Depends::<Config>::builder();
	/// let result = builder.resolve(&ctx).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn resolve(self, ctx: &InjectionContext) -> DiResult<Depends<T>> {
		Depends::resolve(ctx, self.use_cache).await
	}
}

impl<T: Send + Sync + 'static> Deref for Depends<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T: Send + Sync + 'static> Clone for Depends<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
			metadata: self.metadata,
		}
	}
}

impl<T: Send + Sync + 'static> AsRef<T> for Depends<T> {
	fn as_ref(&self) -> &T {
		&self.inner
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{DependencyScope as RegistryScope, SingletonScope, global_registry};
	use rstest::rstest;

	#[derive(Clone, Default, Debug)]
	struct TestConfig {
		value: String,
	}

	/// Register TestConfig in the global registry for resolution tests.
	fn register_test_config() {
		let registry = global_registry();
		if !registry.is_registered::<TestConfig>() {
			registry.register_async::<TestConfig, _, _>(RegistryScope::Request, |_ctx| async {
				Ok(TestConfig {
					value: "test".to_string(),
				})
			});
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_from_value() {
		// Arrange
		let config = TestConfig {
			value: "custom".to_string(),
		};

		// Act
		let depends = Depends::from_value(config);

		// Assert
		assert_eq!(depends.value, "custom");
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_into_inner() {
		// Arrange
		let config = TestConfig {
			value: "test".to_string(),
		};
		let depends = Depends::from_value(config);

		// Act
		let extracted = depends.into_inner();

		// Assert
		assert_eq!(extracted.value, "test");
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_clone() {
		// Arrange
		let config = TestConfig {
			value: "test".to_string(),
		};
		let depends1 = Depends::from_value(config);

		// Act
		let depends2 = depends1.clone();

		// Assert
		assert_eq!(depends1.value, "test");
		assert_eq!(depends2.value, "test");
		assert!(Arc::ptr_eq(depends1.as_arc(), depends2.as_arc()));
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_deref() {
		// Arrange
		let config = TestConfig {
			value: "test".to_string(),
		};
		let depends = Depends::from_value(config);

		// Act & Assert
		assert_eq!(depends.value, "test");
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_metadata() {
		// Arrange
		let config = TestConfig {
			value: "test".to_string(),
		};

		// Act
		let depends = Depends::from_value(config);

		// Assert
		let metadata = depends.metadata();
		assert_eq!(metadata.scope, DependencyScope::Request);
		assert!(!metadata.cached);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_as_arc() {
		// Arrange
		let config = TestConfig {
			value: "arc_test".to_string(),
		};
		let depends = Depends::from_value(config);

		// Act
		let arc = depends.as_arc();

		// Assert
		assert_eq!(arc.value, "arc_test");
		assert_eq!(Arc::strong_count(arc), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_as_ref() {
		// Arrange
		let config = TestConfig {
			value: "ref_test".to_string(),
		};
		let depends = Depends::from_value(config);

		// Act
		let reference: &TestConfig = depends.as_ref();

		// Assert
		assert_eq!(reference.value, "ref_test");
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_resolve_with_context() {
		// Arrange
		register_test_config();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let depends = Depends::<TestConfig>::resolve(&ctx, true).await.unwrap();

		// Assert
		assert_eq!(depends.value, "test");
		assert!(depends.metadata().cached);
		assert_eq!(depends.metadata().scope, DependencyScope::Request);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_resolve_uncached() {
		// Arrange
		register_test_config();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let depends = Depends::<TestConfig>::resolve(&ctx, false).await.unwrap();

		// Assert
		assert_eq!(depends.value, "test");
		assert!(!depends.metadata().cached);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_builder_resolve() {
		// Arrange
		register_test_config();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let depends = Depends::<TestConfig>::builder()
			.resolve(&ctx)
			.await
			.unwrap();

		// Assert
		assert_eq!(depends.value, "test");
		assert!(depends.metadata().cached);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_builder_no_cache_resolve() {
		// Arrange
		register_test_config();
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let depends = Depends::<TestConfig>::builder_no_cache()
			.resolve(&ctx)
			.await
			.unwrap();

		// Assert
		assert_eq!(depends.value, "test");
		assert!(!depends.metadata().cached);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_resolve_unregistered_type_returns_error() {
		// Arrange
		#[derive(Debug)]
		struct UnregisteredType;

		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let result = Depends::<UnregisteredType>::resolve(&ctx, true).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_metadata_preserved_on_clone() {
		// Arrange
		let config = TestConfig {
			value: "metadata".to_string(),
		};
		let depends1 = Depends::from_value(config);

		// Act
		let depends2 = depends1.clone();

		// Assert
		assert_eq!(depends1.metadata().scope, depends2.metadata().scope);
		assert_eq!(depends1.metadata().cached, depends2.metadata().cached);
	}

	/// Non-Clone type can be used with `Depends<T>` via `from_value()`,
	/// `clone()` (Arc-based), `Deref`, and `AsRef`.
	/// `into_inner()` is NOT available for non-Clone types.
	#[rstest]
	#[tokio::test]
	async fn test_depends_non_clone_type() {
		// Arrange
		#[derive(Debug)]
		struct NonCloneService {
			id: u32,
		}

		let service = NonCloneService { id: 42 };

		// Act
		let depends = Depends::from_value(service);
		let cloned_depends = depends.clone();

		// Assert
		assert_eq!(depends.id, 42);
		assert_eq!(cloned_depends.id, 42);
		assert!(Arc::ptr_eq(depends.as_arc(), cloned_depends.as_arc()));
		assert_eq!(depends.as_ref().id, 42);
	}

	/// Non-Clone type can be resolved via the global registry.
	#[rstest]
	#[tokio::test]
	async fn test_depends_non_clone_type_resolve() {
		// Arrange
		#[derive(Debug)]
		struct NonCloneRouter {
			prefix: String,
		}

		let registry = global_registry();
		if !registry.is_registered::<NonCloneRouter>() {
			registry.register_async::<NonCloneRouter, _, _>(RegistryScope::Request, |_ctx| async {
				Ok(NonCloneRouter {
					prefix: "/api".to_string(),
				})
			});
		}

		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let depends = Depends::<NonCloneRouter>::resolve(&ctx, true)
			.await
			.unwrap();

		// Assert
		assert_eq!(depends.prefix, "/api");
		assert!(depends.metadata().cached);
	}

	/// `try_unwrap()` succeeds when there is only one strong reference.
	#[rstest]
	#[tokio::test]
	async fn test_depends_try_unwrap_success() {
		// Arrange
		let config = TestConfig {
			value: "owned".to_string(),
		};
		let depends = Depends::from_value(config);

		// Act
		let result = depends.try_unwrap();

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap().value, "owned");
	}

	/// `try_unwrap()` returns `Err(Self)` when multiple references exist.
	#[rstest]
	#[tokio::test]
	async fn test_depends_try_unwrap_err_multiple_refs() {
		// Arrange
		let config = TestConfig {
			value: "shared".to_string(),
		};
		let depends = Depends::from_value(config);
		let _clone = depends.clone();

		// Act
		let result = depends.try_unwrap();

		// Assert
		let returned = result.unwrap_err();
		assert_eq!(returned.value, "shared");
		assert_eq!(returned.metadata().scope, DependencyScope::Request);
	}

	/// `try_unwrap()` works with non-Clone types (the primary use case).
	#[rstest]
	#[tokio::test]
	async fn test_depends_try_unwrap_non_clone_type() {
		// Arrange
		#[derive(Debug, PartialEq)]
		struct NonCloneRouter {
			prefix: String,
		}

		let router = NonCloneRouter {
			prefix: "/api".to_string(),
		};
		let depends = Depends::from_value(router);

		// Act
		let result = depends.try_unwrap();

		// Assert
		assert_eq!(
			result.unwrap(),
			NonCloneRouter {
				prefix: "/api".to_string()
			}
		);
	}
}
