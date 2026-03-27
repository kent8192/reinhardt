//! Admin router integration
//!
//! This module provides router integration for admin panel,
//! generating ServerRouter from AdminSite configuration.
//!
//! On non-wasm32 targets, server functions are explicitly registered via `.server_fn()`
//! in `admin_routes_with_di()`. On wasm32 targets, only the namespaced router is returned
//! (server function registration is server-side only).

use reinhardt_di::SingletonScope;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_pages::server_fn::ServerFnRouterExt;
use reinhardt_urls::routers::ServerRouter;

use crate::core::AdminSite;
use std::sync::Arc;

/// Serves the admin SPA HTML shell for client-side routing.
///
/// Applies admin-specific security headers (CSP, X-Frame-Options, etc.)
/// to prevent XSS, clickjacking, and other browser-side attacks.
#[cfg(not(target_arch = "wasm32"))]
async fn admin_spa_handler(
	_request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	let security_headers = crate::server::security::SecurityHeaders::default();
	let mut response =
		reinhardt_http::Response::ok().with_header("Content-Type", "text/html; charset=utf-8");
	for (name, value) in security_headers.to_header_map() {
		response = response.with_header(name, &value);
	}
	Ok(response.with_body(admin_spa_html()))
}

/// Generates the HTML shell for the admin SPA
#[cfg(not(target_arch = "wasm32"))]
fn admin_spa_html() -> String {
	r#"<!DOCTYPE html>
<html lang="en">
<head>
	<meta charset="utf-8" />
	<meta name="viewport" content="width=device-width, initial-scale=1.0" />
	<title>Reinhardt Admin</title>
	<link rel="stylesheet" href="/static/admin/style.css" />
</head>
<body>
	<div id="app"></div>
	<script type="module" src="/static/admin/main.js"></script>
</body>
</html>"#
		.to_string()
}

/// Embedded admin CSS asset (bytes for zero-copy `Bytes::from_static`)
#[cfg(not(target_arch = "wasm32"))]
const ADMIN_CSS: &[u8] = include_bytes!("../../assets/style.css");

/// Embedded admin JS asset (bytes for zero-copy `Bytes::from_static`)
#[cfg(not(target_arch = "wasm32"))]
const ADMIN_JS: &[u8] = include_bytes!("../../assets/main.js");

/// Serves the embedded admin CSS stylesheet
#[cfg(not(target_arch = "wasm32"))]
async fn admin_css_handler(
	_request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	Ok(reinhardt_http::Response::ok()
		.with_header("Content-Type", "text/css; charset=utf-8")
		.with_header("Cache-Control", "public, max-age=3600")
		.with_body(bytes::Bytes::from_static(ADMIN_CSS)))
}

/// Serves the embedded admin JS entry point
#[cfg(not(target_arch = "wasm32"))]
async fn admin_js_handler(
	_request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	Ok(reinhardt_http::Response::ok()
		.with_header("Content-Type", "application/javascript; charset=utf-8")
		.with_header("Cache-Control", "public, max-age=3600")
		.with_body(bytes::Bytes::from_static(ADMIN_JS)))
}

/// Returns a `ServerRouter` that serves the admin panel's static assets.
///
/// Mount this router at `/static/admin/` alongside the main `admin_routes_with_di()`:
///
/// ```rust,no_run
/// use reinhardt_admin::core::{AdminSite, admin_routes_with_di, admin_static_routes};
/// use reinhardt_di::SingletonScope;
/// use std::sync::Arc;
///
/// let site = Arc::new(AdminSite::new("Admin"));
/// let singleton = SingletonScope::new();
/// // Mount admin views and static assets
/// let admin = admin_routes_with_di(site, &singleton);  // mount at /admin/
/// let assets = admin_static_routes();                   // mount at /static/admin/
/// ```
///
/// The admin HTML page references `/static/admin/style.css` and
/// `/static/admin/main.js`. This router serves those embedded assets.
pub fn admin_static_routes() -> ServerRouter {
	let router = ServerRouter::new();

	#[cfg(not(target_arch = "wasm32"))]
	let router = router
		.function("/style.css", hyper::Method::GET, admin_css_handler)
		.function("/main.js", hyper::Method::GET, admin_js_handler);

	router
}

