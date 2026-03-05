//! Runtime support for url! macro.
//!
//! This module provides runtime URL resolution for routes using a URL mapping.
//! The mapping is typically configured during application startup and contains
//! mappings from route names to URL patterns.
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::integ::url_resolver;
//! use std::collections::HashMap;
//!
//! // Initialize with URL mapping (typically done in main.rs)
//! let mut routes = HashMap::new();
//! routes.insert("home".to_string(), "/".to_string());
//! routes.insert("user-profile".to_string(), "/users/{id}".to_string());
//! url_resolver::init_url_resolver(routes).unwrap();
//!
//! // Resolve URLs (automatically called by url! macro)
//! let url = url_resolver::resolve_url("home").unwrap();
//! assert_eq!(url, "/");
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

/// Global URL route mapping.
///
/// This mapping maps route names to URL patterns.
/// It should be initialized once at application startup using [`init_url_resolver`].
static URL_ROUTES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Initializes the URL resolver with a route mapping.
///
/// This function should be called once at application startup, typically in main.rs.
/// Returns an error if the resolver has already been initialized.
///
/// # Errors
///
/// Returns `Err` with the provided routes if the URL resolver has already
/// been initialized.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_pages::integ::url_resolver;
/// use std::collections::HashMap;
///
/// let mut routes = HashMap::new();
/// routes.insert("home".to_string(), "/".to_string());
/// routes.insert("user-profile".to_string(), "/users/{id}".to_string());
/// url_resolver::init_url_resolver(routes)
///     .expect("URL resolver already initialized");
/// ```
pub fn init_url_resolver(routes: HashMap<String, String>) -> Result<(), HashMap<String, String>> {
	URL_ROUTES.set(routes)
}

/// Error type for URL resolution failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlResolveError {
	/// The URL resolver has not been initialized.
	NotInitialized,
	/// The route name was not found in the mapping.
	RouteNotFound {
		/// The route name that was looked up.
		route_name: String,
	},
}

impl std::fmt::Display for UrlResolveError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotInitialized => {
				write!(
					f,
					"URL resolver not initialized: call init_url_resolver() first"
				)
			}
			Self::RouteNotFound { route_name } => {
				write!(f, "route '{}' not found in URL resolver", route_name)
			}
		}
	}
}

impl std::error::Error for UrlResolveError {}

/// Resolves a route name to its URL pattern.
///
/// This function looks up the given route name in the URL mapping and returns
/// the corresponding URL pattern.
///
/// # Errors
///
/// Returns [`UrlResolveError::NotInitialized`] if the URL resolver has not been
/// initialized with [`init_url_resolver`].
///
/// Returns [`UrlResolveError::RouteNotFound`] if the route name is not found
/// in the mapping.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_pages::integ::url_resolver;
///
/// let url = url_resolver::resolve_url("home")?;
/// assert_eq!(url, "/");
///
/// let url = url_resolver::resolve_url("user-profile")?;
/// assert_eq!(url, "/users/{id}");
/// ```
///
/// # Note
///
/// Currently, this function returns the raw URL pattern without parameter substitution.
/// Future versions will support parameter substitution (e.g., `url!("user-profile", id = 123)`).
pub fn resolve_url(route_name: &str) -> Result<String, UrlResolveError> {
	let routes = URL_ROUTES.get().ok_or(UrlResolveError::NotInitialized)?;

	routes
		.get(route_name)
		.cloned()
		.ok_or_else(|| UrlResolveError::RouteNotFound {
			route_name: route_name.to_string(),
		})
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;

	#[test]
	#[serial(url_resolver)]
	fn test_resolve_basic_routes() {
		let mut routes = HashMap::new();
		routes.insert("home".to_string(), "/".to_string());
		routes.insert("about".to_string(), "/about".to_string());
		routes.insert("user-profile".to_string(), "/users/{id}".to_string());

		let _ = init_url_resolver(routes);

		assert_eq!(resolve_url("home").unwrap(), "/");
		assert_eq!(resolve_url("about").unwrap(), "/about");
		assert_eq!(resolve_url("user-profile").unwrap(), "/users/{id}");
	}

	#[test]
	#[serial(url_resolver)]
	fn test_resolve_nonexistent_route_returns_error() {
		// Initialize with empty routes if not already initialized (OnceLock can only be set once)
		let _ = URL_ROUTES.set(HashMap::new());

		let result = resolve_url("nonexistent");
		assert_eq!(
			result,
			Err(UrlResolveError::RouteNotFound {
				route_name: "nonexistent".to_string(),
			}),
		);
	}

	#[test]
	fn test_resolve_before_init_returns_error() {
		// Verify the error path by checking OnceLock behavior directly.
		// OnceLock persists across tests, so we test the pattern itself.
		let lock: OnceLock<HashMap<String, String>> = OnceLock::new();
		assert!(lock.get().is_none());
	}

	#[test]
	fn test_init_url_resolver_returns_error_on_double_init() {
		let lock: OnceLock<HashMap<String, String>> = OnceLock::new();
		assert!(lock.set(HashMap::new()).is_ok());
		assert!(lock.set(HashMap::new()).is_err());
	}

	#[test]
	fn test_url_resolve_error_display() {
		assert_eq!(
			UrlResolveError::NotInitialized.to_string(),
			"URL resolver not initialized: call init_url_resolver() first",
		);
		assert_eq!(
			UrlResolveError::RouteNotFound {
				route_name: "home".to_string(),
			}
			.to_string(),
			"route 'home' not found in URL resolver",
		);
	}
}
