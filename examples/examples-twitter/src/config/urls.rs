//! URL configuration for examples-twitter project
//!
//! This project uses reinhardt-pages with Server Functions for API communication.
//! Each app defines unified routes (server + client) in `urls.rs`, which are mounted here.
//!
//! Admin panel routes are integrated via `admin_routes_with_di()`, which
//! captures `AdminSite` DI registration for later application by the server.
//! `AdminDatabase` is lazily constructed from `DatabaseConnection` at first request.

use reinhardt::UnifiedRouter;
#[cfg(native)]
use reinhardt::admin::{admin_routes_with_di, admin_static_routes};
#[cfg(native)]
use reinhardt::routes;

// Import app URL modules
use crate::apps::{auth, dm, profile, relationship, tweet};
#[cfg(native)]
use crate::config::admin::configure_admin;
#[cfg(native)]
use crate::config::middleware::{
	create_cache_control_middleware, create_cors_middleware, create_security_middleware,
	create_session_middleware, create_static_files_middleware,
};
#[cfg(native)]
use reinhardt::LoggingMiddleware;

/// Build URL patterns for the application
///
/// This project uses:
/// - Server Functions (`#[server_fn]`) for API communication
/// - Client routing for SPA navigation
/// - Production-ready middleware stack for security and performance
/// - Admin panel mounted at `/admin/` via `admin_routes_with_di()`
///
/// Admin DI setup:
/// - `AdminSite` registration is deferred via `DiRegistrationList` and
///   applied to the server's singleton scope during startup
/// - `AdminDatabase` is lazily constructed from `DatabaseConnection` at first request
///
/// Middleware stack (in execution order):
/// 1. LoggingMiddleware - Request/response logging
/// 2. SecurityMiddleware - Security headers (HSTS, X-Content-Type-Options)
/// 3. CorsMiddleware - Cross-origin resource sharing
/// 4. SessionMiddleware - Cookie-based session management
/// 5. CacheControlMiddleware - HTTP cache headers
/// 6. StaticFilesMiddleware - Static and media file serving
///
/// Each app's `routes()` function returns a `UnifiedRouter` with both
/// server and client routes defined.
#[cfg_attr(native, routes(standalone))]
pub fn routes() -> UnifiedRouter {
	// Configure admin site (registration only, no DB needed yet)
	#[cfg(native)]
	let admin_site = {
		let site = configure_admin();
		std::sync::Arc::new(site)
	};

	let router = UnifiedRouter::new()
		// Mount each app's unified routes
		.mount_unified("/", auth::urls::routes())
		.mount_unified("/", tweet::urls::routes())
		.mount_unified("/", profile::urls::routes())
		.mount_unified("/", relationship::urls::routes())
		.mount_unified("/", dm::urls::routes());
	// Mount admin panel routes and static assets with deferred DI registration (server-only)
	#[cfg(native)]
	let router = {
		let (admin_router, admin_di) = admin_routes_with_di(admin_site);
		router
			.mount("/admin/", admin_router)
			.mount("/static/admin/", admin_static_routes())
			.with_di_registrations(admin_di)
	};
	// Apply middleware stack (server-only)
	#[cfg(native)]
	let router = router
		.with_middleware(LoggingMiddleware::new())
		.with_middleware(create_security_middleware())
		.with_middleware(create_cors_middleware())
		.with_middleware(create_session_middleware())
		.with_middleware(create_cache_control_middleware())
		.with_middleware(create_static_files_middleware());
	router
}
