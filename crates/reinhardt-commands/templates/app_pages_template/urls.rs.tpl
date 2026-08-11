//! URL configuration for the {{ app_name }} application.
//!
#[cfg(client)]
pub mod client_router;

#[cfg(server)]
pub mod grpc_urls;

#[cfg(server)]
pub mod server_router;

#[cfg(server)]
pub mod ws_urls;

#[cfg(client)]
pub use client_router::{client_url_patterns, reverse};

#[cfg(server)]
pub use server_router::server_url_patterns;

use reinhardt::prelude::*;

/// Aggregate the HTTP, WebSocket, gRPC, and client routes for this app.
pub fn url_patterns() -> UnifiedRouter {
	let router = UnifiedRouter::new();

	#[cfg(server)]
	let router = router
		.server(|server| {
			server.mount(
				"/api/{{ app_name }}/",
				server_router::server_url_patterns(),
			)
		})
		.websocket(|websocket| websocket.mount("/ws/", ws_urls::ws_url_patterns()))
		.grpc(|grpc| grpc.merge(grpc_urls::grpc_services()));

	#[cfg(client)]
	let router = router.client(|_| client_router::client_url_patterns());

	router.with_namespace("{{ app_name }}")
}
