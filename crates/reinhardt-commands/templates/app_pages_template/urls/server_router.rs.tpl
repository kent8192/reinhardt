//! Server-side URL configuration for the {{ app_name }} application.
//!
//! Per-app routers are NOT aggregated automatically. Endpoints added here
//! become reachable only after `config/urls.rs` aggregates
//! `crate::apps::{{ app_name }}::urls::server_url_patterns()`.
//!
//! Server functions declared by this app are collected automatically.

use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new().auto_server_fns_in_crate(
		module_path!(),
		concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")),
		option_env!("CARGO_BIN_NAME"),
	)
}
