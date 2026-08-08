//! Free-standing imperative navigation entry point.
//!
//! Issue #4610: the form! macro's WASM-side codegen needs an imperative
//! navigation primitive it can splice into the generated `submit()` body
//! without going through a hook (hooks must be called from a reactive
//! context, which the generated `async fn submit(&self)` is not). This free
//! function is a thin wrapper over [`crate::reactive::hooks::RouterHandle`].
//! Generated form code calls
//! `#pages_crate::navigate_or_reload(__url, NavigationType::Push)` from
//! anywhere on WASM; same-origin SPA destinations delegate through this
//! function.
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

#[cfg(any(wasm, test))]
enum BrowserNavigationTarget {
	Spa { path: String, fallback_path: String },
	Hard(String),
}

#[cfg(any(wasm, test))]
fn dispatch_browser_navigation<S, H>(
	target: BrowserNavigationTarget,
	navigation: NavigationType,
	spa_navigate: S,
	hard_navigate: H,
) -> Result<(), NavigateError>
where
	S: FnOnce(&str) -> Result<(), NavigateError>,
	H: FnOnce(&str) -> Result<(), NavigateError>,
{
	if matches!(navigation, NavigationType::Pop | NavigationType::Initial) {
		return Ok(());
	}

	match target {
		BrowserNavigationTarget::Hard(path) => hard_navigate(&path),
		BrowserNavigationTarget::Spa {
			path,
			fallback_path,
		} => match spa_navigate(&path) {
			Err(NavigateError::RouterNotInstalled) => hard_navigate(&fallback_path),
			other => other,
		},
	}
}

#[cfg(wasm)]
fn hard_navigation_error(context: &str, error: impl core::fmt::Debug) -> NavigateError {
	NavigateError::HardNavigationFailed(format!("{context}: {error:?}"))
}

