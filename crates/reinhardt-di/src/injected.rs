//! Injected wrapper for dependency injection
//!
//! FastAPI-inspired dependency injection wrapper that provides:
//! - Automatic dependency resolution
//! - Caching control via scope and cache flag
//! - Type-safe dependency injection with metadata
//!
//! # Examples
//!
//! ```
//! use reinhardt_di::{Injected, OptionalInjected, Injectable, InjectionContext};
//!
//! # #[derive(Clone, Default)]
//! # struct Database;
//! # #[derive(Clone, Default)]
//! # struct Cache;
//! #
//! # #[async_trait::async_trait]
//! # impl Injectable for Database {
//! #     async fn inject(ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
//! #         Ok(Database::default())
//! #     }
//! # }
//! #
//! # #[async_trait::async_trait]
//! # impl Injectable for Cache {
//! #     async fn inject(ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
//! #         Ok(Cache::default())
//! #     }
//! # }
//! #
//! async fn handler(
//!     db: Injected<Database>,
//!     optional_cache: OptionalInjected<Cache>,
//! ) -> String {
//!     // db is always available
//!     // optional_cache can be treated as Option<Injected<Cache>>
//!     "OK".to_string()
//! }
//! ```

use crate::{
	DiError, DiResult, Injectable, InjectionContext, begin_resolution, with_cycle_detection_scope,
};
use std::any::TypeId;
use std::ops::Deref;
use std::sync::Arc;

/// Injection metadata
///
/// Tracks the scope and caching status of an injected dependency.
#[derive(Debug, Clone, Copy)]
pub struct InjectionMetadata {
	/// Dependency scope (Request or Singleton)
	pub scope: DependencyScope,
	/// Whether caching was enabled during resolution
	pub cached: bool,
}

/// Dependency scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyScope {
	/// Request-scoped dependency (lifetime tied to request)
	Request,
	/// Singleton-scoped dependency (shared across requests)
	Singleton,
}

/// Injected dependency wrapper
///
/// Wraps an `Arc<T>` with injection metadata, providing:
/// - Shared ownership via `Arc`
/// - Metadata tracking (scope, cache status)
/// - Transparent access via `Deref`
///
/// # Examples
///
/// ```
/// use reinhardt_di::{Injected, InjectionContext, Injectable, SingletonScope};
/// use std::sync::Arc;
///
/// # #[derive(Clone, Default)]
/// # struct Config;
/// #
/// # #[async_trait::async_trait]
/// # impl Injectable for Config {
/// #     async fn inject(ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
/// #         Ok(Config::default())
/// #     }
/// # }
/// #
/// # async fn example() -> reinhardt_di::DiResult<()> {
/// let singleton_scope = Arc::new(SingletonScope::new());
/// let ctx = InjectionContext::builder(singleton_scope).build();
///
/// // Resolve with cache enabled (default)
/// let config1 = Injected::<Config>::resolve(&ctx).await?;
/// let config2 = Injected::<Config>::resolve(&ctx).await?;
///
/// // Resolve without cache
/// let config3 = Injected::<Config>::resolve_uncached(&ctx).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Injected<T: Injectable> {
	inner: Arc<T>,
	metadata: InjectionMetadata,
}

