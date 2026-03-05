//! URL configuration for examples-twitter project
//!
//! This project uses reinhardt-pages with Server Functions for API communication.
//! Each app defines unified routes (server + client) in `urls.rs`, which are mounted here.

use reinhardt::UnifiedRouter;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt::admin::admin_routes;
use reinhardt::routes;

// Import app URL modules
use crate::apps::{auth, dm, profile, relationship, tweet};
use crate::config::middleware::{
	create_cache_control_middleware, create_cors_middleware, create_security_middleware,
	create_session_middleware, create_static_files_middleware,
};
use reinhardt::LoggingMiddleware;

/// Build URL patterns for the application
///
/// This project uses:
/// - Server Functions (`#[server_fn]`) for API communication
/// - Client routing for SPA navigation
/// - Production-ready middleware stack for security and performance
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
#[routes]
pub fn routes() -> UnifiedRouter {
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
	router
		// Apply middleware stack (order matters for request processing)
		.with_middleware(LoggingMiddleware::new())
		.with_middleware(create_security_middleware())
		.with_middleware(create_cors_middleware())
		.with_middleware(create_session_middleware())
		.with_middleware(create_cache_control_middleware())
		.with_middleware(create_static_files_middleware())
}
