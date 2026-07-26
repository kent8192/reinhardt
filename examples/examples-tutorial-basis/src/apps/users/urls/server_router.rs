//! Server-side URL patterns for the users application.
//!
//! Authentication is exposed via `#[server_fn]` handlers. This router
//! collects the users app's handler inventory.

use reinhardt::ServerRouter;
use reinhardt::pages::server_fn::ServerFnRouterExt;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new().auto_server_fns(module_path!())
}
