//! Internal launch-time router abstraction. (Refs #4234)
//!
//! `trait SpaRouter` abstracts the launch-time dispatch surface of
//! [`reinhardt_urls::routers::ClientRouter`], letting
//! [`crate::app::ClientLauncher::launch`] operate against a single
//! `Box<dyn SpaRouter>`. The trait enables the launcher to stay agnostic
//! of the concrete router type and supports the hidden
//! [`crate::app::with_spa_router`] accessor used by tests and
//! macro-generated code.
//!
//! On non-wasm targets, the launcher's `launch()` body is largely behind
//! `#[cfg(wasm)]`, so the trait methods are not invoked at compile time.
//! The `#![cfg_attr(not(wasm), allow(dead_code))]` below silences the
//! resulting native-only `dead_code` warnings.
//!
//! The trait is intentionally **object-safe**: every method takes
//! concrete (non-generic) inputs and outputs, and the user-supplied
//! navigation listener is taken as a `Box<dyn Fn ...>` rather than a
//! generic `F: Fn ...` so `Box<dyn SpaRouter>` is dyn-compatible.
//!
//! `SpaRouteMatch` and `SpaRouterError` are launcher-internal
//! translations of the per-router native types so the launcher does not
//! depend on either crate's concrete error / match type. The
//! translations are intentionally lossy (route name -> `Option<String>`,
//! errors -> `String`) because the launcher only needs the path string,
//! parameter map, optional route name, and printable error message.

#![cfg_attr(not(wasm), allow(dead_code))] // (Refs #4234) On native, the launcher's launch() body is largely #[cfg(wasm)], so the trait methods appear unused.

use crate::component::Page;
use crate::reactive::Signal;
use std::collections::HashMap;

/// Internal launch-time router abstraction. (Refs #4234)
///
/// Object-safe by construction: no generic methods, no associated
/// types. The navigation observer registration takes a boxed listener
/// (`Box<dyn Fn ...>`) so the trait stays object-safe; the returned
/// subscription handle is opaque (`Box<dyn Any>`) because each backing
/// router crate defines its own `NavigationSubscription` type.
///
/// `as_any` supports downcasting for diagnostic and test use.
#[doc(hidden)]
pub trait SpaRouter: 'static {
	/// Reactive subscription to the current path.
	fn current_path(&self) -> &Signal<String>;

	/// Reactive subscription to the current route's path parameters.
	#[allow(dead_code)] // Part of the symmetric dispatch surface (mirrors `current_path`); not yet read by `launch`.
	fn current_params(&self) -> &Signal<HashMap<String, String>>;

	/// Match `path` against registered routes, returning a
	/// launcher-internal [`SpaRouteMatch`] when the active router's
	/// native match succeeds (and any guard accepts the match).
	fn match_path(&self, path: &str) -> Option<SpaRouteMatch>;

	/// Render the currently-active route into a [`Page`].
	fn render_current(&self) -> Page;

	/// Install the browser `popstate` listener on the active router.
	/// On non-WASM targets this is a no-op.
	fn setup_history_listener(&self);

	/// Push a new history entry and dispatch navigation observers.
	fn push(&self, path: &str) -> Result<(), SpaRouterError>;

	/// Replace the current history entry and dispatch navigation
	/// observers.
	#[allow(dead_code)] // Part of the symmetric dispatch surface (mirrors `push`); not yet read by `launch`.
	fn replace(&self, path: &str) -> Result<(), SpaRouterError>;

	/// Resolves a registered route name and parameters to its path.
	fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Result<String, SpaRouterError>;

	/// Number of registered routes. Used by the launcher's
	/// `nav_diag!` traces.
	fn route_count(&self) -> usize;

	/// Register a navigation observer.
	///
	/// The returned `Box<dyn Any>` is an opaque handle that owns the
	/// underlying `NavigationSubscription`; the launcher calls
	/// `mem::forget` on it (matching the previous `Router::on_navigate`
	/// flow) so the listener lives for the entire WASM module lifetime.
	#[allow(clippy::type_complexity)] // The boxed listener signature is dictated by trait object-safety; extracting a type alias would not improve readability.
	fn on_navigate_dyn(
		&self,
		listener: Box<dyn Fn(&str, &HashMap<String, String>) + 'static>,
	) -> Box<dyn std::any::Any>;

	/// Cumulative `notify_observers` invocation count. Used by tests
	/// to assert observer-system invariants.
	#[doc(hidden)]
	fn __diag_dispatch_count(&self) -> u64;

	/// Number of currently-alive navigation observers. Used by the
	/// launcher's `nav_diag!` traces and tests.
	#[doc(hidden)]
	fn __diag_observer_count(&self) -> usize;

	/// Stable per-instance router id for diagnostic correlation.
	#[doc(hidden)]
	fn __diag_router_id(&self) -> usize;

	/// Downcast to the concrete backing router.
	fn as_any(&self) -> &dyn std::any::Any;
}

