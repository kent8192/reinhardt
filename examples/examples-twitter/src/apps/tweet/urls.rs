//! URL configuration for tweet application
//!
//! Defines unified routes for tweets with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(server)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(server)]
use crate::apps::tweet::server::server_fn::{create_tweet, delete_tweet, list_tweets};

#[cfg(client)]
use crate::core::client::pages::{home_page, timeline_page};

/// Unified routes for tweet application (client + server)
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(server)]
			{
				s.server_fn(create_tweet::marker)
					.server_fn(list_tweets::marker)
					.server_fn(delete_tweet::marker)
			}
			#[cfg(client)]
			s
		})
		// Client-side routes (SPA)
		.client(|c| {
			#[cfg(client)]
			{
				c.route("/", || home_page())
					.route("/timeline", || timeline_page())
			}
			#[cfg(server)]
			c
		})
}