/// Internal route builder shared by `admin_routes_with_di` and the deprecated `admin_routes`.
fn build_admin_router() -> ServerRouter {
	let router = ServerRouter::new().with_namespace("admin");

	// Register all admin server functions on server-side targets.
	// #[server_fn] generates marker structs but does not auto-register routes;
	// explicit .server_fn(marker) calls are required.
	#[cfg(not(target_arch = "wasm32"))]
	let router = {
		use crate::server::{
			bulk_delete_records, create_record, delete_record, export_data, get_dashboard,
			get_detail, get_fields, get_list, import_data, update_record,
		};
		router
			.server_fn(get_dashboard::marker)
			.server_fn(get_list::marker)
			.server_fn(get_detail::marker)
			.server_fn(get_fields::marker)
			.server_fn(create_record::marker)
			.server_fn(update_record::marker)
			.server_fn(delete_record::marker)
			.server_fn(bulk_delete_records::marker)
			.server_fn(export_data::marker)
			.server_fn(import_data::marker)
			.function("/", hyper::Method::GET, admin_spa_handler)
			.function("/{*tail}", hyper::Method::GET, admin_spa_handler)
	};

	router
}

/// Admin router builder (deprecated)
///
/// This function builds a `ServerRouter` with admin endpoints but does **not**
/// register `AdminSite` in the DI singleton scope. As a result, server function
/// handlers that resolve `AdminSite` via `#[inject]` will fail at runtime with
/// `DiError::NotRegistered`.
///
/// Use `admin_routes_with_di()` instead, which accepts an `Arc<AdminSite>` and
/// a `&SingletonScope`, auto-registers the site, and returns a fully functional
/// admin router.
#[deprecated(
	since = "0.1.0-rc.14",
	note = "Does not register AdminSite in the DI scope; server function handlers will fail \
	        at runtime. Use admin_routes_with_di(site, &singleton_scope) instead."
)]
pub fn admin_routes() -> ServerRouter {
	build_admin_router()
}

/// Admin router builder with automatic DI registration
///
/// Builds a `ServerRouter` from an `AdminSite` with all CRUD endpoints,
/// and auto-registers the `AdminSite` in the singleton scope for DI.
///
/// `AdminDatabase` is **not** registered here; it is lazily constructed
/// from `DatabaseConnection` at first request via its `Injectable` impl.
///
/// # Deprecation
///
/// This function registers `AdminSite` in a caller-provided scope, which
/// may not survive past the `routes()` function boundary. Use
/// [`admin_routes_with_di_deferred`] instead, which captures registrations
/// for later application to the server's singleton scope.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_admin::core::{AdminSite, admin_routes_with_di};
/// use reinhardt_di::{SingletonScope, InjectionContext};
/// use std::sync::Arc;
///
/// let site = Arc::new(AdminSite::new("My Admin"));
/// let singleton = Arc::new(SingletonScope::new());
/// let router = admin_routes_with_di(Arc::clone(&site), &singleton);
///
/// let di_ctx = Arc::new(InjectionContext::builder(singleton).build());
/// // Mount router and attach DI context to UnifiedRouter
/// ```
#[deprecated(
	since = "0.1.0-rc.10",
	note = "Use admin_routes_with_di_deferred() which correctly propagates DI registrations to the server's singleton scope"
)]
pub fn admin_routes_with_di(site: Arc<AdminSite>, singleton: &SingletonScope) -> ServerRouter {
	// Auto-register AdminSite in singleton scope for DI resolution
	singleton.set_arc(site);
	build_admin_router()
}

/// Admin router builder with deferred DI registration
///
/// Builds a `ServerRouter` from an `AdminSite` with all CRUD endpoints,
/// and returns a [`DiRegistrationList`] containing the `AdminSite`
/// registration. The list should be attached to the [`UnifiedRouter`] via
/// [`with_di_registrations`], which ensures it reaches the server's
/// singleton scope during startup.
///
/// `AdminDatabase` is **not** registered here; it is lazily constructed
/// from `DatabaseConnection` at first request via its `Injectable` impl.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_admin::core::{AdminSite, admin_routes_with_di_deferred};
/// use reinhardt_urls::routers::UnifiedRouter;
/// use std::sync::Arc;
///
/// let site = Arc::new(AdminSite::new("My Admin"));
/// let (admin_router, admin_di) = admin_routes_with_di_deferred(site);
///
/// let router = UnifiedRouter::new()
///     .mount("/admin/", admin_router)
///     .with_di_registrations(admin_di);
/// ```
///
/// [`DiRegistrationList`]: reinhardt_di::DiRegistrationList
/// [`UnifiedRouter`]: reinhardt_urls::routers::UnifiedRouter
/// [`with_di_registrations`]: reinhardt_urls::routers::UnifiedRouter::with_di_registrations
pub fn admin_routes_with_di_deferred(
	site: Arc<AdminSite>,
) -> (ServerRouter, reinhardt_di::DiRegistrationList) {
	let mut registrations = reinhardt_di::DiRegistrationList::new();
	registrations.register_arc(site);
	(build_admin_router(), registrations)
}

