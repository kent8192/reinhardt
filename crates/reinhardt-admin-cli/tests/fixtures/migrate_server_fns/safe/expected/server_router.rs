use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.with_namespace("polls")
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), Some(env!("CARGO_CRATE_NAME")))
}
