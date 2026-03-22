//! Admin router integration
//!
//! This module provides router integration for admin panel,
//! generating ServerRouter from AdminSite configuration.
//!
//! Server functions are explicitly registered via `.server_fn()` in `admin_routes()`.

#[cfg(not(target_arch = "wasm32"))]
use reinhardt_pages::server_fn::ServerFnRouterExt;
use reinhardt_urls::routers::ServerRouter;

use crate::core::AdminSite;
use std::sync::Arc;

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
		// Arrange & Act
		let router = admin_routes();

		// Assert - 10 server functions should be registered
		let routes = router.get_all_routes();
		assert_eq!(routes.len(), 10);
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
}
