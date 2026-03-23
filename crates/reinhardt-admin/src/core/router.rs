//! Admin router integration
//!
//! This module provides router integration for admin panel,
//! generating ServerRouter from AdminSite configuration.
//!
//! On non-wasm32 targets, server functions are explicitly registered via `.server_fn()`
//! in `admin_routes()`. On wasm32 targets, only the namespaced router is returned
//! (server function registration is server-side only).

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

/// Admin router builder
///
/// Builds a ServerRouter from an AdminSite with all CRUD endpoints.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_admin::core::{AdminSite, admin_routes};
///
/// let site = AdminSite::new("My Admin");
/// // ... register models ...
///
/// let router = admin_routes();
/// ```
pub fn admin_routes() -> ServerRouter {
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
	pub fn routes(&self) -> ServerRouter {
		admin_routes()
	}

	/// Build the ServerRouter (alias for routes())
	pub fn build(self) -> ServerRouter {
		admin_routes()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_admin_routes_creates_router() {
		// Arrange & Act
		let router = admin_routes();

		// Assert
		assert_eq!(router.namespace(), Some("admin"));
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
		let router = admin_routes();
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
		let router_builder = router_builder.with_favicon_bytes(favicon_data.clone());
		let _router = router_builder.build();

		// Assert
		let stored = site.favicon_data();
		assert!(stored.is_some());
		assert_eq!(stored.unwrap(), favicon_data);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_routes_includes_html_get_routes() {
		// Arrange & Act
		let router = admin_routes();
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
}
