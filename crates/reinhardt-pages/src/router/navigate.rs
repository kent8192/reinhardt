//! Free-standing imperative navigation entry point.
//!
//! Issue #4610: the form! macro's WASM-side codegen needs an imperative
//! navigation primitive it can splice into the generated `submit()` body
//! without going through a hook (hooks must be called from a reactive
//! context, which the generated `async fn submit(&self)` is not). This free
//! function is a thin wrapper over [`crate::reactive::hooks::RouterHandle`]
//! so the macro can call `#pages_crate::navigate(__url, NavigationType::Push)`
//! from anywhere on wasm.
//!
//! Outside the macro, prefer [`crate::reactive::hooks::use_router`] from
//! component bodies so the call site documents that it expects an SPA
//! context.

use crate::app::try_with_spa_router;
use crate::reactive::hooks::router::{NavigateError, RouterHandle};
use crate::router::NavigationType;
use core::fmt::Display;

/// One-shot imperative SPA navigation.
///
/// Equivalent to `use_router().navigate(path, nav)` — see
/// [`crate::reactive::hooks::use_router`] for the hook form.
///
/// # Errors
///
/// - `Err(NavigateError::RouterNotInstalled)` — `ClientLauncher::launch()`
///   has not installed an SPA router on the current thread. The form!
///   macro's WASM-side codegen uses this discriminant to fall back to a
///   hard navigation; component / hook callers SHOULD treat it as a
///   programmer error.
/// - `Err(NavigateError::RouterRejected(_))` — the installed router
///   rejected the navigation (e.g. unknown route, invalid path). The
///   inner string is the router's error message, suitable for logging
///   but not for direct user display.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::{navigate, router::NavigationType};
///
/// let _ = navigate("/welcome", NavigationType::Push);
/// ```
pub fn navigate(path: impl Into<String>, nav: NavigationType) -> Result<(), NavigateError> {
	RouterHandle.navigate(path, nav)
}

/// One-shot named-route SPA navigation.
///
/// The route must be registered on the active SPA router. Pass homogeneous
/// parameter arrays directly, or use [`crate::route_params!`] for mixed
/// [`Display`] values. `NavigationType::Pop` and `NavigationType::Initial`
/// are accepted as no-ops. This function never performs a hard reload.
///
/// # Errors
///
/// - [`NavigateError::RouterNotInstalled`] when no SPA router is active.
/// - [`NavigateError::RouteResolutionFailed`] when the route name or its
///   parameters cannot be reversed.
/// - [`NavigateError::RouterRejected`] when the active router rejects the
///   resolved path.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_pages::{NavigationType, navigate_named};
///
/// let _ = navigate_named("project-settings", [("project_id", 7_i64)], NavigationType::Push);
/// ```
///
/// ```ignore
/// use reinhardt_pages::{NavigationType, navigate_named, route_params};
///
/// let _ = navigate_named(
///     "workspace-document",
///     route_params! {
///         "workspace_id" => 42_i64,
///         "slug" => "draft",
///     },
///     NavigationType::Push,
/// );
/// ```
pub fn navigate_named<I, K, V>(
	name: &str,
	params: I,
	navigation: NavigationType,
) -> Result<(), NavigateError>
where
	I: IntoIterator<Item = (K, V)>,
	K: AsRef<str>,
	V: Display,
{
	if matches!(navigation, NavigationType::Pop | NavigationType::Initial) {
		return Ok(());
	}

	let owned_params = params
		.into_iter()
		.map(|(key, value)| (key.as_ref().to_owned(), value.to_string()))
		.collect::<Vec<_>>();
	let borrowed_params = owned_params
		.iter()
		.map(|(key, value)| (key.as_str(), value.as_str()))
		.collect::<Vec<_>>();

	let path = try_with_spa_router(|router| router.reverse(name, borrowed_params.as_slice()))
		.ok_or(NavigateError::RouterNotInstalled)?
		.map_err(|error| NavigateError::RouteResolutionFailed(error.to_string()))?;

	navigate(path, navigation)
}