impl<T: Injectable> Injected<T>
where
	T: Clone,
{
	/// Resolve dependency with cache enabled (default)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, InjectionContext, Injectable, SingletonScope};
	/// use std::sync::Arc;
	///
	/// # #[derive(Clone, Default)]
	/// # struct Config;
	/// #
	/// # #[async_trait::async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	/// #
	/// # async fn example() -> reinhardt_di::DiResult<()> {
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// let config = Injected::<Config>::resolve(&ctx).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn resolve(ctx: &InjectionContext) -> DiResult<Self> {
		Self::resolve_with_cache(ctx, true).await
	}

	/// Resolve dependency without cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, InjectionContext, Injectable, SingletonScope};
	/// use std::sync::Arc;
	///
	/// # #[derive(Clone, Default)]
	/// # struct Config;
	/// #
	/// # #[async_trait::async_trait]
	/// # impl Injectable for Config {
	/// #     async fn inject(ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Config::default())
	/// #     }
	/// # }
	/// #
	/// # async fn example() -> reinhardt_di::DiResult<()> {
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// let config = Injected::<Config>::resolve_uncached(&ctx).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn resolve_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		Self::resolve_with_cache(ctx, false).await
	}

	/// Resolve dependency with cache control (internal use)
	///
	/// # Arguments
	///
	/// * `ctx` - Injection context
	/// * `use_cache` - Whether to use request-scoped cache
	async fn resolve_with_cache(ctx: &InjectionContext, use_cache: bool) -> DiResult<Self> {
		with_cycle_detection_scope(async {
			let value = if use_cache {
				// Check request cache first
				if let Some(cached) = ctx.get_request::<T>() {
					Arc::try_unwrap(cached).unwrap_or_else(|arc| (*arc).clone())
				} else {
					// Begin circular dependency detection
					let type_id = TypeId::of::<T>();
					let type_name = std::any::type_name::<T>();
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

	/// Create from value for testing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, Injectable};
	///
	/// # #[derive(Clone, Default)]
	/// # struct Database {
	/// #     connection_count: usize,
	/// # }
	/// #
	/// # #[async_trait::async_trait]
	/// # impl Injectable for Database {
	/// #     async fn inject(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Database::default())
	/// #     }
	/// # }
	/// #
	/// let db = Database { connection_count: 10 };
	/// let injected = Injected::from_value(db);
	/// assert_eq!(injected.connection_count, 10);
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

	/// Extract inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, Injectable};
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
	/// let injected = Injected::from_value(Config::default());
	/// let config = injected.into_inner();
	/// ```
	pub fn into_inner(self) -> T {
		Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone())
	}

	/// Get Arc reference
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, Injectable};
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
	/// let injected = Injected::from_value(Config::default());
	/// let arc: &Arc<Config> = injected.as_arc();
	/// ```
	pub fn as_arc(&self) -> &Arc<T> {
		&self.inner
	}

	/// Get injection metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{Injected, Injectable};
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
	/// let injected = Injected::from_value(Config::default());
	/// let metadata = injected.metadata();
	/// assert!(!metadata.cached);
	/// ```
	pub fn metadata(&self) -> &InjectionMetadata {
		&self.metadata
	}
}

impl<T: Injectable> Deref for Injected<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T: Injectable> Clone for Injected<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
			metadata: self.metadata,
		}
	}
}

impl<T: Injectable> AsRef<T> for Injected<T> {
	fn as_ref(&self) -> &T {
		&self.inner
	}
}

