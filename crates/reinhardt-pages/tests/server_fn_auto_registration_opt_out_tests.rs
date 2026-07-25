#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

use reinhardt_apps::AppModuleRegistration;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRouterExt};
use reinhardt_pages_macros::server_fn;
use reinhardt_urls::routers::ServerRouter;

reinhardt_apps::inventory::submit! {
	AppModuleRegistration::new("opt_out", module_path!())
}

#[server_fn(auto_register = false)]
async fn manually_mounted() -> Result<(), ServerFnError> {
	Ok(())
}

#[test]
fn opt_out_is_omitted_from_inventory_but_remains_explicitly_mountable() {
	let automatic = ServerRouter::new().auto_server_fns(module_path!());
	assert_eq!(automatic.registered_endpoints(), Vec::new());

	let explicit = automatic.server_fn(manually_mounted::marker);
	let endpoints = explicit.registered_endpoints();
	assert_eq!(endpoints.len(), 1);
	assert_eq!(endpoints[0].path, "/api/server_fn/manually_mounted");
}