#[cfg(wasm)]
fn is_https_url(path: &str) -> bool {
	path.get(.."https://".len())
		.is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

#[cfg(wasm)]
fn prepare_browser_navigation_target(
	path: String,
) -> Result<BrowserNavigationTarget, NavigateError> {
	let parsed = match web_sys::Url::new(&path) {
		Ok(url) => Some(url),
		Err(error) if is_https_url(&path) => {
			return Err(hard_navigation_error("invalid HTTPS navigation URL", error));
		}
		Err(_) => None,
	};

	let Some(url) = parsed else {
		return Ok(BrowserNavigationTarget::Spa {
			path: path.clone(),
			fallback_path: path,
		});
	};

	let current_origin = web_sys::window()
		.ok_or_else(|| {
			NavigateError::HardNavigationFailed("browser window is unavailable".to_owned())
		})?
		.location()
		.origin()
		.map_err(|error| hard_navigation_error("location.origin failed", error))?;

	if url.origin() != current_origin && url.protocol() == "https:" {
		return Ok(BrowserNavigationTarget::Hard(path));
	}

	if url.origin() == current_origin {
		let spa_path = format!("{}{}{}", url.pathname(), url.search(), url.hash());
		return Ok(BrowserNavigationTarget::Spa {
			path: spa_path,
			fallback_path: path,
		});
	}

	Ok(BrowserNavigationTarget::Spa {
		path: path.clone(),
		fallback_path: path,
	})
}

#[cfg(wasm)]
fn hard_navigate(path: &str) -> Result<(), NavigateError> {
	let window = web_sys::window().ok_or_else(|| {
		NavigateError::HardNavigationFailed("browser window is unavailable".to_owned())
	})?;
	window
		.location()
		.set_href(path)
		.map_err(|error| hard_navigation_error("location.set_href failed", error))
}

/// One-shot navigation that reloads only when no SPA router is installed.
///
/// On browser WASM, a path is first dispatched to the SPA router. Only
/// [`NavigateError::RouterNotInstalled`] triggers a hard navigation through
/// `window.location`. Router rejection, route-resolution, and hard-navigation
/// errors are returned without retrying. Cross-origin HTTPS destinations select
/// hard navigation directly; same-origin absolute URLs are normalized to their
/// path, query, and fragment for SPA navigation. Native and SSR callers never
/// hard-navigate and receive [`NavigateError::RouterNotInstalled`] when no
/// router is installed.
///
/// `NavigationType::Pop` and `NavigationType::Initial` are no-ops before
/// destination classification.
///
/// # Errors
///
/// Returns the SPA or hard-navigation error that applies to the selected
/// destination.
pub fn navigate_or_reload(
	path: impl Into<String>,
	navigation: NavigationType,
) -> Result<(), NavigateError> {
	let path = path.into();
	if matches!(navigation, NavigationType::Pop | NavigationType::Initial) {
		return Ok(());
	}

	#[cfg(wasm)]
	{
		let target = prepare_browser_navigation_target(path)?;
		dispatch_browser_navigation(
			target,
			navigation,
			|spa_path| navigate(spa_path, navigation),
			hard_navigate,
		)
	}

	#[cfg(native)]
	{
		navigate(path, navigation)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::Page;
	use std::cell::Cell;

	#[test]
	fn router_rejected_does_not_invoke_hard_navigation() {
		let _component = Page::text("Rejected");
		let hard_calls = Cell::new(0_u8);
		let result = dispatch_browser_navigation(
			BrowserNavigationTarget::Spa {
				path: "/blocked/".to_owned(),
				fallback_path: "/blocked/".to_owned(),
			},
			NavigationType::Push,
			|_| Err(NavigateError::RouterRejected("blocked".to_owned())),
			|_| {
				hard_calls.set(hard_calls.get() + 1);
				Ok(())
			},
		);

		assert!(matches!(result, Err(NavigateError::RouterRejected(_))));
		assert_eq!(hard_calls.get(), 0);
	}

	#[test]
	fn router_not_installed_invokes_hard_navigation_once() {
		let _component = Page::text("Fallback");
		let hard_calls = Cell::new(0_u8);
		let result = dispatch_browser_navigation(
			BrowserNavigationTarget::Spa {
				path: "/fallback/".to_owned(),
				fallback_path: "https://app.example/fallback/".to_owned(),
			},
			NavigationType::Push,
			|path| {
				assert_eq!(path, "/fallback/");
				Err(NavigateError::RouterNotInstalled)
			},
			|path| {
				hard_calls.set(hard_calls.get() + 1);
				assert_eq!(path, "https://app.example/fallback/");
				Err(NavigateError::HardNavigationFailed("denied".to_owned()))
			},
		);

		assert_eq!(hard_calls.get(), 1);
		assert!(matches!(
			&result,
			Err(NavigateError::HardNavigationFailed(_))
		));
		assert_eq!(
			result.expect_err("fallback should fail").to_string(),
			"hard navigation failed: denied"
		);
	}

	#[test]
	fn hard_navigation_failure_from_spa_is_not_retried() {
		let _component = Page::text("Hard failure");
		let hard_calls = Cell::new(0_u8);
		let result = dispatch_browser_navigation(
			BrowserNavigationTarget::Spa {
				path: "/failed/".to_owned(),
				fallback_path: "/failed/".to_owned(),
			},
			NavigationType::Replace,
			|_| Err(NavigateError::HardNavigationFailed("blocked".to_owned())),
			|_| {
				hard_calls.set(hard_calls.get() + 1);
				Ok(())
			},
		);

		assert!(matches!(
			result,
			Err(NavigateError::HardNavigationFailed(_))
		));
		assert_eq!(hard_calls.get(), 0);
	}

	#[test]
	fn external_https_target_bypasses_spa_navigation() {
		let _component = Page::text("External");
		let spa_calls = Cell::new(0_u8);
		let hard_calls = Cell::new(0_u8);
		let result = dispatch_browser_navigation(
			BrowserNavigationTarget::Hard("https://accounts.example/success".to_owned()),
			NavigationType::Push,
			|_| {
				spa_calls.set(spa_calls.get() + 1);
				Ok(())
			},
			|path| {
				hard_calls.set(hard_calls.get() + 1);
				assert_eq!(path, "https://accounts.example/success");
				Ok(())
			},
		);

		assert!(result.is_ok());
		assert_eq!(spa_calls.get(), 0);
		assert_eq!(hard_calls.get(), 1);
	}

	#[test]
	fn pop_does_not_dispatch_an_external_target() {
		let _component = Page::text("Pop");
		let spa_calls = Cell::new(0_u8);
		let hard_calls = Cell::new(0_u8);
		let result = dispatch_browser_navigation(
			BrowserNavigationTarget::Hard("https://accounts.example/success".to_owned()),
			NavigationType::Pop,
			|_| {
				spa_calls.set(spa_calls.get() + 1);
				Ok(())
			},
			|_| {
				hard_calls.set(hard_calls.get() + 1);
				Ok(())
			},
		);

		assert!(result.is_ok());
		assert_eq!(spa_calls.get(), 0);
		assert_eq!(hard_calls.get(), 0);
	}
}
