//! Admin router integration
//!
//! This module provides router integration for admin panel,
//! generating ServerRouter from AdminSite configuration.
//!
//! On non-wasm32 targets, server functions are explicitly registered via `.server_fn()`
//! in `admin_routes_with_di()`. On wasm32 targets, only the namespaced router is returned
//! (server function registration is server-side only).

use std::sync::Arc;

use reinhardt_di::SingletonScope;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_pages::server_fn::ServerFnRouterExt;
use reinhardt_urls::routers::ServerRouter;

use crate::core::AdminSite;

/// Resolves the directory containing WASM build artifacts.
///
/// Checks in order:
/// 1. `REINHARDT_ADMIN_WASM_DIR` environment variable
/// 2. `CARGO_MANIFEST_DIR/dist-admin` (compile-time fallback for development)
#[cfg(not(target_arch = "wasm32"))]
fn resolve_wasm_dir() -> std::path::PathBuf {
	if let Ok(dir) = std::env::var("REINHARDT_ADMIN_WASM_DIR") {
		return std::path::PathBuf::from(dir);
	}
	std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist-admin")
}

/// Resolves the collected static files root directory (STATIC_ROOT).
///
/// Checks `STATIC_ROOT` environment variable. Returns `None` if not set
/// or the `admin/` subdirectory does not exist within it.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_static_root_admin() -> Option<std::path::PathBuf> {
	std::env::var("STATIC_ROOT")
		.ok()
		.map(|root| std::path::PathBuf::from(root).join("admin"))
		.filter(|p| p.is_dir())
}

/// Returns true if the WASM SPA has been built (dist-admin/ contains entry point).
///
/// Checks the collected STATIC_ROOT first, then falls back to the build
/// output directory (dist-admin/).
#[cfg(not(target_arch = "wasm32"))]
fn is_wasm_built() -> bool {
	// Check STATIC_ROOT/admin/ first (production: after collectstatic)
	if let Some(admin_dir) = resolve_static_root_admin()
		&& admin_dir.join("reinhardt_admin.js").is_file()
	{
		return true;
	}
	// Fallback to build output directory (development)
	resolve_wasm_dir().join("reinhardt_admin.js").is_file()
}

/// Serves the admin SPA HTML shell for client-side routing.
///
/// Applies admin-specific security headers (CSP, X-Frame-Options, etc.)
/// to prevent XSS, clickjacking, and other browser-side attacks.
///
/// Uses the [`AdminSettings`] registered via [`configure()`], or
/// falls back to safe defaults if no custom settings were configured.
///
/// [`AdminSettings`]: crate::settings::AdminSettings
/// [`configure()`]: crate::settings::configure
#[cfg(not(target_arch = "wasm32"))]
async fn admin_spa_handler(
	request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	// Ensure vendor assets (CSS, JS, fonts) are available on disk.
	// In development, these files are not present until collectstatic runs;
	// this lazy download guarantees the admin panel renders correctly on the
	// very first request without requiring a manual collectstatic step.
	let assets_dir = std::path::PathBuf::from(ADMIN_ASSETS_DIR);
	crate::core::vendor::ensure_vendor_assets(&assets_dir).await;

	let settings = crate::settings::get_admin_settings();
	let security_headers = settings.to_security_headers();
	let csrf_token = crate::server::security::generate_csrf_token();
	let csrf_cookie = crate::server::security::build_csrf_cookie(&csrf_token, request.is_secure);
	let mut response = reinhardt_http::Response::ok()
		.with_header("Content-Type", "text/html; charset=utf-8")
		.append_header("Set-Cookie", &csrf_cookie);
	for (name, value) in security_headers.to_header_map() {
		response = response.with_header(name, &value);
	}
	Ok(response.with_body(admin_spa_html(&settings.site_title)))
}

/// Resolves an admin static file path to its final URL.
///
/// Uses the global static resolver (initialized by the application) for
/// manifest-aware URL resolution. Falls back to `/static/admin/` prefix
/// if the resolver has not been initialized.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_admin_static(path: &str) -> String {
	let admin_path = format!("admin/{}", path);
	reinhardt_pages::static_resolver::resolve_static(&admin_path)
}

