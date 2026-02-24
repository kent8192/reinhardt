//! Client-side routing
//!
//! This module provides the client-side router for the Twitter clone application.
//! Routes are defined in each app's `urls.rs` using the `UnifiedRouter<Page>` pattern,
//! and this module handles router initialization and global access.

use reinhardt::ClientRouter;
use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use std::cell::RefCell;
use uuid::Uuid;

use crate::config::urls::routes;

// Global Router instance
thread_local! {
	static ROUTER: RefCell<Option<ClientRouter<Page>>> = const { RefCell::new(None) };
}

/// Initialize the global router instance
///
/// This must be called once at application startup before any routing operations.
/// It registers server routes globally and sets up the client router with
/// history API listener for browser back/forward navigation.
pub fn init_global_router() {
	ROUTER.with(|r| {
		// Get unified routes from config and split into server/client
		let unified = routes();
		// Register server router globally and get client router
		let client = unified.register_globally();
		// Set up popstate listener for browser back/forward navigation
		client.setup_history_listener();
		*r.borrow_mut() = Some(client);
	});
}

/// Provides access to the global router instance
///
/// # Panics
///
/// Panics if the router has not been initialized via `init_global_router()`.
pub fn with_router<F, R>(f: F) -> R
where
	F: FnOnce(&ClientRouter<Page>) -> R,
{
	ROUTER.with(|r| {
		f(r.borrow()
			.as_ref()
			.expect("Router not initialized. Call init_global_router() first."))
	})
}

/// Home page view
pub fn home_page_view() -> Page {
	__reinhardt_placeholder__!(/*0*/)()
}

/// Login page view
pub fn login_page_view() -> Page {
	use crate::apps::auth::client::components::login_form;
	login_form()
}

/// Register page view
pub fn register_page_view() -> Page {
	use crate::apps::auth::client::components::register_form;
	register_form()
}

/// Profile page view
pub fn profile_page_view(user_id: Uuid) -> Page {
	use crate::apps::profile::client::components::profile_view;

	profile_view(user_id)
}

/// Profile edit page view
pub fn profile_edit_page_view(user_id: Uuid) -> Page {
	use crate::apps::profile::client::components::profile_edit;

	profile_edit(user_id)
}

/// Timeline page view
pub fn timeline_page_view() -> Page {
	use crate::apps::tweet::client::components::{tweet_form, tweet_list};

	// Get component views
	let form_view = tweet_form();
	let list_view = tweet_list(None);

	__reinhardt_placeholder__!(/*1*/)(form_view, list_view)
}

/// DM chat page view
pub fn dm_chat_page_view(room_id: String) -> Page {
	use crate::apps::dm::client::components::dm_chat;

	dm_chat(room_id)
}

/// Not found page view
pub fn not_found_page_view() -> Page {
	__reinhardt_placeholder__!(/*2*/)()
}
