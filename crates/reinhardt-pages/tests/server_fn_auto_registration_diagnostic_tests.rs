#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

use reinhardt_pages::server_fn::{ServerFnError, validate_server_fn_inventory};
use reinhardt_pages_macros::server_fn;

mod owned {
	use super::*;
	use reinhardt_apps::AppModuleRegistration;

	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("owned", module_path!())
	}

	pub(super) mod first {
		use super::*;

		#[server_fn(endpoint = "/api/duplicate")]
		async fn duplicate_first() -> Result<(), ServerFnError> {
			Ok(())
		}
	}

	pub(super) mod second {
		use super::*;

		#[server_fn(endpoint = "/api/duplicate")]
		async fn duplicate_second() -> Result<(), ServerFnError> {
			Ok(())
		}
	}
}

mod ambiguous {
	use super::*;
	use reinhardt_apps::AppModuleRegistration;

	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("alpha", module_path!())
	}
	reinhardt_apps::inventory::submit! {
		AppModuleRegistration::new("zeta", module_path!())
	}

	#[server_fn(endpoint = "/api/ambiguous")]
	async fn ambiguous_owner() -> Result<(), ServerFnError> {
		Ok(())
	}
}

mod orphan {
	use super::*;

	#[server_fn(endpoint = "/api/orphan")]
	async fn orphaned() -> Result<(), ServerFnError> {
		Ok(())
	}
}

#[test]
fn inventory_diagnostics_are_exact_and_deterministically_sorted() {
	let crate_module = module_path!();
	let errors = validate_server_fn_inventory();
	let actual = errors.iter().map(ToString::to_string).collect::<Vec<_>>();

	assert_eq!(
		actual,
		vec![
			format!(
				"pages.server_fn.E002: no application owns server function `{crate_module}::orphan` at `/api/orphan`"
			),
			format!(
				"pages.server_fn.E003: multiple applications own module `{crate_module}::ambiguous`: alpha, zeta"
			),
			format!(
				"pages.server_fn.E004: application `owned` has duplicate server function path `/api/duplicate`: {crate_module}::owned::first, {crate_module}::owned::second"
			),
		]
	);
}
