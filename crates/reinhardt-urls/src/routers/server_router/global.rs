//! Global router registry for URL inspection (showurls command)

use crate::routers::server_router::ServerRouter;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock as StdRwLock;

/// Global router registry
static GLOBAL_ROUTER: OnceCell<StdRwLock<Option<Arc<ServerRouter>>>> = OnceCell::new();

/// Global deferred DI registrations from route configuration
static GLOBAL_DI_REGISTRATIONS: OnceCell<StdRwLock<Option<reinhardt_di::DiRegistrationList>>> =
	OnceCell::new();

/// Register the application's main router globally
///
/// This allows commands like `showurls` to inspect registered routes.
/// The router is automatically wrapped in `Arc` internally.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_urls::routers::{ServerRouter, register_router};
/// use hyper::Method;
/// # use reinhardt_http::{Request, Response, Result};
/// # async fn health_handler(_req: Request) -> Result<Response> {
/// #     Ok(Response::ok())
/// # }
///
/// let router = ServerRouter::new()
///     .with_prefix("/api/v1")
///     .function("/health", Method::GET, health_handler);
///
/// // No Arc::new() needed!
/// register_router(router);
/// ```
pub fn register_router(mut router: ServerRouter) {
	let errors = router.register_all_routes();
	for error in &errors {
		tracing::warn!("{}", error);
	}
	register_router_arc(Arc::new(router));
}

/// Register a router that is already wrapped in Arc.
///
/// This is provided for cases where you already have an `Arc<ServerRouter>`.
/// In most cases, you should use [`register_router()`] instead.
///
/// **Important:** Unlike [`register_router()`], this function does **not** call
/// `register_all_routes()` because `Arc<ServerRouter>` cannot be mutated.
/// Callers must ensure routes have been registered before wrapping in `Arc`.
pub fn register_router_arc(router: Arc<ServerRouter>) {
	let cell = GLOBAL_ROUTER.get_or_init(|| StdRwLock::new(None));
	let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
	*guard = Some(router);
}

/// Get a reference to the globally registered router
///
/// Returns `None` if no router has been registered.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_urls::routers::get_router;
///
/// if let Some(router) = get_router() {
///     let routes = router.get_all_routes();
///     println!("Registered routes: {}", routes.len());
/// }
/// ```
pub fn get_router() -> Option<Arc<ServerRouter>> {
	GLOBAL_ROUTER
		.get()
		.and_then(|cell| cell.read().unwrap_or_else(PoisonError::into_inner).clone())
}

/// Check if a router has been registered
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_urls::routers::is_router_registered;
///
/// if !is_router_registered() {
///     println!("Warning: No router registered");
/// }
/// ```
pub fn is_router_registered() -> bool {
	GLOBAL_ROUTER
		.get()
		.map(|cell| {
			cell.read()
				.unwrap_or_else(PoisonError::into_inner)
				.is_some()
		})
		.unwrap_or(false)
}

/// Register deferred DI registrations globally
///
/// These registrations are captured during route configuration (e.g., in
/// `routes()` functions) and applied to the server's [`SingletonScope`]
/// during startup. This bridges the lifecycle gap between synchronous
/// route setup and the server's DI context creation.
///
/// [`SingletonScope`]: reinhardt_di::SingletonScope
pub fn register_di_registrations(list: reinhardt_di::DiRegistrationList) {
	let cell = GLOBAL_DI_REGISTRATIONS.get_or_init(|| StdRwLock::new(None));
	let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
	match guard.as_mut() {
		Some(existing) => existing.merge(list),
		None => *guard = Some(list),
	}
}

/// Take the globally registered DI registrations
///
/// Returns `None` if no registrations have been stored.
/// After this call, the global cell is emptied (subsequent calls return `None`).
pub fn take_di_registrations() -> Option<reinhardt_di::DiRegistrationList> {
	GLOBAL_DI_REGISTRATIONS
		.get()
		.and_then(|cell| cell.write().unwrap_or_else(PoisonError::into_inner).take())
}

/// Get a clone of the DI context from the globally registered router, if one was set.
///
/// This allows server startup code to detect when the user has already
/// configured a DI context on their router (via [`ServerRouter::with_di_context`]
/// or [`UnifiedRouter::with_di_context`]) and reuse its singleton scope
/// instead of creating a new one.
///
/// Returns `None` if no router is registered or the router has no DI context.
///
/// [`UnifiedRouter::with_di_context`]: crate::routers::UnifiedRouter::with_di_context
pub fn get_router_di_context() -> Option<Arc<reinhardt_di::InjectionContext>> {
	get_router().and_then(|router| router.di_context().cloned())
}

/// Clear the registered router (useful for tests)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_urls::routers::{clear_router, is_router_registered};
///
/// clear_router();
/// assert!(!is_router_registered());
/// ```
pub fn clear_router() {
	if let Some(cell) = GLOBAL_ROUTER.get() {
		let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
		*guard = None;
	}
	// Also clear deferred DI registrations
	if let Some(cell) = GLOBAL_DI_REGISTRATIONS.get() {
		let mut guard = cell.write().unwrap_or_else(PoisonError::into_inner);
		*guard = None;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_testkit::resource::{TeardownGuard, TestResource};
	use rstest::{fixture, rstest};

	/// Fixture that clears global router state before each test
	/// and restores it after test completion via RAII.
	struct CleanGlobalRouter;

	impl TestResource for CleanGlobalRouter {
		fn setup() -> Self {
			clear_router();
			Self
		}

		fn teardown(&mut self) {
			clear_router();
		}
	}

	#[fixture]
	fn env() -> TeardownGuard<CleanGlobalRouter> {
		TeardownGuard::new()
	}

	#[rstest]
	#[serial_test::serial(global_router)]
	fn returns_none_when_no_router_registered(_env: TeardownGuard<CleanGlobalRouter>) {
		// Act
		let result = get_router_di_context();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_router)]
	fn returns_none_when_router_has_no_di_context(_env: TeardownGuard<CleanGlobalRouter>) {
		// Arrange
		let router = crate::routers::ServerRouter::new();
		register_router(router);

		// Act
		let result = get_router_di_context();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_router)]
	fn returns_context_when_router_has_di_context(_env: TeardownGuard<CleanGlobalRouter>) {
		// Arrange
		let singleton_scope = Arc::new(reinhardt_di::SingletonScope::new());
		singleton_scope.set(42u32);
		let di_ctx = Arc::new(reinhardt_di::InjectionContext::builder(singleton_scope).build());

		let router = crate::routers::ServerRouter::new().with_di_context(di_ctx);
		register_router(router);

		// Act
		let result = get_router_di_context();

		// Assert
		assert!(result.is_some());
		let ctx = result.unwrap();
		let value = ctx.singleton_scope().get::<u32>();
		assert!(value.is_some());
		assert_eq!(*value.unwrap(), 42);
	}
}
