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
	ServerRouter::new().auto_server_fns(module_path!())
}
