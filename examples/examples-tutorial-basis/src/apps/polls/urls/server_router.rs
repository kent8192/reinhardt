//! Server-side URL configuration for the polls application.
//!
//! The polls app exposes its dynamic data path through `#[server_fn]`
//! handlers. This router collects the app-local handler inventory.

use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new().auto_server_fns(module_path!())
}
