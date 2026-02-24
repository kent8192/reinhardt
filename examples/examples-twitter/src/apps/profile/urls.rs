//! URL configuration for profile application
//!
//! Defines unified routes for user profiles with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(server)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(server)]
use crate::apps::profile::server::server_fn::{fetch_profile, update_profile, update_profile_form};

#[cfg(client)]
use {
	crate::core::client::pages::{profile_edit_page, profile_page},
	reinhardt::ClientPath,
	uuid::Uuid,
};

/// Unified routes for profile application (client + server)
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(server)]
			{
				s.server_fn(fetch_profile::marker)
					.server_fn(update_profile::marker)
					.server_fn(update_profile_form::marker)
			}
			#[cfg(client)]
			s
		})
		// Client-side routes (SPA) with typed path parameters
		.client(|c| {
			#[cfg(client)]
			{
				c.route_path(
					"/profile/{user_id}/edit",
					|ClientPath(user_id): ClientPath<Uuid>| profile_edit_page(user_id),
				)
				.route_path(
					"/profile/{user_id}",
					|ClientPath(user_id): ClientPath<Uuid>| profile_page(user_id),
				)
			}
			#[cfg(server)]
			c
		})
}
