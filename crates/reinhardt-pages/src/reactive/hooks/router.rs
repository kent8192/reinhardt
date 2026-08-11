//! Imperative router hook (`use_router`).
//!
//! Issue #4610: provides a hook-based API for programmatic SPA navigation
//! from components and reactive contexts, dispatching through the
//! [`reinhardt_urls::routers::ClientRouter`] installed by
//! `ClientLauncher::router_client`.
//!
//! `use_router()` returns a zero-sized [`RouterHandle`] that dispatches into
//! whichever router builder the application picked, by re-entering the
//! `APP_ROUTER` thread-local on every call. Using a handle rather than a
//! cloned `Rc<Router>` matches the shape of `use_state` / `use_effect`
//! (zero-cost wrapper, no strong reference bookkeeping) and avoids pinning
//! the router for the lifetime of any captured closure.
//!
//! See also the free [`crate::router::navigate`] function for one-shot
//! navigation calls outside a hook context.

use crate::app::NavigationIntent;
use crate::app::try_with_navigation_coordinator;
use crate::app::try_with_spa_router;
use crate::router::NavigationType;

/// Public navigation error returned by [`RouterHandle::push`],
/// [`RouterHandle::replace`], and [`RouterHandle::navigate`].
///
/// The inner SPA router translates its concrete error
/// (`reinhardt_urls::routers::client_router::error::RouterError`) into a
/// stringly-typed message so the public API does not couple to the
/// concrete error enum.
#[derive(Debug)]
#[non_exhaustive]
pub enum NavigateError {
	/// `ClientLauncher::launch()` has not installed an SPA router on the
	/// current thread, so SPA navigation is impossible.
	///
	/// Callers can use this discriminant to choose an appropriate fallback.
	RouterNotInstalled,
	/// Route reversal failed before attempting SPA navigation.
	///
	/// The string is the inner router's `Display` representation.
	RouteResolutionFailed(String),
	/// The underlying SPA router rejected the navigation.
	///
	/// The string is the inner router's `Display` representation.
	RouterRejected(String),
	/// A browser hard-navigation fallback failed.
	///
	/// The string describes the browser API failure.
	HardNavigationFailed(String),
}

impl core::fmt::Display for NavigateError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::RouterNotInstalled => write!(f, "SPA router not installed"),
			Self::RouteResolutionFailed(message) => {
				write!(f, "route resolution failed: {message}")
			}
			Self::RouterRejected(message) => {
				write!(f, "router rejected navigation: {message}")
			}
			Self::HardNavigationFailed(message) => {
				write!(f, "hard navigation failed: {message}")
			}
		}
	}
}

impl std::error::Error for NavigateError {}

/// Imperative navigation handle returned by [`use_router`].
///
/// Zero-sized: every method re-enters the `APP_ROUTER` thread-local rather
/// than caching a strong reference to the underlying router. That keeps
/// captured closures free of router-lifetime bookkeeping and lets the
/// launcher swap the router out at teardown without dangling clones.
///
/// # Behaviour when no router is installed
///
/// All methods return `Err(NavigateError::RouterNotInstalled)` when
/// `ClientLauncher::launch()` has not installed an SPA router on the
/// current thread. The form! macro's WASM-side codegen relies on this
/// fallible shape to fall back to a hard `location.set_href` when a form
/// is rendered outside `ClientLauncher::launch`. Callers from a normal
/// component / hook context SHOULD treat the variant as a programmer
/// error (the hook was called outside a mounted SPA).
#[derive(Clone, Copy, Debug)]
pub struct RouterHandle;

impl RouterHandle {
	/// Pushes a new entry onto the browser history stack and dispatches
	/// the SPA router's navigation observers.
	///
	/// This is the SPA equivalent of setting `window.location.href`: the
	/// route changes, registered observers fire, and the matching route
	/// component re-renders — all without a document reload.
	pub fn push(&self, path: impl Into<String>) -> Result<(), NavigateError> {
		let path = path.into();
		if let Some(result) = try_with_navigation_coordinator(|coordinator| {
			coordinator.navigate(path.clone(), NavigationIntent::Push)
		}) {
			return result;
		}
		try_with_spa_router(|router| router.push(&path))
			.ok_or(NavigateError::RouterNotInstalled)?
			.map_err(|e| NavigateError::RouterRejected(e.to_string()))
	}

	/// Replaces the current browser history entry and dispatches the SPA
	/// router's navigation observers.
	///
	/// Useful when the new path should not appear as a separate entry in
	/// the user's back/forward history (e.g. post-login redirect).
	pub fn replace(&self, path: impl Into<String>) -> Result<(), NavigateError> {
		let path = path.into();
		if let Some(result) = try_with_navigation_coordinator(|coordinator| {
			coordinator.navigate(path.clone(), NavigationIntent::Replace)
		}) {
			return result;
		}
		try_with_spa_router(|router| router.replace(&path))
			.ok_or(NavigateError::RouterNotInstalled)?
			.map_err(|e| NavigateError::RouterRejected(e.to_string()))
	}

	/// Dispatches navigation based on the supplied [`NavigationType`].
	///
	/// `NavigationType::Push` and `NavigationType::Replace` delegate to
	/// [`Self::push`] and [`Self::replace`] respectively. `Pop` and
	/// `Initial` are interpreted as "do not modify history" — they are
	/// produced by the browser itself (popstate / first paint) and are
	/// accepted here as no-ops so callers can pass through a value
	/// received from a navigation observer without filtering.
	pub fn navigate(
		&self,
		path: impl Into<String>,
		nav: NavigationType,
	) -> Result<(), NavigateError> {
		match nav {
			NavigationType::Push => self.push(path),
			NavigationType::Replace => self.replace(path),
			// `Pop` and `Initial` are browser-originated events; the
			// imperative API has nothing to do for them.
			NavigationType::Pop | NavigationType::Initial => Ok(()),
		}
	}
}

/// Returns a [`RouterHandle`] for imperative navigation from the current
/// component or reactive context.
///
/// The handle itself is infallible to obtain; navigation methods on
/// [`RouterHandle`] (`push`, `replace`, `navigate`) return a
/// `Result<(), NavigateError>` whose variants document the fallible
/// cases. In particular, `Err(NavigateError::RouterNotInstalled)` is
/// returned when `ClientLauncher::launch()` has not installed an SPA
/// router on the current thread — the form! macro's WASM-side codegen
/// uses this discriminant to fall back to a hard navigation, while
/// component / hook callers SHOULD treat it as a programmer error.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::{
///     deps,
///     reactive::hooks::{use_retained_effect, use_router, use_state},
/// };
///
/// // Navigate to `/welcome` once `should_redirect` flips to `true`.
/// let (should_redirect, _set) = use_state(false);
/// let router = use_router();
/// use_retained_effect(
///     {
///         let should_redirect = should_redirect.clone();
///         move || {
///             if should_redirect.get() {
///                 let _ = router.push("/welcome");
///             }
///             None::<fn()>
///         }
///     },
///     deps![should_redirect.clone()],
/// );
/// ```
pub fn use_router() -> RouterHandle {
	RouterHandle
}
