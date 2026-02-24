//! URL configuration for dm application
//!
//! Defines unified routes for direct messaging with both server and client routing.

use reinhardt::UnifiedRouter;

#[cfg(server)]
use reinhardt::pages::server_fn::ServerFnRouterExt;

#[cfg(server)]
use crate::apps::dm::server::server_fn::{
	create_room, get_room, list_messages, list_rooms, mark_as_read, send_message,
};

#[cfg(client)]
use {crate::core::client::pages::dm_chat_page, reinhardt::ClientPath};

/// Unified routes for dm application (client + server)
///
/// Server functions handle REST API access.
/// WebSocket handlers are registered separately through the WebSocket middleware.
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Server-side routes (server functions)
		.server(|s| {
			#[cfg(server)]
			{
				s.server_fn(create_room::marker)
					.server_fn(list_rooms::marker)
					.server_fn(get_room::marker)
					.server_fn(send_message::marker)
					.server_fn(list_messages::marker)
					.server_fn(mark_as_read::marker)
			}
			#[cfg(client)]
			s
		})
		// Client-side routes (SPA)
		.client(|c| {
			#[cfg(client)]
			{
				c.route_path(
					"/dm/{room_id}",
					|ClientPath(room_id): ClientPath<String>| dm_chat_page(room_id),
				)
			}
			#[cfg(server)]
			c
		})
}
