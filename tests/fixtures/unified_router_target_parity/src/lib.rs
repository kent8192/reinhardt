use reinhardt::{ClientRouter, ServerRouter, UnifiedRouter};

fn configure_server_routes(router: ServerRouter) -> ServerRouter {
	router
}

fn configure_client_routes(router: ClientRouter) -> ClientRouter {
	router
}

fn auth_url_patterns() -> UnifiedRouter {
	let router = UnifiedRouter::new().client(configure_client_routes);

	#[cfg(server)]
	let router = router.server(configure_server_routes);

	router
}

fn dashboard_url_patterns() -> UnifiedRouter {
	UnifiedRouter::new().client(configure_client_routes)
}

pub fn url_patterns() -> UnifiedRouter {
	UnifiedRouter::new()
		.client(configure_client_routes)
		.mount_unified("/", auth_url_patterns())
		.mount_unified("/dashboard/", dashboard_url_patterns())
}

#[cfg(client)]
pub fn client_url_patterns() -> UnifiedRouter {
	url_patterns()
}
