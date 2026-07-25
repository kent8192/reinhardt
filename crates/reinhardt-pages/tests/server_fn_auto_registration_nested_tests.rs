#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

mod storefront {
	use reinhardt_apps::AppModuleRegistration;
	use reinhardt_pages::server_fn::ServerFnError;
	use reinhardt_pages_macros::server_fn;

	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("storefront", module_path!())
	}

	#[server_fn(endpoint = "/api/storefront/catalog")]
	async fn catalog() -> Result<(), ServerFnError> {
		Ok(())
	}

	pub(super) fn router() -> reinhardt_urls::routers::ServerRouter {
		use reinhardt_pages::server_fn::ServerFnRouterExt;

		reinhardt_urls::routers::ServerRouter::new().auto_server_fns(module_path!())
	}

	pub(super) mod admin {
		use reinhardt_apps::AppModuleRegistration;
		use reinhardt_pages::server_fn::ServerFnError;
		use reinhardt_pages_macros::server_fn;

		reinhardt_apps::inventory::submit! {
			AppModuleRegistration::new("storefront_admin", module_path!())
		}

		#[server_fn(endpoint = "/api/storefront/admin/audit")]
		async fn audit() -> Result<(), ServerFnError> {
			Ok(())
		}

		pub(crate) fn router() -> reinhardt_urls::routers::ServerRouter {
			use reinhardt_pages::server_fn::ServerFnRouterExt;

			reinhardt_urls::routers::ServerRouter::new().auto_server_fns(module_path!())
		}
	}
}

fn paths(router: reinhardt_urls::routers::ServerRouter) -> Vec<String> {
	router
		.registered_endpoints()
		.into_iter()
		.map(|endpoint| endpoint.path)
		.collect()
}

#[test]
fn auto_registration_uses_the_longest_app_module_owner() {
	assert_eq!(paths(storefront::router()), vec!["/api/storefront/catalog"]);
	assert_eq!(
		paths(storefront::admin::router()),
		vec!["/api/storefront/admin/audit"]
	);
}