/// Internal match result that normalises [`crate::router::RouteMatch`]
/// and [`reinhardt_urls::routers::ClientRouteMatch`].
///
/// `path` is the path the launcher asked to match (stored back so the
/// launcher does not need to keep a separate reference). `params` is a
/// snapshot of the matched parameter map. `name` is the route's
/// optional name, if any (mirrors `Route::name()` /
/// `ClientRoute::name()`).
#[doc(hidden)]
pub struct SpaRouteMatch {
	#[allow(dead_code)] // Reserved for diagnostic use; not yet read by `launch`.
	pub path: String,
	#[allow(dead_code)] // Reserved for diagnostic use; not yet read by `launch`.
	pub params: HashMap<String, String>,
	pub name: Option<String>,
}

/// Internal error type that wraps either
/// [`reinhardt_urls::routers::client_router::error::RouterError`] for
/// launcher-internal use. Stringly-typed: the launcher only renders
/// errors via `JsValue::from_str(...)`.
#[derive(Debug)]
#[doc(hidden)]
pub enum SpaRouterError {
	/// Wraps the underlying router error's `Display` representation.
	#[allow(dead_code)] // Constructed by trait impls; consumed via `Display`.
	Inner(String),
}

impl std::fmt::Display for SpaRouterError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Inner(msg) => write!(f, "{}", msg),
		}
	}
}

impl std::error::Error for SpaRouterError {}

impl SpaRouter for reinhardt_urls::routers::ClientRouter {
	fn current_path(&self) -> &Signal<String> {
		self.current_path()
	}

	fn current_params(&self) -> &Signal<HashMap<String, String>> {
		self.current_params()
	}

	fn match_path(&self, path: &str) -> Option<SpaRouteMatch> {
		self.match_path(path).map(|m| SpaRouteMatch {
			path: path.to_string(),
			params: m.params.clone(),
			name: m.route.name().map(str::to_string),
		})
	}

	fn render_current(&self) -> Page {
		self.render_current()
	}

	fn setup_history_listener(&self) {
		self.setup_history_listener();
	}

	fn push(&self, path: &str) -> Result<(), SpaRouterError> {
		self.push(path)
			.map_err(|e| SpaRouterError::Inner(e.to_string()))
	}

	fn replace(&self, path: &str) -> Result<(), SpaRouterError> {
		self.replace(path)
			.map_err(|e| SpaRouterError::Inner(e.to_string()))
	}

	fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Result<String, SpaRouterError> {
		reinhardt_urls::routers::ClientRouter::reverse(self, name, params)
			.map_err(|error| SpaRouterError::Inner(error.to_string()))
	}

	fn route_count(&self) -> usize {
		self.route_count()
	}

	#[allow(clippy::type_complexity)] // The boxed listener signature mirrors the trait method; see `SpaRouter::on_navigate_dyn`.
	fn on_navigate_dyn(
		&self,
		listener: Box<dyn Fn(&str, &HashMap<String, String>) + 'static>,
	) -> Box<dyn std::any::Any> {
		Box::new(self.on_navigate(move |path, params| listener(path, params)))
	}

	fn __diag_dispatch_count(&self) -> u64 {
		self.__diag_dispatch_count()
	}

	fn __diag_observer_count(&self) -> usize {
		self.__diag_observer_count()
	}

	fn __diag_router_id(&self) -> usize {
		self.__diag_router_id()
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}
