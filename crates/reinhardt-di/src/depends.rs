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
//! ```
//! use reinhardt_di::{Depends, DiResult, Injectable, InjectionContext, SingletonScope};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! #[derive(Clone, Default)]
//! struct Config {
//!     database_url: String,
//! }
//!
//! #[async_trait]
//! impl Injectable for Config {
//!     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
//!         Ok(Self::default())
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let singleton = Arc::new(SingletonScope::new());
//! let ctx = InjectionContext::builder(singleton).build();
//!
//! // Basic usage - with caching (default)
//! let config = Depends::<Config>::builder().resolve(&ctx).await?;
//!
//! // Without caching - creates new instance every time
//! let config = Depends::<Config>::builder_no_cache().resolve(&ctx).await?;
//! # Ok(())
//! # }
//! ```

use crate::injected::DependencyScope;
use crate::{
	DiError, DiResult, Injectable, begin_resolution, context::InjectionContext,
	injected::InjectionMetadata, with_cycle_detection_scope,
};
use std::any::TypeId;
use std::ops::Deref;
use std::sync::Arc;

/// Dependency injection wrapper similar to FastAPI's Depends.
///
/// Provides automatic dependency resolution with optional caching
/// and circular dependency detection.
#[derive(Debug)]
pub struct Depends<T: Injectable> {
	inner: Arc<T>,
	metadata: InjectionMetadata,
}

impl<T: Injectable> Depends<T>
where
	T: Clone,
{
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
	/// This method will:
	/// 1. Detect circular dependencies
	/// 2. Check cache if `use_cache` is true
	/// 3. Call `T::inject(ctx)` if not cached or cache is disabled
	/// 4. Store in cache if `use_cache` is true
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
		with_cycle_detection_scope(async {
			let value = if use_cache {
				// Check request cache first
				if let Some(cached) = ctx.get_request::<T>() {
					Arc::try_unwrap(cached).unwrap_or_else(|arc| (*arc).clone())
				} else {
					// Begin circular dependency detection
					let type_id = TypeId::of::<T>();
					let type_name = std::any::type_name::<T>();
					crate::register_type_name::<T>(type_name);
					let _guard = begin_resolution(type_id, type_name)
						.map_err(|e| DiError::CircularDependency(e.to_string()))?;

					let v = T::inject(ctx).await?;
					ctx.set_request(v.clone());
					v
				}
			} else {
				// Begin circular dependency detection (even for uncached)
				let type_id = TypeId::of::<T>();
				let type_name = std::any::type_name::<T>();
				crate::register_type_name::<T>(type_name);
				let _guard = begin_resolution(type_id, type_name)
					.map_err(|e| DiError::CircularDependency(e.to_string()))?;

				// Skip cache
				T::inject_uncached(ctx).await?
			};

			Ok(Self {
				inner: Arc::new(value),
				metadata: InjectionMetadata {
					scope: DependencyScope::Request,
					cached: use_cache,
				},
			})
		})
		.await
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
}

/// Builder for Depends to support FastAPI-style API.
pub struct DependsBuilder<T: Injectable> {
	use_cache: bool,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: Injectable> DependsBuilder<T>
where
	T: Clone,
{
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

impl<T: Injectable> Deref for Depends<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T: Injectable> Clone for Depends<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
			metadata: self.metadata,
		}
	}
}

impl<T: Injectable> AsRef<T> for Depends<T> {
	fn as_ref(&self) -> &T {
		&self.inner
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::SingletonScope;
	use rstest::rstest;

	#[derive(Clone, Default, Debug)]
	struct TestConfig {
		value: String,
	}

	#[async_trait::async_trait]
	impl Injectable for TestConfig {
		async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
			Ok(TestConfig {
				value: "test".to_string(),
			})
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
	async fn test_depends_circular_dependency_detection() {
		// Arrange
		#[derive(Clone, Default, Debug)]
		struct CircularA;

		#[async_trait::async_trait]
		impl Injectable for CircularA {
			async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
				// Attempt to resolve self, creating a circular dependency
				let _self_ref = Depends::<CircularA>::resolve(ctx, true).await?;
				Ok(CircularA)
			}
		}

		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Act
		let result = Depends::<CircularA>::resolve(&ctx, true).await;

		// Assert
		assert!(result.is_err());
		if let Err(DiError::CircularDependency(msg)) = &result {
			assert!(msg.contains("CircularA"));
		} else {
			panic!("Expected CircularDependency error, got: {:?}", result);
		}
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
}
