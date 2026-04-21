//! Client-side routing for {{ project_name }}
//!
//! Defines client-side routes using `reinhardt::pages::router::Router`.
//! Add new routes by calling `.route(path, handler)` inside `init_router()`.

use reinhardt::pages::router::Router;
use std::cell::RefCell;

thread_local! {
	static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Initialize the global router instance.
///
/// Call once at application startup before any routing operations.
pub fn init_global_router() {
	ROUTER.with(|r| {
		*r.borrow_mut() = Some(init_router());
	});
}

/// Provides access to the global router instance.
///
/// # Panics
///
/// Panics if `init_global_router()` has not been called.
pub fn with_router<F, R>(f: F) -> R
where
	F: FnOnce(&Router) -> R,
{
	ROUTER.with(|r| {
		f(r.borrow()
			.as_ref()
			.expect("Router not initialized. Call init_global_router() first."))
	})
}

fn init_router() -> Router {
	Router::new()
		// Add routes here, e.g.:
		// .route("/", || home_page())
		// .route("/about/", || about_page())
}

