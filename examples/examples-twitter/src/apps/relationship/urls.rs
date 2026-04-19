//! URL configuration for relationship application
//!
//! Defines unified routes for user relationships (follow/unfollow).

use reinhardt::UnifiedRouter;

#[cfg(native)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(native)]
use crate::apps::relationship::shared::server_fn::{
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
			#[cfg(native)]
			{
				s.server_fn(follow_user::marker)
					.server_fn(unfollow_user::marker)
					.server_fn(fetch_followers::marker)
					.server_fn(fetch_following::marker)
			}
			#[cfg(wasm)]
			s
		})
	// No client-side routes - relationship UI is embedded in profile components
}
