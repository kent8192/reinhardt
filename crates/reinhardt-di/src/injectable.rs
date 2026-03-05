//! Injectable trait for dependencies

use crate::{DiResult, context::InjectionContext};

/// Injectable trait for dependencies.
///
/// This trait defines how a type can be injected as a dependency.
/// Types implementing this trait can be used with `Depends<T>`.
///
/// # Automatic Implementation
///
/// Types that implement `Default + Clone + Send + Sync + 'static` automatically
/// get an `Injectable` implementation that:
/// 1. Checks if the value is already cached in the request scope
/// 2. Checks if the value is available in the singleton scope
/// 3. Creates a new instance using `Default::default()`
///
/// This automatic behavior is similar to FastAPI's dependency injection,
/// where simple types can be auto-injected without explicit implementation.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_di::{Injectable, InjectionContext, DiResult, Depends};
/// use async_trait::async_trait;
///
/// // Automatic injection for types with Default + Clone
/// #[derive(Default, Clone)]
/// struct Config {
///     api_key: String,
/// }
///
// Config now has Injectable automatically
// Can be used directly: Depends<Config>
///
// Custom injection logic
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
///         // Custom logic here
///         Ok(Database {
///             pool: DbPool::connect().await?,
///         })
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Injectable: Sized + Send + Sync + 'static {
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
	T: Injectable + Clone,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		T::inject(ctx).await.map(std::sync::Arc::new)
	}

	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		T::inject_uncached(ctx).await.map(std::sync::Arc::new)
	}
}
