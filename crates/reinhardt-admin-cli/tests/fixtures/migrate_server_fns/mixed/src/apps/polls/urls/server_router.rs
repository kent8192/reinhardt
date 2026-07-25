use crate::apps::polls::server_fn::{automatic, manual};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(automatic::marker)
		.server_fn(manual::marker)
}
