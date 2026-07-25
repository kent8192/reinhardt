use crate::apps::polls::server_fn::{get_questions, vote};
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.with_namespace("polls")
		.server_fn(get_questions::marker)
		.server_fn(vote::marker)
}