/// Admin router builder (for backward compatibility)
///
/// This struct is kept for backward compatibility with existing code.
/// New code should use `admin_routes()` function directly.
pub struct AdminRouter {
	site: Arc<AdminSite>,
}

impl std::fmt::Debug for AdminRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AdminRouter")
			.field("site_name", &self.site.name())
			.finish()
	}
}

impl AdminRouter {
	/// Create a new admin router builder from Arc-wrapped site
	pub fn from_arc(site: Arc<AdminSite>) -> Self {
		Self { site }
	}

	/// Set favicon from file path
	///
	/// Returns an error if the file cannot be read.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_admin::core::{AdminSite, AdminRouter};
	/// use std::sync::Arc;
	///
	/// let site = Arc::new(AdminSite::new("Admin"));
	/// let router = AdminRouter::from_arc(site)
	///     .with_favicon_file("static/favicon.ico")
	///     .expect("Failed to read favicon file")
	///     .build();
	/// ```
	///
	/// # Errors
	///
	/// Returns `std::io::Error` if the file cannot be read.
	pub fn with_favicon_file(
		self,
		path: impl AsRef<std::path::Path>,
	) -> Result<Self, std::io::Error> {
		let data = std::fs::read(path.as_ref())?;
		self.site.set_favicon(data);
		Ok(self)
	}

	/// Set favicon from raw bytes
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// // Cannot run: favicon.ico file does not exist
	/// use reinhardt_admin::core::{AdminSite, AdminRouter};
	/// use std::sync::Arc;
	///
	/// let favicon_bytes = include_bytes!("favicon.ico").to_vec();
	/// let site = Arc::new(AdminSite::new("Admin"));
	/// let router = AdminRouter::from_arc(site)
	///     .with_favicon_bytes(favicon_bytes)
	///     .build();
	/// ```
	pub fn with_favicon_bytes(self, data: Vec<u8>) -> Self {
		self.site.set_favicon(data);
		self
	}