/// Optional injected dependency
///
/// Type alias for `Option<Injected<T>>`, used for optional dependencies.
///
/// # Critical Constraint
///
/// When using `#[inject]` attribute:
/// - `#[inject(optional = true)]` → **MUST** use `OptionalInjected<T>` type
/// - `#[inject(optional = false)]` or `#[inject]` → **MUST** use `Injected<T>` type
/// - Type/attribute mismatches will cause compile errors
///
/// # Examples
///
/// ```
/// use reinhardt_di::{Injected, OptionalInjected};
///
/// // ✅ Correct: optional = true with OptionalInjected<T>
/// // #[get("/data", use_inject = true)]
/// // async fn handler(
/// //     #[inject(optional = true)] cache: OptionalInjected<RedisCache>,
/// // ) -> Result<String> {
/// //     if let Some(cache) = cache {
/// //         Ok(cache.get("data").await?)
/// //     } else {
/// //         Ok("No cache available".to_string())
/// //     }
/// // }
///
/// // ✅ Correct: no optional (default false) with Injected<T>
/// // #[get("/users", use_inject = true)]
/// // async fn list_users(
/// //     #[inject] db: Injected<Database>,
/// // ) -> Result<String> {
/// //     Ok(db.query("SELECT * FROM users").await?)
/// // }
///
/// // ❌ Error: optional = true but type is Injected<T>
/// // #[get("/bad", use_inject = true)]
/// // async fn bad_handler(
/// //     #[inject(optional = true)] cache: Injected<RedisCache>,
/// //     //                                  ^^^^^^^^^^^^^^^^^ Error!
/// // ) -> Result<String> { ... }
///
/// // ❌ Error: optional = false but type is OptionalInjected<T>
/// // #[get("/bad2", use_inject = true)]
/// // async fn bad_handler2(
/// //     #[inject(optional = false)] db: OptionalInjected<Database>,
/// //     //                               ^^^^^^^^^^^^^^^^^^^^^^^^^ Error!
/// // ) -> Result<String> { ... }
/// ```
pub type OptionalInjected<T> = Option<Injected<T>>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::SingletonScope;

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

	#[tokio::test]
	async fn test_injected_from_value() {
		let config = TestConfig {
			value: "custom".to_string(),
		};
		let injected = Injected::from_value(config);
		assert_eq!(injected.value, "custom");
	}

	#[tokio::test]
	async fn test_injected_into_inner() {
		let config = TestConfig {
			value: "test".to_string(),
		};
		let injected = Injected::from_value(config);
		let extracted = injected.into_inner();
		assert_eq!(extracted.value, "test");
	}

	#[tokio::test]
	async fn test_injected_clone() {
		let config = TestConfig {
			value: "test".to_string(),
		};
		let injected1 = Injected::from_value(config);
		let injected2 = injected1.clone();

		assert_eq!(injected1.value, "test");
		assert_eq!(injected2.value, "test");
	}

	#[tokio::test]
	async fn test_injected_deref() {
		let config = TestConfig {
			value: "test".to_string(),
		};
		let injected = Injected::from_value(config);

		// Can be accessed directly via Deref
		assert_eq!(injected.value, "test");
	}

	#[tokio::test]
	async fn test_injected_metadata() {
		let config = TestConfig {
			value: "test".to_string(),
		};
		let injected = Injected::from_value(config);

		let metadata = injected.metadata();
		assert_eq!(metadata.scope, DependencyScope::Request);
		assert!(!metadata.cached);
	}

	#[tokio::test]
	async fn test_optional_injected_some() {
		let config = TestConfig {
			value: "test".to_string(),
		};
		let optional: OptionalInjected<TestConfig> = Some(Injected::from_value(config));

		assert!(optional.is_some());
		if let Some(injected) = optional {
			assert_eq!(injected.value, "test");
		}
	}

	#[tokio::test]
	async fn test_optional_injected_none() {
		let optional: OptionalInjected<TestConfig> = None;
		assert!(optional.is_none());
	}

	// Additional dependency scope tests

	#[test]
	fn test_dependency_scope_equality() {
		assert_eq!(DependencyScope::Request, DependencyScope::Request);
		assert_eq!(DependencyScope::Singleton, DependencyScope::Singleton);
		assert_ne!(DependencyScope::Request, DependencyScope::Singleton);
	}

	#[test]
	fn test_dependency_scope_debug() {
		let request = DependencyScope::Request;
		let singleton = DependencyScope::Singleton;

		let request_debug = format!("{:?}", request);
		let singleton_debug = format!("{:?}", singleton);

		assert!(request_debug.contains("Request"));
		assert!(singleton_debug.contains("Singleton"));
	}

	#[test]
	fn test_dependency_scope_clone() {
		let request = DependencyScope::Request;
		let cloned = request;

		assert_eq!(request, cloned);
	}

	#[test]
	fn test_injection_metadata_debug() {
		let metadata = InjectionMetadata {
			scope: DependencyScope::Request,
			cached: true,
		};

		let debug_str = format!("{:?}", metadata);

		assert!(debug_str.contains("InjectionMetadata"));
		assert!(debug_str.contains("Request"));
		assert!(debug_str.contains("true"));
	}

	#[test]
	fn test_injection_metadata_clone() {
		let metadata = InjectionMetadata {
			scope: DependencyScope::Singleton,
			cached: false,
		};

		let cloned = metadata;

		assert_eq!(cloned.scope, DependencyScope::Singleton);
		assert!(!cloned.cached);
	}

	#[test]
	fn test_injection_metadata_copy() {
		let metadata = InjectionMetadata {
			scope: DependencyScope::Request,
			cached: true,
		};

		// InjectionMetadata derives Copy
		fn takes_copy<T: Copy>(_: T) {}
		takes_copy(metadata);

		// Original is still valid after copy
		assert_eq!(metadata.scope, DependencyScope::Request);
		assert!(metadata.cached);
	}

	#[tokio::test]
	async fn test_injected_as_arc() {
		let config = TestConfig {
			value: "arc_test".to_string(),
		};
		let injected = Injected::from_value(config);

		let arc = injected.as_arc();

		// Arc reference provides access to inner value
		assert_eq!(arc.value, "arc_test");

		// Arc strong count should be 1 (only one reference)
		assert_eq!(Arc::strong_count(arc), 1);
	}

	#[tokio::test]
	async fn test_injected_as_ref() {
		let config = TestConfig {
			value: "ref_test".to_string(),
		};
		let injected = Injected::from_value(config);

		// AsRef trait implementation
		let reference: &TestConfig = injected.as_ref();
		assert_eq!(reference.value, "ref_test");
	}

	#[tokio::test]
	async fn test_injected_debug() {
		let config = TestConfig {
			value: "debug_test".to_string(),
		};
		let injected = Injected::from_value(config);

		let debug_str = format!("{:?}", injected);

		assert!(debug_str.contains("Injected"));
	}

	#[tokio::test]
	async fn test_injected_resolve_with_context() {
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		let config = Injected::<TestConfig>::resolve(&ctx).await.unwrap();

		assert_eq!(config.value, "test");
		assert!(config.metadata().cached);
		assert_eq!(config.metadata().scope, DependencyScope::Request);
	}

	#[tokio::test]
	async fn test_injected_resolve_uncached_with_context() {
		let singleton_scope = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton_scope).build();

		let config = Injected::<TestConfig>::resolve_uncached(&ctx)
			.await
			.unwrap();

		assert_eq!(config.value, "test");
		assert!(!config.metadata().cached);
	}

	#[tokio::test]
	async fn test_injected_clone_shares_arc() {
		let config = TestConfig {
			value: "shared".to_string(),
		};
		let injected1 = Injected::from_value(config);
		let injected2 = injected1.clone();

		// Both should share the same Arc
		assert_eq!(Arc::strong_count(injected1.as_arc()), 2);
		assert_eq!(Arc::strong_count(injected2.as_arc()), 2);

		// Both point to the same data
		assert!(Arc::ptr_eq(injected1.as_arc(), injected2.as_arc()));
	}

	#[tokio::test]
	async fn test_injected_metadata_preserved_on_clone() {
		let config = TestConfig {
			value: "metadata".to_string(),
		};
		let injected1 = Injected::from_value(config);
		let injected2 = injected1.clone();

		// Metadata should be identical
		assert_eq!(injected1.metadata().scope, injected2.metadata().scope);
		assert_eq!(injected1.metadata().cached, injected2.metadata().cached);
	}

	#[tokio::test]
	async fn test_injected_into_inner_with_single_reference() {
		let config = TestConfig {
			value: "single".to_string(),
		};
		let injected = Injected::from_value(config);

		// With single reference, Arc::try_unwrap succeeds
		let inner = injected.into_inner();
		assert_eq!(inner.value, "single");
	}

	#[tokio::test]
	async fn test_injected_into_inner_with_multiple_references() {
		let config = TestConfig {
			value: "multiple".to_string(),
		};
		let injected1 = Injected::from_value(config);
		let _injected2 = injected1.clone(); // Create second reference

		// With multiple references, Arc::try_unwrap fails, falls back to clone
		let inner = injected1.into_inner();
		assert_eq!(inner.value, "multiple");
	}
}
