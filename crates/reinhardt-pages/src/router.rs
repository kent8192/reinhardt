//! Client-Side Router for reinhardt-pages
//!
//! This module provides a client-side routing system compatible with reinhardt-urls.
//! It supports the History API for navigation without full page reloads.
//!
//! ## Features
//!
//! - **Path Pattern Matching**: Django-style URL patterns (`/users/{id}/`)
//! - **History API Integration**: `pushState` and `popstate` handling
//! - **Named Routes**: Reverse URL lookup by route name
//! - **Reactive Navigation**: Signal-based current route tracking
//! - **Route Guards**: Optional authentication/authorization checks
//! - **Route Loaders**: Entry-blocking layout and leaf data with prefetch
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::router::{Link, RouterOutlet};
//! use reinhardt_urls::routers::ClientRouter;
//!
//! // Create a router
//! let router = ClientRouter::new()
//!     .route("home", "/", home_page)
//!     .route("user_list", "/users/", user_list)
//!     .route("user_detail", "/users/{id}/", user_detail);
//!
//! // Navigate programmatically
//! router.push("/users/42/");
//!
//! // Reverse URL lookup
//! let url = router.reverse("user_detail", &[("id", "42")]);
//!
//! // Create a router outlet to render current route
//! let outlet = RouterOutlet::new(router);
//! ```
//!
//! # Migration to `urls::ClientRouter`
//!
//! As of `0.1.0-rc.27`, `Router` and its related types are
//! `#[deprecated]`. New code should use
//! `reinhardt_urls::routers::ClientRouter` instead. The migration
//! map for the most common patterns is:
//!
//! Note: in the table below, `urls::*` abbreviates
//! `reinhardt_urls::routers::*`.
//!
//! ```text
//! pages::Router               -> urls::ClientRouter
//! pages::Route                -> urls::ClientRoute
//! pages::RouteMatch           -> urls::ClientRouteMatch
//! pages::PathPattern          -> urls::ClientPathPattern
//! pages::PathParams<T>        -> urls::Path<T>
//! pages::NavigationSubscription -> urls::NavigationSubscription
//! pages::RouterError          -> urls::RouterError
//! pages::PathError            -> urls::PathError
//! ClientLauncher::router(F)   -> ClientLauncher::router_client(F)
//! ```
//!
//! `Link` and `RouterOutlet` remain in this module and are NOT
//! deprecated — they are rendering primitives with no migration target.
//!
//! See `kent8192/reinhardt-web#4234` for the full design.
//!
//! # Layout Routes
//!
//! `ClientRouter::routes` supports nested layout routes for SPA shells. A
//! `#[layout]` function receives normal `Path` / `Query` extractors plus one
//! [`Outlet`](crate::component::Outlet) parameter, and child `#[component]`
//! paths are relative to that layout scope:
//!
//! ```ignore
//! use reinhardt_pages::{Outlet, Page, Path, component, layout, page};
//! use reinhardt_urls::routers::ClientRouter;
//!
//! #[layout("/workspaces/{workspace_id}/", name = "workspace-shell")]
//! fn workspace_shell(Path(workspace_id): Path<i64>, outlet: Outlet) -> Page {
//!     page!(|workspace_id: i64, outlet: Outlet| {
//!         section {
//!             h1 { { format!("Workspace {workspace_id}") } }
//!             { outlet }
//!         }
//!     })(workspace_id, outlet)
//! }
//!
//! #[component("jobs", name = "workspace-jobs")]
//! fn workspace_jobs(Path(workspace_id): Path<i64>) -> Page {
//!     page!(|workspace_id: i64| {
//!         p { { format!("Jobs for {workspace_id}") } }
//!     })(workspace_id)
//! }
//!
//! let router = ClientRouter::new().routes(|routes| {
//!     routes.layout(workspace_shell, |children| {
//!         children.component(workspace_jobs)
//!     })
//! });
//! ```
//!
//! On browser WASM, navigating between sibling children with the same layout
//! key preserves the layout shell and remounts only the outlet subtree.
//!
//! Route loaders are bound with `loader = ...` on `#[component]` and
//! `#[layout]`; see [`crate::router::loader`] and
//! `docs/route_loaders.md` for the prepare/commit and hydration contract.

mod components;
mod history;
mod navigate;
mod params;

pub mod loader;
pub mod loader_registry;
pub mod loader_store;

/// Manouche DSL v2 spec §4.3 `FromRequest`-based page handlers.
///
/// Re-exports the building blocks for
/// `ClientRouter::page<F, P>(pattern, handler)` where
/// `P: FromRequest`. The canonical implementations live in
/// `reinhardt_urls::routers::client_router::from_request` (because
/// `ClientRouter` itself is defined there); they are re-exposed here
/// so application code can write
/// `use reinhardt_pages::router::request::FromRequest;` matching the
/// spec's namespace.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::router::request::{
///     ExtractError, FromRequest, PathParam, RouteContext,
/// };
/// use reinhardt_urls::routers::ClientRouter;
///
/// struct UserPageProps { id: PathParam<i32> }
///
/// impl FromRequest for UserPageProps {
///     fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError> {
///         Ok(Self { id: PathParam::extract(ctx, "id")? })
///     }
/// }
/// ```
///
/// The legacy non-generic `PathParam` re-exported at
/// `reinhardt_pages::router::PathParam` (deprecated since
/// `0.1.0-rc.27`) is unrelated to the v2 `PathParam<T>` extractor in
/// this submodule.
pub mod request {
	pub use reinhardt_urls::routers::client_router::from_request::{
		ExtractError, FromRequest, PathParam, QueryParam, RouteContext,
	};
}

pub use components::{Link, PrefetchMode, Redirect, RouterOutlet, guard, guard_or};
pub use history::{HistoryState, NavigationType};
pub use navigate::{navigate, navigate_named, navigate_or_reload};
pub use params::route_params;
pub use reinhardt_urls::routers::ClientRouter;
pub use reinhardt_urls::routers::client_router::{Path, RouteLoaderId};
// `setup_popstate_listener` is wasm-only — see `history` module docs.
#[cfg(wasm)]
pub use history::setup_popstate_listener;

/// Query extractor marker used by `#[component]` function signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Query<T>(pub T);

impl<T> Query<T> {
	/// Unwraps the inner value.
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> std::ops::Deref for Query<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T> AsRef<T> for Query<T> {
	fn as_ref(&self) -> &T {
		&self.0
	}
}
