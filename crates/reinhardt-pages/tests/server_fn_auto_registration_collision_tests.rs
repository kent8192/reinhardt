#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

use reinhardt_apps::AppModuleRegistration;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRouterExt};
use reinhardt_pages_macros::server_fn;
use reinhardt_urls::routers::ServerRouter;

reinhardt_apps::inventory::submit! {
	AppModuleRegistration::new("collision", module_path!())
}

fn __reinhardt_auto_register_collision_safe() {}

#[server_fn(endpoint = "/api/collision-safe")]
async fn collision_safe() -> Result<(), ServerFnError> {
	Ok(())
}

#[test]
fn generated_registration_does_not_collide_with_a_sibling_user_item() {
	__reinhardt_auto_register_collision_safe();

	let endpoints = ServerRouter::new()
		.auto_server_fns(module_path!())
		.registered_endpoints();
	assert_eq!(endpoints.len(), 1);
	assert_eq!(endpoints[0].path, "/api/collision-safe");
}
