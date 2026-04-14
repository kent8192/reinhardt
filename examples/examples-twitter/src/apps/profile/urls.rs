//! URL configuration for profile application
//!
//! Defines unified routes for user profiles with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(native)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(native)]
use crate::apps::profile::shared::server_fn::{fetch_profile, update_profile, update_profile_form};

#[cfg(wasm)]
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
			#[cfg(native)]
			{
				s.server_fn(fetch_profile::marker)
					.server_fn(update_profile::marker)
					.server_fn(update_profile_form::marker)
			}
			#[cfg(wasm)]
			s
		})
		// Client-side routes (SPA) with typed path parameters
		.client(|c| {
			#[cfg(wasm)]
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
			#[cfg(native)]
			c
		})
}