	/// Build the ServerRouter with all admin endpoints
	///
	/// Generated endpoints:
	/// - `GET /` - Dashboard (list of registered models)
	/// - `GET /favicon.ico` - Favicon
	/// - `GET /{model}/` - List model instances
	/// - `GET /{model}/{id}/` - Get model instance detail
	/// - `POST /{model}/` - Create model instance
	/// - `PUT /{model}/{id}/` - Update model instance
	/// - `DELETE /{model}/{id}/` - Delete model instance
	/// - `POST /{model}/bulk-delete/` - Bulk delete model instances
	/// - `GET /{model}/export/` - Export model data
	/// - `POST /{model}/import/` - Import model data
	///
	/// # Deprecation
	///
	/// Use `admin_routes_with_di()` with `SingletonScope` parameter instead.
	#[deprecated(
		since = "0.1.0-rc.10",
		note = "Use admin_routes_with_di(site, &singleton_scope) instead"
	)]
	#[allow(deprecated)]
	pub fn routes(&self) -> ServerRouter {
		// Create a temporary singleton scope for backward compat
		let singleton = SingletonScope::new();
		admin_routes_with_di(Arc::clone(&self.site), &singleton)
	}

	/// Build the ServerRouter with DI auto-registration
	///
	/// Registers the `AdminSite` in the provided singleton scope
	/// and returns a `ServerRouter` with all admin endpoints.
	///
	/// # Deprecation
	///
	/// Use `admin_routes_with_di_deferred()` which correctly propagates
	/// DI registrations to the server's singleton scope.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_admin::core::{AdminSite, AdminRouter};
	/// use reinhardt_di::SingletonScope;
	/// use std::sync::Arc;
	///
	/// let site = Arc::new(AdminSite::new("Admin"));
	/// let singleton = SingletonScope::new();
	/// let router = AdminRouter::from_arc(site)
	///     .build_with_di(&singleton);
	/// ```
	#[deprecated(
		since = "0.1.0-rc.10",
		note = "Use admin_routes_with_di_deferred() which correctly propagates DI registrations to the server's singleton scope"
	)]
	#[allow(deprecated)]
	pub fn build_with_di(self, singleton: &SingletonScope) -> ServerRouter {
		admin_routes_with_di(self.site, singleton)
	}

	/// Build the ServerRouter (alias for routes())
	///
	/// # Deprecation
	///
	/// Use `build_with_di()` or `admin_routes_with_di()` instead.
	#[deprecated(
		since = "0.1.0-rc.10",
		note = "Use build_with_di(&singleton_scope) or admin_routes_with_di(site, &singleton_scope) instead"
	)]
	#[allow(deprecated)]
	pub fn build(self) -> ServerRouter {
		let singleton = SingletonScope::new();
		admin_routes_with_di(self.site, &singleton)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	/// Helper to create test admin router
	fn test_admin_routes() -> ServerRouter {
		build_admin_router()
	}

	#[rstest]
	fn test_admin_routes_creates_router() {
		// Arrange & Act
		let router = test_admin_routes();

		// Assert
		assert_eq!(router.namespace(), Some("admin"));
	}

	#[rstest]
	#[allow(deprecated)]
	fn test_admin_routes_with_di_auto_registers_site_in_singleton() {
		// Arrange
		let site = Arc::new(AdminSite::new("Auto-Registered Admin"));
		let singleton = SingletonScope::new();

		// Act
		let _router = admin_routes_with_di(Arc::clone(&site), &singleton);

		// Assert - AdminSite should be registered in singleton scope
		let registered = singleton.get::<AdminSite>();
		assert!(
			registered.is_some(),
			"AdminSite should be auto-registered in singleton scope"
		);
		assert_eq!(registered.unwrap().name(), "Auto-Registered Admin");
	}

	#[rstest]
	fn test_admin_routes_with_di_deferred_returns_router_and_registrations() {
		// Arrange
		let site = Arc::new(AdminSite::new("Deferred Admin"));

		// Act
		let (router, registrations) = admin_routes_with_di_deferred(site);

		// Assert - router is valid
		assert_eq!(router.namespace(), Some("admin"));
		// Assert - registrations are non-empty
		assert!(!registrations.is_empty());
	}

	#[rstest]
	fn test_admin_routes_with_di_deferred_applies_site_to_scope() {
		// Arrange
		let site = Arc::new(AdminSite::new("Applied Admin"));
		let scope = SingletonScope::new();

		// Act
		let (_router, registrations) = admin_routes_with_di_deferred(site);
		registrations.apply_to(&scope);

		// Assert - AdminSite should be registered after apply_to
		let registered = scope.get::<AdminSite>();
		assert!(
			registered.is_some(),
			"AdminSite should be registered after apply_to"
		);
		assert_eq!(registered.unwrap().name(), "Applied Admin");
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_routes_registers_all_server_functions() {
		// Arrange
		let expected_paths = [
			"/api/server_fn/get_dashboard",
			"/api/server_fn/get_list",
			"/api/server_fn/get_detail",
			"/api/server_fn/get_fields",
			"/api/server_fn/create_record",
			"/api/server_fn/update_record",
			"/api/server_fn/delete_record",
			"/api/server_fn/bulk_delete_records",
			"/api/server_fn/export_data",
			"/api/server_fn/import_data",
			"/",
			"/{*tail}",
		];

		// Act
		let router = test_admin_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Assert - 10 server functions + 2 GET routes should be registered
		assert_eq!(routes.len(), 12);
		for expected in &expected_paths {
			assert_eq!(
				paths.iter().filter(|p| p == &expected).count(),
				1,
				"expected path {} to be registered exactly once, but found paths: {:?}",
				expected,
				paths
			);
		}
	}

	#[allow(deprecated)] // testing backward compat of deprecated method
	#[rstest]
	fn test_admin_router_backward_compat() {
		// Arrange
		let site = Arc::new(AdminSite::new("Test Admin"));
		let router_builder = AdminRouter::from_arc(site);

		// Act
		let router = router_builder.routes();

		// Assert
		assert_eq!(router.namespace(), Some("admin"));
	}

	#[rstest]
	fn test_admin_router_build_with_di() {
		// Arrange
		let site = Arc::new(AdminSite::new("DI Admin"));
		let singleton = SingletonScope::new();
		let router_builder = AdminRouter::from_arc(site);

		// Act
		let router = router_builder.build_with_di(&singleton);

		// Assert
		assert_eq!(router.namespace(), Some("admin"));
		let registered = singleton.get::<AdminSite>();
		assert!(registered.is_some());
		assert_eq!(registered.unwrap().name(), "DI Admin");
	}

	#[rstest]
	fn test_with_favicon_file_returns_error_for_missing_file() {
		// Arrange
		let site = Arc::new(AdminSite::new("Test Admin"));
		let router_builder = AdminRouter::from_arc(site);

		// Act
		let result = router_builder.with_favicon_file("/nonexistent/path/favicon.ico");

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
	}

	#[rstest]
	fn test_with_favicon_bytes_succeeds() {
		// Arrange
		let site = Arc::new(AdminSite::new("Test Admin"));
		let router_builder = AdminRouter::from_arc(site.clone());
		let favicon_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes

		// Act
		let singleton = SingletonScope::new();
		let router_builder = router_builder.with_favicon_bytes(favicon_data.clone());
		let _router = router_builder.build_with_di(&singleton);

		// Assert
		let stored = site.favicon_data();
		assert!(stored.is_some());
		assert_eq!(stored.unwrap(), favicon_data);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_routes_includes_html_get_routes() {
		// Arrange & Act
		let router = test_admin_routes();
		let routes = router.get_all_routes();

		// Assert - GET routes should be registered
		let get_routes: Vec<_> = routes
			.iter()
			.filter(|(_, _, _, methods)| methods.contains(&hyper::Method::GET))
			.collect();
		assert!(get_routes.len() >= 2, "Should have at least 2 GET routes");

		let paths: Vec<&str> = get_routes
			.iter()
			.map(|(path, _, _, _)| path.as_str())
			.collect();
		assert!(paths.contains(&"/"), "Should have root GET route");
		assert!(
			paths.contains(&"/{*tail}"),
			"Should have catch-all GET route"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_contains_mount_point() {
		// Arrange & Act
		let html = admin_spa_html();

		// Assert
		assert!(
			html.contains(r#"id="app""#),
			"HTML should contain app mount point"
		);
		assert!(
			html.contains("/static/admin/"),
			"HTML should reference admin static files"
		);
		assert!(
			html.contains("<!DOCTYPE html>"),
			"HTML should be valid HTML5"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_references_css_and_js() {
		// Arrange & Act
		let html = admin_spa_html();

		// Assert
		assert!(
			html.contains("/static/admin/style.css"),
			"HTML should reference admin CSS"
		);
		assert!(
			html.contains("/static/admin/main.js"),
			"HTML should reference admin JS"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_embedded_admin_css_is_not_empty() {
		// Arrange
		let css = std::str::from_utf8(ADMIN_CSS).expect("CSS should be valid UTF-8");

		// Assert
		assert!(!css.is_empty(), "Embedded admin CSS should not be empty");
		assert!(
			css.contains("box-sizing"),
			"CSS should contain UnoCSS preflight reset"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_embedded_admin_js_is_not_empty() {
		// Arrange
		let js = std::str::from_utf8(ADMIN_JS).expect("JS should be valid UTF-8");

		// Assert
		assert!(!js.is_empty(), "Embedded admin JS should not be empty");
		assert!(
			js.contains("Reinhardt Admin"),
			"JS should contain admin panel identifier"
		);
	}

	#[rstest]
	fn test_admin_static_routes_creates_router() {
		// Arrange & Act
		let router = admin_static_routes();

		// Assert - should not have namespace (mounted separately)
		assert_eq!(router.namespace(), None);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_static_routes_registers_asset_routes() {
		// Arrange & Act
		let router = admin_static_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Assert
		assert!(
			paths.contains(&"/style.css"),
			"Should serve style.css, found: {:?}",
			paths
		);
		assert!(
			paths.contains(&"/main.js"),
			"Should serve main.js, found: {:?}",
			paths
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_spa_handler_includes_csp_headers() {
		// Arrange
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/")
			.build()
			.unwrap();

		// Act
		let response = admin_spa_handler(request).await.unwrap();

		// Assert
		let headers = response.headers;
		assert!(
			headers.contains_key("content-security-policy"),
			"Response should include CSP header"
		);
		assert!(
			headers.contains_key("x-frame-options"),
			"Response should include X-Frame-Options header"
		);
		assert!(
			headers.contains_key("x-content-type-options"),
			"Response should include X-Content-Type-Options header"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_css_handler_returns_css_content_type() {
		// Arrange
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/style.css")
			.build()
			.unwrap();

		// Act
		let response = admin_css_handler(request).await.unwrap();

		// Assert
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("text/css"),
			"CSS handler should return text/css content type, got: {}",
			content_type
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_js_handler_returns_js_content_type() {
		// Arrange
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/main.js")
			.build()
			.unwrap();

		// Act
		let response = admin_js_handler(request).await.unwrap();

		// Assert
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("javascript"),
			"JS handler should return application/javascript content type, got: {}",
			content_type
		);
	}
}
