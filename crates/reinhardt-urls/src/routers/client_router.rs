//! Client-side routing for Single Page Applications.
//!
//! This module provides client-side routing functionality that works with
//! the browser History API for SPA navigation.
//!
//! # Architecture
//!
//! [`ClientRouter`] renders views using the [`Page`] type from `reinhardt_core::page`.
//! All route handlers must return `Page` values.
//!
//! # Features
//!
//! - URL pattern matching with path parameters
//! - Named routes for reverse URL generation
//! - Route guards for access control
//! - Browser History API integration
//! - Reactive signals for current path, params, and route name
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_urls::routers::client_router::{ClientRouter, Path};
//! use reinhardt_core::page::Page;
//!
//! // Create a router with routes
//! let router = ClientRouter::new()
//!     .route("/", || home_page())
//!     .route_path("/users/{id}/", |Path(id): Path<u64>| {
//!         user_page(id)
//!     })
//!     .named_route("settings", "/settings/", || settings_page())
//!     .not_found(|| not_found_page());
//!
//! // Setup browser history listener
//! router.setup_history_listener();
//!
//! // Navigate programmatically
//! router.push("/users/42/");
//!
//! // Reverse URL generation
//! let url = router.reverse("settings", &[]).unwrap();
//! assert_eq!(url, "/settings/");
//! ```
//!
//! # Path Parameter Extraction
//!
//! Path parameters can be extracted using the `Path<T>` extractor:
//!
//! ```rust,ignore
//! // Single parameter
//! .route_path("/posts/{id}/", |Path(id): Path<i64>| {
//!     post_page(id)
//! })
//!
//! // Multiple parameters
//! .route_path2("/users/{user_id}/posts/{post_id}/",
//!     |Path(user_id): Path<u64>, Path(post_id): Path<u64>| {
//!         user_post_page(user_id, post_id)
//!     })
//! ```
//!
//! [`Page`]: reinhardt_core::page::Page

mod core;
mod error;
mod global;
mod handler;
mod history;
mod params;
mod pattern;
mod reverser;

// Public re-exports
pub use core::{ClientRoute, ClientRouteMatch, ClientRouter};
pub use error::{PathError, RouterError};
pub use global::{clear_client_reverser, get_client_reverser, register_client_reverser};
pub use handler::RouteHandler;
pub use history::{
	HistoryState, NavigationType, current_path, go_back, go_forward, push_state, replace_state,
};
pub use params::{FromPath, ParamContext, Path, SingleFromPath};
pub use pattern::ClientPathPattern;
pub use reverser::ClientUrlReverser;
