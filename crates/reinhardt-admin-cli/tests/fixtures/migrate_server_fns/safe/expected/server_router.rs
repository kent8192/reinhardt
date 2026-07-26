use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.with_namespace("polls")
		.auto_server_fns_in_crate(module_path!(), concat!(env!("CARGO_MANIFEST_DIR"), "@", env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), if cfg!(test) { Some(concat!(env!("CARGO_CRATE_NAME"), "@test")) } else if let Some(binary_name) = option_env!("CARGO_BIN_NAME") { Some(binary_name) } else { Some(concat!(env!("CARGO_CRATE_NAME"), "@lib")) })
}
