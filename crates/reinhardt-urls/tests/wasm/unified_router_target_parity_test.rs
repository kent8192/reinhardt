//! WASM regression coverage for the target-neutral UnifiedRouter contract.

#![cfg(all(target_arch = "wasm32", feature = "wasm-diag-test"))]

use reinhardt_core::page::Page;
use reinhardt_urls::routers::UnifiedRouter;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn wasm_server_builder_is_inert_and_client_composition_remains_active() {
	let child = UnifiedRouter::new()
		.client(|client| client.route("login", "/login/", || Page::Empty))
		.with_namespace("auth");

	let client = UnifiedRouter::new()
		.server(|_| panic!("WASM server builder must not be invoked"))
		.client(|client| client.route("home", "/", || Page::Empty))
		.mount_unified("/ignored-native-prefix/", child)
		.into_client();

	assert_eq!(client.route_count(), 2);
	assert_eq!(client.reverse("home", &[]).unwrap(), "/");
	assert_eq!(client.reverse("auth:login", &[]).unwrap(), "/login/");
}
