//! URL configuration for examples-multi-auth project
//!
//! Demonstrates CompositeAuthentication setup with multiple auth backends:
//! - JWT Bearer token authentication
//! - API Token authentication
//! - Basic HTTP authentication
//! - Session-based authentication (via middleware)

use reinhardt::routes;
use reinhardt::UnifiedRouter;

use crate::apps::admin;
use crate::apps::notes;
use crate::apps::users;

use super::views;

/// Build URL patterns for the application
///
/// Routes are organized by application module:
/// - /health - Health check (no auth required)
/// - /api/auth/* - Authentication endpoints (register, login, token, me, logout)
/// - /api/notes/* - Note CRUD (requires authentication)
/// - /api/admin/* - Admin endpoints (requires admin/staff status)
#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.endpoint(views::health_check)
		.mount("/", users::urls::url_patterns())
		.mount("/", notes::urls::url_patterns())
		.mount("/", admin::urls::url_patterns())
}
