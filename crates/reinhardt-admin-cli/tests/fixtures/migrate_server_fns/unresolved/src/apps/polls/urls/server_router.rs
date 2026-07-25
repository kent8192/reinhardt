use crate::apps::polls::server_fn::missing as missing_alias;
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.server_fn(missing_alias::marker)
}
