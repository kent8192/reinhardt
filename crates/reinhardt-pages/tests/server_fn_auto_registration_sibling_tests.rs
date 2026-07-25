#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

mod polls {
	use reinhardt_apps::AppModuleRegistration;
	use reinhardt_pages::server_fn::ServerFnError;
	use reinhardt_pages_macros::server_fn;

	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("polls", module_path!())
	}

	#[server_fn(endpoint = "/api/polls/vote")]
	async fn vote() -> Result<(), ServerFnError> {
		Ok(())
	}

	pub(super) fn router() -> reinhardt_urls::routers::ServerRouter {
		use reinhardt_pages::server_fn::ServerFnRouterExt;

		reinhardt_urls::routers::ServerRouter::new().auto_server_fns(module_path!())
	}
}

mod users {
	use reinhardt_apps::AppModuleRegistration;
	use reinhardt_pages::server_fn::ServerFnError;
	use reinhardt_pages_macros::server_fn;

	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("users", module_path!())
	}

	#[server_fn(endpoint = "/api/users/deactivate")]
	async fn deactivate() -> Result<(), ServerFnError> {
		Ok(())
	}
}

#[test]
fn auto_registration_selects_only_the_callers_sibling_app() {
	let paths = polls::router()
		.registered_endpoints()
		.into_iter()
		.map(|endpoint| endpoint.path)
		.collect::<Vec<_>>();

	assert_eq!(paths, vec!["/api/polls/vote"]);
}
