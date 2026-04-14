//! URL configuration for tweet application
//!
//! Defines unified routes for tweets with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(native)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(native)]
use crate::apps::tweet::shared::server_fn::{create_tweet, delete_tweet, list_tweets};

#[cfg(wasm)]
use crate::core::client::pages::{home_page, timeline_page};

/// Unified routes for tweet application (client + server)
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(native)]
			{
				s.server_fn(create_tweet::marker)
					.server_fn(list_tweets::marker)
					.server_fn(delete_tweet::marker)
			}
			#[cfg(wasm)]
			s
		})
		// Client-side routes (SPA)
		.client(|c| {
			#[cfg(wasm)]
			{
				c.route("/", || home_page())
					.route("/timeline", || timeline_page())
			}
			#[cfg(native)]
			c
		})
}
