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
//!     .route("home", "/", || home_page())
//!     .route_path("user_detail", "/users/{id}/", |Path(id): Path<u64>| {
//!         user_page(id)
//!     })
//!     .route("settings", "/settings/", || settings_page())
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
//! .route_path("post_detail", "/posts/{id}/", |Path(id): Path<i64>| {
//!     post_page(id)
//! })
//!
//! // Multiple parameters — same method, the arity is inferred from
//! // the closure signature (Issue #4637).
//! .route_path("user_post", "/users/{user_id}/posts/{post_id}/",
//!     |Path(user_id): Path<u64>, Path(post_id): Path<u64>| {
//!         user_post_page(user_id, post_id)
//!     })
//! ```
//!
//! [`Page`]: reinhardt_core::page::Page
//!
//! # Reactive navigation observation
//!
//! `ClientRouter` exposes [`ClientRouter::on_navigate`] returning a
//! [`NavigationSubscription`] handle, plus diagnostic counters
//! (`__diag_observer_count`, `__diag_dispatch_count`,
//! `__diag_router_id`). These were ported from `pages::Router` in
//! `0.1.0-rc.27`; behaviour matches the pages-side invariants
//! Inv-1 ~ Inv-6 documented in `pages::router::core`.
//! See `kent8192/reinhardt-web#4234`.

pub mod component;
mod core;
mod error;
pub mod from_request;
mod handler;
// Issue #4217: `history` is exposed publicly so reinhardt-pages can
// re-export the canonical primitives. The functions inside remain
// callable cross-crate, but the more ergonomic re-exports at this
// module level are intentionally limited (see below).
pub mod history;
pub mod loader;
mod params;
mod pattern;
pub mod scope;
pub mod tree;
// Public re-exports
pub use component::{
	ComponentInfo, ComponentMetadata, FromLayoutRequest, LayoutInfo, LayoutMetadata,
};
pub use core::{
	ClientRoute, ClientRouteMatch, ClientRouter, NavigationSubscription, RouteMetadata,
};
pub use error::{MergeError, PathError, RouteRegistrationError, RouterError};
// Re-export the `FromRequest` building blocks at the
// `client_router` module level so callers can write
// `use reinhardt_urls::routers::client_router::{FromRequest, ...}`.
pub use from_request::{
	ExtractError, FromRequest, OptionalQueryParam, PathParam, QueryParam, RouteContext,
};
pub use handler::RouteHandler;
pub use loader::RouteLoaderId;
// Issue #4217: drop helper-function re-exports from this module's
// public surface. Callers should use `Router::push()` / `ClientRouter::push()`
// instead. The functions remain `pub` at `history::*` so reinhardt-pages
// can re-export them across the crate boundary.
pub use history::{HistoryState, NavigationType};
#[cfg(wasm)]
pub use history::{PopNavigationRequest, PopStateSubscription, listen_pop_requests};
pub use params::{FromPath, ParamContext, Path, SingleFromPath};
pub use pattern::ClientPathPattern;
pub use scope::{RouteScope, ScopeKind};
pub use tree::{
	ClientRouteTreeMatch, LayoutKey, MatchedLayout, ResolvedRouteMetadata, RouteNode, RouteNodeKind,
};
