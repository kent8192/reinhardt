//! URL configuration for auth application
//!
//! Defines unified routes for authentication with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(server)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(server)]
use crate::apps::auth::server::server_fn::{current_user, login, logout, register};

#[cfg(client)]
use crate::apps::auth::client::components::{login_form, register_form};

/// Unified routes for auth application (client + server)
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(server)]
			{
				s.server_fn(login::marker)
					.server_fn(register::marker)
					.server_fn(logout::marker)
					.server_fn(current_user::marker)
			}
			#[cfg(client)]
			s
		})
		// Client-side routes (SPA)
		.client(|c| {
			#[cfg(client)]
			{
				c.route("/login", || login_form())
					.route("/register", || register_form())
			}
			#[cfg(server)]
			c
		})
}
