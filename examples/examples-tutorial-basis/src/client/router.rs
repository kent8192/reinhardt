//! Client-side routing
//!
//! This module defines the client-side router for the polling application.

use crate::client::pages::{index_page, polls_detail_page, polls_results_page};
use reinhardt::pages::component::View;
use reinhardt::pages::page;
use reinhardt::pages::router::Router;
use std::cell::RefCell;

// Global Router instance
thread_local! {
	static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Initialize the global router instance
///
/// This must be called once at application startup before any routing operations.
pub fn init_global_router() {
	ROUTER.with(|r| {
		*r.borrow_mut() = Some(init_router());
	});
}

/// Provides access to the global router instance
///
/// # Panics
///
/// Panics if the router has not been initialized via `init_global_router()`.
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

/// Initialize the router with all application routes
fn init_router() -> Router {
	Router::new()
		// Home/Index route - List all polls
		.route("/", || index_page())
		// Poll detail route with dynamic parameter
		.route("/polls/{question_id}/", || {
			with_router(|r| {
				let params = r.current_params().get();
				let question_id_str = params
					.get("question_id")
					.cloned()
					.unwrap_or_else(|| "0".to_string());

				// Parse question_id
				match question_id_str.parse::<i64>() {
					Ok(question_id) => polls_detail_page(question_id),
					Err(_) => error_page("Invalid question ID"),
				}
			})
		})
		// Poll results route
		.route("/polls/{question_id}/results/", || {
			with_router(|r| {
				let params = r.current_params().get();
				let question_id_str = params
					.get("question_id")
					.cloned()
					.unwrap_or_else(|| "0".to_string());

				// Parse question_id
				match question_id_str.parse::<i64>() {
					Ok(question_id) => polls_results_page(question_id),
					Err(_) => error_page("Invalid question ID"),
				}
			})
		})
		// 404 not found
		.not_found(|| error_page("Page not found"))
}

/// Error page
fn error_page(message: &str) -> View {
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
