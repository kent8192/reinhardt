//! URL configuration for examples-twitter project
//!
//! This project uses reinhardt-pages with Server Functions for API communication.
//! Each app defines unified routes (server + client) in `urls.rs`, which are mounted here.
//!
//! Admin panel routes are integrated via `AdminSite::get_urls()`, which requires
//! a `DatabaseConnection`. In production, the connection comes from the DI container.

use reinhardt::UnifiedRouter;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt::admin::admin_routes;
#[cfg(server)]
use reinhardt::routes;

// Import app URL modules
use crate::apps::{auth, dm, profile, relationship, tweet};
#[cfg(server)]
use crate::config::admin::configure_admin;
#[cfg(server)]
use crate::config::middleware::{
	create_cache_control_middleware, create_cors_middleware, create_security_middleware,
	create_session_middleware, create_static_files_middleware,
};
#[cfg(server)]
use reinhardt::LoggingMiddleware;

/// Build URL patterns for the application
///
/// This project uses:
/// - Server Functions (`#[server_fn]`) for API communication
/// - Client routing for SPA navigation
/// - Production-ready middleware stack for security and performance
/// - Admin panel mounted at `/admin/` via `AdminSite::get_urls()`
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
#[cfg_attr(server, routes)]
pub fn routes() -> UnifiedRouter {
	// Configure admin site (registration only, no DB needed yet)
	#[cfg(server)]
	let _admin = configure_admin();

	// Admin routes require DatabaseConnection for query execution.
	// In production, mount admin routes like this:
	//
	//   let db = DatabaseConnection::connect("postgres://...").await?;
	//   let admin_router = admin.get_urls(db);
	//   router.mount("/admin", admin_router)
	//
	// For this example, admin is configured but not mounted since
	// get_urls() requires an async DatabaseConnection.

	let router = UnifiedRouter::new()
		// Mount each app's unified routes
		.mount_unified("/", auth::urls::routes())
		.mount_unified("/", tweet::urls::routes())
		.mount_unified("/", profile::urls::routes())
		.mount_unified("/", relationship::urls::routes())
		.mount_unified("/", dm::urls::routes());
	// Mount admin panel routes (server-only, not available on wasm32)
	#[cfg(not(target_arch = "wasm32"))]
	let router = router.mount("/admin/", admin_routes());
	// Apply middleware stack (server-only)
	#[cfg(server)]
	let router = router
		.with_middleware(LoggingMiddleware::new())
		.with_middleware(create_security_middleware())
		.with_middleware(create_cors_middleware())
		.with_middleware(create_session_middleware())
		.with_middleware(create_cache_control_middleware())
		.with_middleware(create_static_files_middleware());
	router
}
