//! Injectable trait for dependencies

use crate::{DiResult, context::InjectionContext};

/// Injectable trait for dependencies.
///
/// This trait defines how a type can be injected as a dependency.
/// Types implementing this trait can be used with `Depends<T>`.
///
/// # Blanket Implementations
///
/// The following blanket implementations are provided:
///
/// - **`Arc<T>`** where `T: Injectable` — injects the inner `T` and wraps it in `Arc`
/// - **`Depends<T>`** where `T: Send + Sync + 'static` — resolves `T` via the global
///   registry with caching and circular dependency detection
/// - **`Option<T>`** where `T: Injectable` — returns `None` on injection failure
///   instead of propagating the error
///
/// # Custom Implementation
///
/// To make a type injectable, use one of these approaches:
///
/// 1. **`#[injectable]` attribute macro** — generates an `Injectable` impl from
///    a constructor function
/// 2. **`#[injectable_factory]` attribute macro** — generates an `Injectable` impl
///    from a factory function
/// 3. **Manual `impl Injectable`** — implement the trait directly with
///    `#[async_trait]`
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_di::{Injectable, InjectionContext, DiResult, Depends};
/// use async_trait::async_trait;
///
/// # #[derive(Clone)]
/// # struct DbPool;
/// # impl DbPool {
/// #     async fn connect() -> DiResult<Self> { Ok(DbPool) }
/// # }
/// struct Database {
///     pool: DbPool,
/// }
///
/// #[async_trait]
/// impl Injectable for Database {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(Database {
///             pool: DbPool::connect().await?,
///         })
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Injectable: Sized + Send + Sync + 'static {
	/// Creates an instance of this type by resolving dependencies from the injection context.
	async fn inject(ctx: &InjectionContext) -> DiResult<Self>;

	/// Inject without using cache (for `cache = false` support).
	///
	/// This method creates a new instance without checking or updating any cache.
	/// By default, it delegates to `inject()`, but types can override this
	/// to provide cache-free injection.
	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		Self::inject(ctx).await
	}
}

/// Blanket implementation of Injectable for `Arc<T>`
///
/// This allows using `Arc<T>` directly in endpoint handlers with `#[inject]`:
///
/// ```ignore
/// # use reinhardt_di::Injectable;
/// # use std::sync::Arc;
/// # struct DatabaseConnection;
/// # struct Response;
/// # type ViewResult<T> = Result<T, Box<dyn std::error::Error>>;
/// # use reinhardt_core::endpoint;
/// #[endpoint]
/// async fn handler(
///     #[inject] db: Arc<DatabaseConnection>,
/// ) -> ViewResult<Response> {
///     // ...
/// #   Ok(Response)
/// }
/// ```
///
/// The implementation injects `T` first, then wraps it in `Arc`.
#[async_trait::async_trait]
impl<T> Injectable for std::sync::Arc<T>
where
	T: Injectable,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		T::inject(ctx).await.map(std::sync::Arc::new)
	}

	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		T::inject_uncached(ctx).await.map(std::sync::Arc::new)
	}
}

/// Blanket implementation of Injectable for `Depends<T>`
///
/// This allows using `Depends<T>` directly in endpoint handlers with `#[inject]`:
///
/// ```ignore
/// # use reinhardt_di::{Depends, Injectable};
/// # struct DatabaseConnection;
/// # struct Response;
/// # type ViewResult<T> = Result<T, Box<dyn std::error::Error>>;
/// # use reinhardt_core::endpoint;
/// #[endpoint]
/// async fn handler(
///     #[inject] db: Depends<DatabaseConnection>,
/// ) -> ViewResult<Response> {
///     // ...
/// #   Ok(Response)
/// }
/// ```
///
/// The implementation delegates to `Depends::resolve()`, which resolves `T`
/// from the global registry with caching and circular dependency detection.
/// Falls back to `T::inject()` if the type is not in the global registry.
#[async_trait::async_trait]
impl<T> Injectable for crate::depends::Depends<T>
where
	T: Injectable,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		crate::depends::Depends::<T>::resolve(ctx, true).await
	}

	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		crate::depends::Depends::<T>::resolve(ctx, false).await
	}
}

/// Blanket implementation of Injectable for `Option<T>`
///
/// This allows optional injection where failure results in `None`
/// instead of an error. Useful for endpoints that serve both
/// authenticated and anonymous users.
///
/// # Security Note
///
/// `Option<T>` swallows ALL injection errors into `None`.
/// For security-critical endpoints, use `T` directly to ensure
/// errors are surfaced as HTTP 401/500.
///
/// ```ignore
/// # use reinhardt_di::Injectable;
/// # struct AuthInfo;
/// # struct Response;
/// # type ViewResult<T> = Result<T, Box<dyn std::error::Error>>;
/// # use reinhardt_core::endpoint;
/// #[endpoint]
/// async fn handler(
///     #[inject] auth: Option<AuthInfo>,
/// ) -> ViewResult<Response> {
///     // auth is None if not authenticated
/// #   Ok(Response)
/// }
/// ```
#[async_trait::async_trait]
impl<T> Injectable for Option<T>
where
	T: Injectable,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		match T::inject(ctx).await {
			Ok(value) => Ok(Some(value)),
			Err(_) => Ok(None),
		}
	}

	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		match T::inject_uncached(ctx).await {
			Ok(value) => Ok(Some(value)),
			Err(_) => Ok(None),
		}
	}
}
