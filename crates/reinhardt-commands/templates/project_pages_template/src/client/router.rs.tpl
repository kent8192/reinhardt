//! Client-side router for {{ project_name }}.
//!
//! The router is built once at startup by [`init_global_router`]. Use
//! [`with_router`] from any component to inspect routing state.

use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use reinhardt::pages::router::Router;
use std::cell::RefCell;

// Global Router instance
thread_local! {
	static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Initialize the global router instance.
///
/// Must be called once at application startup before any routing operations.
pub fn init_global_router() {
	ROUTER.with(|r| {
		*r.borrow_mut() = Some(init_router());
	});
}

/// Provides access to the global router instance.
///
/// # Panics
///
/// Panics if the router has not been initialized via [`init_global_router`].
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

/// Build the application router.
///
/// Add new routes here, e.g.:
///
/// ```rust,ignore
/// .route("/", || crate::client::pages::index_page())
/// ```
fn init_router() -> Router {
	Router::new()
		// Add routes here
		.not_found(|| not_found_page("Page not found"))
}

/// Default 404 / error page used by `init_router`.
fn not_found_page(message: &str) -> Page {
	let message = message.to_string();
	page!(|message: String| {
		div {
			class: "container mt-5",
			div {
				class: "alert alert-danger",
				{ message }
			}
			a {
				href: "/",
				class: "btn btn-primary",
				"Back to Home"
			}
		}
	})(message)
}
