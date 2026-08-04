//! URL configuration for examples-tutorial-basis project
//!
//! The `routes` function defines the top-level project router.
//!
//! Middleware stack (server-only):
//! 1. `SessionMiddleware` — cookie-based session management used by the
//!    `users` app's login/logout server functions and `CurrentUser` auth state

use crate::apps::{polls::urls as polls_urls, users::urls as users_urls};
use reinhardt::UnifiedRouter;
#[cfg(server)]
use reinhardt::admin::{admin_routes_with_di, admin_static_routes};
use reinhardt::routes;

#[cfg(server)]
use crate::config::admin::configure_admin;

#[cfg(server)]
use reinhardt::middleware::session::{SessionConfig, SessionMiddleware};
#[cfg(server)]
use std::{sync::Arc, time::Duration};

/// Build the session middleware with a two-week TTL and Lax SameSite.
///
/// Uses the production-oriented defaults shared by the tutorial examples.
#[cfg(server)]
fn create_session_middleware() -> SessionMiddleware {
	let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(1_209_600))
		.with_http_only(true)
		.with_same_site("Lax".to_string())
		.with_path("/".to_string());
	SessionMiddleware::new(config)
}

/// Build the top-level project router.
///
/// The `#[routes]` macro registers this function for automatic discovery
/// via `inventory::submit!(UrlPatternsRegistration)` and emits a linker
/// marker to enforce single-usage.
///
/// This function aggregates app-level server routers, client routers,
/// the admin panel, and the session middleware for their respective targets.
#[routes]
pub fn routes() -> UnifiedRouter {
	let router = UnifiedRouter::new();

	// Each app owns its server-function marker registration in its own
	// `urls` module. The project router only aggregates app routers.
	#[cfg(server)]
	let router = router.server(|s| {
		s.mount("/", polls_urls::server_url_patterns())
			.mount("/", users_urls::server_url_patterns())
	});

	// Aggregate every app's client routes for the WASM SPA.
	// Each `client_url_patterns()` already namespaces its routes
	// (`polls:` / `users:`). We compose them by wrapping each in a single-purpose
	// `UnifiedRouter` and stitching with `mount_unified`, which uses
	// `ClientRouter::merge` internally.
	#[cfg(client)]
	let router = router
		.mount_unified(
			"/",
			UnifiedRouter::new().client(|_| polls_urls::client_url_patterns()),
		)
		.mount_unified(
			"/",
			UnifiedRouter::new().client(|_| users_urls::client_url_patterns()),
		);

	// Mount the auto-generated admin panel at /admin/ (server-only).
	// `admin_routes_with_di` returns both the router and a DI registration
	// list that lazily provides `AdminDatabase` to admin handlers from the
	// project's `DatabaseConnection`.
	#[cfg(server)]
	let router = {
		let admin_site = Arc::new(configure_admin());
		let (admin_router, admin_di) = admin_routes_with_di(admin_site);
		router
			.mount("/admin/", admin_router)
			.mount("/static/admin/", admin_static_routes())
			.with_di_registrations(admin_di)
	};

	// `SessionMiddleware` auto-registers its `SessionStore` as a DI singleton
	// via `Middleware::di_registrations` (keyed by `TypeId::of::<SessionStore>()`
	// post-#4437), so server functions that
	// `#[inject] session: SessionData` or
	// `#[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>`
	// can resolve the same store the middleware writes to without a parallel
	// `with_di_registrations(...)` call. The same middleware also derives
	// `AuthState` from `USER_ID_SESSION_KEY`, so authenticated handlers can use
	// `CurrentUser<U>` without adding a second cookie-session auth layer.
	// See #4426 (and the original #4423 regression that motivated the
	// auto-registration hook) and #4740.
	#[cfg(server)]
	let router = router.with_middleware(create_session_middleware());

	router
}
