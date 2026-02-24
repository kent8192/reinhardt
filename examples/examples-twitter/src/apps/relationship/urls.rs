//! URL configuration for relationship application
//!
//! Defines unified routes for user relationships (follow/unfollow).

use reinhardt::UnifiedRouter;

#[cfg(server)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(server)]
use crate::apps::relationship::server::server_fn::{
	fetch_followers, fetch_following, follow_user, unfollow_user,
};

/// Unified routes for relationship application (server only)
///
/// This app currently only has server-side routes.
/// Client-side relationship management is handled through profile components.
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(server)]
			{
				s.server_fn(follow_user::marker)
					.server_fn(unfollow_user::marker)
					.server_fn(fetch_followers::marker)
					.server_fn(fetch_following::marker)
			}
			#[cfg(client)]
			s
		})
	// No client-side routes - relationship UI is embedded in profile components
}