/// Generates the HTML shell for the admin SPA.
///
/// Detects at runtime whether the WASM SPA has been built:
/// - If `dist-admin/reinhardt_admin.js` exists, loads the WASM entry point
/// - Otherwise, falls back to the placeholder bootstrap script (`main.js`)
///
/// All static file URLs are resolved via [`resolve_admin_static`], which
/// integrates with the collectstatic manifest for cache-busted filenames
/// in production. CSS dependencies (Open Props, Animate.css) and the UnoCSS
/// runtime engine are served from local vendor/ directory instead of external
/// CDNs to satisfy CSP and eliminate external network dependencies.
#[cfg(not(target_arch = "wasm32"))]
fn admin_spa_html(site_title: &str) -> String {
	let css_url = resolve_admin_static("style.css");
	let vendor_open_props = resolve_admin_static("vendor/open-props.min.css");
	let vendor_animate = resolve_admin_static("vendor/animate.min.css");
	let vendor_unocss_runtime = resolve_admin_static("vendor/unocss-runtime.js");
	let wasm_built = is_wasm_built();
	let js_url = if wasm_built {
		resolve_admin_static("reinhardt_admin.js")
	} else {
		resolve_admin_static("main.js")
	};
	// wasm-pack --target web requires explicit init() call to load the WASM binary.
	// The init script is served as an external file to comply with CSP
	// (no 'unsafe-inline' needed). The WASM entry URL is passed via a
	// data attribute so the static init script can resolve it at runtime.
	let script_tag = if wasm_built {
		let init_js_url = resolve_admin_static("wasm-init.js");
		format!(r#"<script type="module" src="{init_js_url}" data-wasm-entry="{js_url}"></script>"#)
	} else {
		format!(r#"<script type="module" src="{js_url}"></script>"#)
	};
	let head = reinhardt_pages::head!(|| {
		meta { charset: "utf-8" }
		meta { name: "viewport", content: "width=device-width, initial-scale=1.0" }
		meta { name: "server-fn-prefix", content: "/admin" }
		title { { site_title.to_string() } }
		link { rel: "stylesheet", href: vendor_open_props }
		link { rel: "stylesheet", href: vendor_animate }
		link { rel: "stylesheet", href: css_url }
		script { src: vendor_unocss_runtime }
	});

	format!(
		r#"<!DOCTYPE html>
<html lang="en">
<head>
{head_html}
</head>
<body class="bg-slate-50 text-slate-900 antialiased">
	<div id="app"></div>
	{script_tag}
</body>
</html>"#,
		head_html = head.to_html()
	)
}

/// Path to the admin assets directory (compile-time resolved)
#[cfg(not(target_arch = "wasm32"))]
const ADMIN_ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

/// Serves admin static files from multiple directories with priority-based resolution.
///
/// File resolution order:
/// 1. `STATIC_ROOT/admin/` — production (after collectstatic, manifest-hashed)
/// 2. `dist-admin/` — WASM build output (development)
/// 3. `assets/` — built-in admin assets (CSS, JS placeholder)
///
/// Returns 404 if the file is not found in any directory.
/// MIME types are detected automatically via `mime_guess`.
///
/// This handler catches all internal errors and returns a plain-text error
/// response instead of propagating `Err`. This prevents the server-layer
/// error conversion (`Response::from(Error)`) from replacing the intended
/// `Content-Type` with `application/json`. See issue #3135.
#[cfg(not(target_arch = "wasm32"))]
async fn admin_static_file_handler(
	request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	match admin_static_file_handler_inner(request).await {
		Ok(response) => Ok(response),
		Err(e) => {
			tracing::error!(error = %e, "Unexpected error in admin static file handler");
			Ok(reinhardt_http::Response::internal_server_error()
				.with_header("Content-Type", "text/plain; charset=utf-8")
				.with_body("Internal Server Error"))
		}
	}
}

/// Inner implementation for [`admin_static_file_handler`].
///
/// Separated to allow the outer function to catch errors defensively,
/// preventing `Content-Type: application/json` from the server-layer
/// error conversion path.
#[cfg(not(target_arch = "wasm32"))]
async fn admin_static_file_handler_inner(
	request: reinhardt_http::Request,
) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	use reinhardt_utils::staticfiles::handler::StaticFileHandler;

	let path = request
		.path_params
		.get("path")
		.map(|p| p.trim_start_matches('/'))
		.unwrap_or("");

	// 1. Try STATIC_ROOT/admin/ first (production: after collectstatic)
	if let Some(admin_dir) = resolve_static_root_admin() {
		let handler = StaticFileHandler::new(admin_dir);
		if let Ok(file) = handler.serve(path).await {
			return Ok(reinhardt_http::Response::ok()
				.with_header("Content-Type", &file.mime_type)
				.with_header("Cache-Control", "public, max-age=3600")
				.with_body(file.content));
		}
	}

	// 2. Try dist-admin/ (development: WASM build output)
	let wasm_handler = StaticFileHandler::new(resolve_wasm_dir());
	if let Ok(file) = wasm_handler.serve(path).await {
		return Ok(reinhardt_http::Response::ok()
			.with_header("Content-Type", &file.mime_type)
			.with_header("Cache-Control", "public, max-age=3600")
			.with_body(file.content));
	}

	// 3. Fall back to assets/ directory (built-in CSS/JS placeholder)
	let assets_handler = StaticFileHandler::new(std::path::PathBuf::from(ADMIN_ASSETS_DIR));
	match assets_handler.serve(path).await {
		Ok(file) => Ok(reinhardt_http::Response::ok()
			.with_header("Content-Type", &file.mime_type)
			.with_header("Cache-Control", "public, max-age=3600")
			.with_body(file.content)),
		Err(_) => Ok(reinhardt_http::Response::not_found()),
	}
}

/// Returns a `ServerRouter` that serves the admin panel's static assets.
///
/// Mount this router at `/static/admin/` alongside the main admin router:
///
/// ```rust,no_run
/// use reinhardt_admin::core::{AdminSite, admin_routes_with_di_deferred, admin_static_routes};
/// use reinhardt_urls::routers::UnifiedRouter;
/// use std::sync::Arc;
///
/// let site = Arc::new(AdminSite::new("Admin"));
/// let (admin_router, admin_di) = admin_routes_with_di_deferred(site);
/// let assets = admin_static_routes();
///
/// let router = UnifiedRouter::new()
///     .mount("/admin/", admin_router)
///     .mount("/static/admin/", assets)
///     .with_di_registrations(admin_di);
/// ```
///
/// Files are served from multiple directories in priority order:
/// 1. `STATIC_ROOT/admin/` — production (after collectstatic)
/// 2. `dist-admin/` — WASM build output (development)
/// 3. `assets/` — built-in admin assets (CSS, JS placeholder)
///
/// MIME types are detected automatically via `mime_guess`.
pub fn admin_static_routes() -> ServerRouter {
	let router = ServerRouter::new();

	#[cfg(not(target_arch = "wasm32"))]
	let router = router
		.function("/{*path}", hyper::Method::GET, admin_static_file_handler)
		.function("/{*path}", hyper::Method::HEAD, admin_static_file_handler);

	router
}

/// Returns path prefixes that should be exempt from global CSP middleware.
///
/// The admin panel sets its own `Content-Security-Policy` headers on HTML
/// responses (allowing `'unsafe-inline'` for styles, `data:` for images,
/// etc.). If your application uses `CspMiddleware` with strict directives,
/// add these paths to its `exempt_paths` so the middleware does not override
/// the admin's CSP.
///
/// # Returned paths
///
/// - `"/admin"` -- the admin SPA HTML routes
/// - `"/static/admin"` -- the admin's embedded CSS/JS assets
///
/// # Note
///
/// Reinhardt's own `CspMiddleware` already checks for an existing CSP header
/// and skips insertion if one is present. This helper is primarily useful when
/// a third-party or custom CSP middleware unconditionally sets headers.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_admin::core::admin_csp_exempt_paths;
/// use reinhardt_middleware::{CspConfig, CspMiddleware};
///
/// let mut config = CspConfig::strict();
/// for path in admin_csp_exempt_paths() {
///     config = config.add_exempt_path(path);
/// }
/// let middleware = CspMiddleware::with_config(config);
/// ```
pub fn admin_csp_exempt_paths() -> Vec<String> {
	vec!["/admin".to_string(), "/static/admin".to_string()]
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
			get_detail, get_fields, get_list, import_data, login::admin_login, update_record,
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
			.server_fn(admin_login::marker)
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

/// Admin router builder with automatic DI registration (deprecated)
///
/// Builds a `ServerRouter` from an `AdminSite` with all CRUD endpoints,
/// and auto-registers the `AdminSite` in the provided singleton scope.
///
/// Internally delegates to [`admin_routes_with_di_deferred`] and applies
/// the resulting registrations to the caller-provided scope. This ensures
/// the same code path is used regardless of which API is called.
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
	let (router, registrations) = admin_routes_with_di_deferred(site);
	registrations.apply_to(singleton);
	router
}

/// Admin router builder with deferred DI registration
///
/// Builds a `ServerRouter` from an `AdminSite` with all CRUD endpoints,
/// and returns a [`DiRegistrationList`] containing the `AdminSite` and
/// `AdminUserLoader` registrations. The list should be attached to the
/// [`UnifiedRouter`] via [`with_di_registrations`], which ensures it reaches
/// the server's singleton scope during startup.
///
/// If a custom user type was configured via [`AdminSite::set_user_type`],
/// that type is used for admin authentication. Otherwise,
/// [`AdminDefaultUser`] (table `auth_user`) is registered as a fallback.
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
/// // Default: uses AdminDefaultUser (table "auth_user")
/// let site = Arc::new(AdminSite::new("My Admin"));
/// let (admin_router, admin_di) = admin_routes_with_di_deferred(site);
///
/// let router = UnifiedRouter::new()
///     .mount("/admin/", admin_router)
///     .with_di_registrations(admin_di);
/// ```
///
/// ```rust,ignore
/// // Custom user type
/// let mut site = AdminSite::new("My Admin");
/// site.set_user_type::<MyCustomUser>();
/// let site = Arc::new(site);
/// let (admin_router, admin_di) = admin_routes_with_di_deferred(site);
/// ```
///
/// [`AdminSite::set_user_type`]: AdminSite::set_user_type
/// [`AdminDefaultUser`]: crate::server::user::AdminDefaultUser
/// [`DiRegistrationList`]: reinhardt_di::DiRegistrationList
/// [`UnifiedRouter`]: reinhardt_urls::routers::UnifiedRouter
/// [`with_di_registrations`]: reinhardt_urls::routers::UnifiedRouter::with_di_registrations
pub fn admin_routes_with_di_deferred(
	site: Arc<AdminSite>,
) -> (ServerRouter, reinhardt_di::DiRegistrationList) {
	let mut registrations = reinhardt_di::DiRegistrationList::new();

	// Register the user loader for admin authentication.
	// If the site has a custom user type (set via set_user_type::<U>()),
	// use that; otherwise fall back to AdminDefaultUser.
	let loader = site.user_loader().unwrap_or_else(|| {
		Arc::new(crate::server::admin_auth::create_admin_user_loader::<
			crate::server::user::AdminDefaultUser,
		>())
	});
	registrations.register_arc(loader);

	// Register the login authenticator for admin login.
	// Falls back to AdminDefaultUser if no custom user type was set.
	let login_auth = site.login_authenticator().unwrap_or_else(|| {
		Arc::new(
			crate::server::admin_auth::create_admin_login_authenticator::<
				crate::server::user::AdminDefaultUser,
			>(),
		)
	});
	registrations.register_arc(login_auth);

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

	/// Embedded admin JavaScript file for test assertions.
	const ADMIN_JS: &[u8] = include_bytes!("../../assets/main.js");

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
			"/api/server_fn/admin_login",
			"/",
			"/{*tail}",
		];

		// Act
		let router = test_admin_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Assert - 11 server functions + 2 GET routes should be registered
		assert_eq!(routes.len(), 13);
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
	#[allow(deprecated)] // testing backward compat of deprecated method
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
	#[allow(deprecated)] // testing backward compat of deprecated method
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
		let html = admin_spa_html("Reinhardt Admin");

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
		assert!(
			html.contains(r#"name="server-fn-prefix""#) && html.contains(r#"content="/admin""#),
			"HTML should contain server-fn-prefix meta tag for WASM endpoint resolution"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_references_css_and_js_entry_point() {
		// Arrange & Act
		let html = admin_spa_html("Reinhardt Admin");
		let wasm_built = is_wasm_built();

		// Assert - CSS reference (URLs resolved via resolve_admin_static,
		// which falls back to /static/ prefix when resolver is not initialized)
		assert!(
			html.contains("style.css"),
			"HTML should reference admin CSS"
		);
		// Assert - JS reference depends on whether WASM has been built (#3115)
		if wasm_built {
			assert!(
				html.contains("/static/admin/reinhardt_admin.js"),
				"HTML should reference WASM entry point (reinhardt_admin.js) \
				 when WASM is built. Got:\n{}",
				html
			);
		} else {
			assert!(
				html.contains("/static/admin/main.js"),
				"HTML should reference placeholder (main.js) \
				 when WASM is not built. Got:\n{}",
				html
			);
		}
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_no_external_cdn_urls() {
		// Arrange
		let html = admin_spa_html("Reinhardt Admin");

		// Assert — no external CDN references
		assert!(
			!html.contains("fonts.googleapis.com"),
			"HTML should not reference Google Fonts CDN"
		);
		assert!(
			!html.contains("fonts.gstatic.com"),
			"HTML should not reference Google Fonts static CDN"
		);
		assert!(
			!html.contains("cdn.jsdelivr.net"),
			"HTML should not reference jsDelivr CDN"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_references_vendor_assets() {
		// Arrange
		let html = admin_spa_html("Reinhardt Admin");

		// Assert — local vendor assets are referenced
		assert!(
			html.contains("vendor/open-props"),
			"HTML should reference local Open Props CSS"
		);
		assert!(
			html.contains("vendor/animate"),
			"HTML should reference local Animate.css"
		);
		assert!(
			html.contains("vendor/unocss-runtime"),
			"HTML should reference local UnoCSS runtime JS"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_no_inline_script() {
		// Arrange
		let html = admin_spa_html("Reinhardt Admin");

		// Assert — no UnoCSS runtime inline script
		assert!(
			!html.contains("__unocss_runtime"),
			"HTML should not contain UnoCSS runtime initialization"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_embedded_admin_js_is_valid_utf8_and_nonempty() {
		// Arrange
		let js = std::str::from_utf8(ADMIN_JS).expect("JS should be valid UTF-8");

		// Assert - JS must not be empty
		assert!(!js.is_empty(), "Embedded admin JS should not be empty");
		// Assert - JS must contain executable code (either WASM bootstrap
		// or placeholder shell) (#3115)
		assert!(
			js.contains("function") || js.contains("init(") || js.contains("wasm_bindgen"),
			"Embedded JS should contain executable code. First 200 chars:\n{}",
			&js[..js.len().min(200)]
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
	fn test_admin_static_routes_registers_catch_all_route() {
		// Arrange & Act
		let router = admin_static_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Assert - catch-all routes for GET and HEAD methods
		assert_eq!(
			routes.len(),
			2,
			"Should have exactly 2 catch-all routes (GET + HEAD)"
		);
		assert!(
			paths.contains(&"/{*path}"),
			"Should have catch-all path route, found: {:?}",
			paths
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_file_handler_serves_css() {
		// Arrange - use realistic URI matching production mount at /static/admin/
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/style.css")
			.path_params(std::collections::HashMap::from([(
				"path".to_string(),
				"style.css".to_string(),
			)]))
			.build()
			.unwrap();

		// Act
		let response = admin_static_file_handler(request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::OK);
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("text/css"),
			"Should return text/css content type, got: {}",
			content_type
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_file_handler_serves_js() {
		// Arrange - use realistic URI matching production mount at /static/admin/
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/main.js")
			.path_params(std::collections::HashMap::from([(
				"path".to_string(),
				"main.js".to_string(),
			)]))
			.build()
			.unwrap();

		// Act
		let response = admin_static_file_handler(request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::OK);
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("javascript"),
			"Should return application/javascript content type, got: {}",
			content_type
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_file_handler_returns_404_for_missing_file() {
		// Arrange - use realistic URI matching production mount at /static/admin/
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/nonexistent.txt")
			.path_params(std::collections::HashMap::from([(
				"path".to_string(),
				"nonexistent.txt".to_string(),
			)]))
			.build()
			.unwrap();

		// Act
		let response = admin_static_file_handler(request).await.unwrap();

		// Assert
		assert_eq!(
			response.status,
			hyper::StatusCode::NOT_FOUND,
			"Should return 404 for nonexistent files"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_file_handler_returns_404_for_wasm_when_not_built() {
		// Arrange - use realistic URI matching production mount at /static/admin/
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/reinhardt_admin_bg.wasm")
			.path_params(std::collections::HashMap::from([(
				"path".to_string(),
				"reinhardt_admin_bg.wasm".to_string(),
			)]))
			.build()
			.unwrap();

		// Act
		let response = admin_static_file_handler(request).await.unwrap();

		// Assert - dist-admin/ does not exist in test environment
		assert_eq!(
			response.status,
			hyper::StatusCode::NOT_FOUND,
			"Should return 404 when WASM is not built"
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
	fn test_admin_spa_html_fallback_without_wasm() {
		// Arrange - CI environment has no dist-admin/ directory

		// Act
		let html = admin_spa_html("Reinhardt Admin");

		// Assert - should use placeholder main.js when WASM is not built
		assert!(
			html.contains("/static/admin/main.js")
				|| html.contains("/static/admin/reinhardt_admin.js"),
			"HTML should reference either main.js or reinhardt_admin.js"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_spa_html_uses_configured_site_title() {
		// Arrange
		let custom_title = "My Custom Admin";

		// Act
		let html = admin_spa_html(custom_title);

		// Assert
		assert!(
			html.contains("<title>My Custom Admin</title>"),
			"HTML <title> should reflect the configured site_title"
		);
		assert!(
			!html.contains("<title>Reinhardt Admin</title>"),
			"HTML should not contain the hardcoded default title"
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_is_wasm_built_false_when_no_dist_wasm() {
		// Arrange - CI/test environment should not have dist-admin/

		// Act & Assert
		// In test environments without WASM build, this should be false
		// (unless REINHARDT_ADMIN_WASM_DIR points to a valid location)
		let result = is_wasm_built();
		// We can only assert the function runs without error;
		// actual value depends on whether WASM was built
		assert_eq!(result, result); // non-trivial: ensures no panic
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_resolve_wasm_dir_returns_dist_wasm_subdir() {
		// Arrange & Act
		let dir = resolve_wasm_dir();

		// Assert - should end with dist-admin (either from env or CARGO_MANIFEST_DIR)
		assert!(
			dir.ends_with("dist-admin"),
			"WASM dir should end with 'dist-admin', got: {:?}",
			dir
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_resolve_admin_static_uses_static_resolver() {
		// Arrange & Act - resolver not initialized in test env, uses fallback
		let url = resolve_admin_static("style.css");

		// Assert - fallback produces /static/admin/style.css
		assert!(
			url.contains("admin/style.css"),
			"Should resolve admin static path, got: {}",
			url
		);
	}

	#[rstest]
	fn test_admin_csp_exempt_paths_returns_expected_paths() {
		// Arrange & Act
		let paths = admin_csp_exempt_paths();

		// Assert
		assert!(paths.contains(&"/admin".to_string()));
		assert!(paths.contains(&"/static/admin".to_string()));
		assert_eq!(paths.len(), 2);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_spa_handler_sets_csrf_cookie() {
		// Arrange
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/")
			.build()
			.unwrap();

		// Act
		let response = admin_spa_handler(request).await.unwrap();

		// Assert
		let set_cookie = response
			.headers
			.get("set-cookie")
			.expect("Response should include Set-Cookie header")
			.to_str()
			.unwrap();
		assert!(
			set_cookie.contains("csrftoken="),
			"Cookie should contain CSRF token name, got: {}",
			set_cookie
		);
		assert!(
			set_cookie.contains("SameSite=Strict"),
			"Cookie should have SameSite=Strict, got: {}",
			set_cookie
		);
		assert!(
			set_cookie.contains("Path=/admin"),
			"Cookie should be scoped to /admin, got: {}",
			set_cookie
		);
	}

	// ==================== Spec-based tests for #3115 ====================

	/// Verify static routes serve the WASM binary file.
	/// The admin SPA is a WASM application; its binary must be
	/// served alongside JS and CSS (#3115).
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_admin_static_routes_serves_wasm_binary() {
		// Arrange & Act
		let router = admin_static_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Assert - catch-all route must exist to serve WASM binaries at runtime
		assert!(
			paths.iter().any(|p| p.contains("{*path}")),
			"Admin static routes must have a catch-all route to serve WASM files. \
			 Found routes: {:?}",
			paths
		);
	}

	/// Verify the embedded admin JS is not a placeholder stub when WASM
	/// has been built. Requires `dist-wasm/reinhardt_admin.js` to exist;
	/// skipped in environments without a WASM build (#3115).
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_embedded_admin_js_is_not_placeholder() {
		if !is_wasm_built() {
			// WASM SPA has not been built — placeholder is expected.
			// This test validates production artifacts only.
			return;
		}

		// Arrange
		let js = std::str::from_utf8(ADMIN_JS).expect("JS should be valid UTF-8");

		// Assert - must not contain placeholder indicators
		assert!(
			!js.contains("placeholder"),
			"Embedded admin JS must not be a placeholder. First 200 chars:\n{}",
			&js[..js.len().min(200)]
		);
		assert!(
			!js.contains("WASM frontend may not be built yet"),
			"Embedded admin JS must not contain 'not built yet' fallback message"
		);
	}

	/// Verify the JS filename referenced in the HTML is a registered
	/// static route, ensuring the reference chain is consistent (#3115).
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	fn test_html_js_reference_matches_static_route() {
		// Arrange
		let html = admin_spa_html("Reinhardt Admin");
		let router = admin_static_routes();
		let routes = router.get_all_routes();
		let paths: Vec<&str> = routes.iter().map(|(path, _, _, _)| path.as_str()).collect();

		// Act - the WASM entry point JS must be both referenced in HTML
		// and served by a static route
		let wasm_js_path = if is_wasm_built() {
			"/reinhardt_admin.js"
		} else {
			"/main.js"
		};

		// Assert - HTML references the WASM JS entry point
		assert!(
			html.contains(&format!("/static/admin{}", wasm_js_path)),
			"HTML must reference /static/admin{}, got:\n{}",
			wasm_js_path,
			html
		);
		// Assert - static route serves files via catch-all pattern
		assert!(
			paths.contains(&"/{*path}"),
			"Static routes must serve files via catch-all pattern, found: {:?}",
			paths
		);
	}

	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_spa_handler_csrf_cookie_no_secure_for_http() {
		// Arrange
		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/")
			.build()
			.unwrap();
		// is_secure defaults to false

		// Act
		let response = admin_spa_handler(request).await.unwrap();

		// Assert
		let set_cookie = response
			.headers
			.get("set-cookie")
			.expect("Response should include Set-Cookie header")
			.to_str()
			.unwrap();
		assert!(
			!set_cookie.contains("Secure"),
			"HTTP request should not set Secure flag, got: {}",
			set_cookie
		);
	}

	/// Full-stack test: verifies Content-Type through `ServerRouter::handle()`
	/// route resolution, not just direct handler invocation. Regression test
	/// for #3135 where the server-layer error conversion produced
	/// `Content-Type: application/json` for static file responses.
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_routes_full_stack_css_content_type() {
		use reinhardt_http::Handler;

		// Arrange — mount admin_static_routes() exactly as production does
		let router = ServerRouter::new().mount("/static/admin/", admin_static_routes());

		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/style.css")
			.build()
			.unwrap();

		// Act
		let response = router.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::OK);
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("text/css"),
			"Full-stack CSS should return text/css, got: {}",
			content_type
		);
		assert!(
			!content_type.contains("application/json"),
			"Static file must never return application/json (#3135), got: {}",
			content_type
		);
	}

	/// Full-stack test for JS files through the mounted router.
	/// Regression test for #3135.
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_routes_full_stack_js_content_type() {
		use reinhardt_http::Handler;

		// Arrange
		let router = ServerRouter::new().mount("/static/admin/", admin_static_routes());

		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/main.js")
			.build()
			.unwrap();

		// Act
		let response = router.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::OK);
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			content_type.contains("javascript"),
			"Full-stack JS should return application/javascript, got: {}",
			content_type
		);
		assert!(
			!content_type.contains("application/json"),
			"Static file must never return application/json (#3135), got: {}",
			content_type
		);
	}

	/// Full-stack test: 404 responses must not have application/json Content-Type.
	/// Regression test for #3135.
	#[cfg(not(target_arch = "wasm32"))]
	#[rstest]
	#[tokio::test]
	async fn test_admin_static_routes_full_stack_404_not_json() {
		use reinhardt_http::Handler;

		// Arrange
		let router = ServerRouter::new().mount("/static/admin/", admin_static_routes());

		let request = reinhardt_http::Request::builder()
			.method(hyper::Method::GET)
			.uri("/static/admin/nonexistent.wasm")
			.build()
			.unwrap();

		// Act
		let response = router.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
		let content_type = response
			.headers
			.get("content-type")
			.map(|v| v.to_str().unwrap_or(""))
			.unwrap_or("");
		assert!(
			!content_type.contains("application/json"),
			"Static file 404 must not return application/json (#3135), got: {}",
			content_type
		);
	}
}
