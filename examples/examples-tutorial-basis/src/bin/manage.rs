//! Reinhardt Project Management CLI for examples-tutorial-basis
//!
//! This binary is intentionally native-only. The whole module body is gated
//! behind `not(target_arch = "wasm32")` so that
//! `cargo check --target wasm32-unknown-unknown` on the workspace does not
//! try to compile a tokio-based CLI for the browser target. The wasm side
//! still requires a `main` symbol for `bin` crate-types, so we keep an
//! empty stub.

#[cfg(not(target_arch = "wasm32"))]
mod native {
	// Force-link the parent library so its `#[routes]`
	// `inventory::submit!` registrations (e.g. the
	// `UrlPatternsRegistration` emitted from `config::urls::routes`)
	// survive Rust's dead-code elimination. Without an explicit
	// reference from this binary, the linker drops the library
	// wholesale and `inventory::iter::<UrlPatternsRegistration>()`
	// returns an empty set, which the framework surfaces as
	// "No URL patterns registered" at runtime.
	use examples_tutorial_basis as _;
	use examples_tutorial_basis::config::settings::get_settings;
	#[cfg(feature = "commands-shell")]
	use examples_tutorial_basis::config::shell::get_shell_config;
	use reinhardt::commands::CargoCheckContext;
	#[cfg(not(feature = "commands-shell"))]
	use reinhardt::commands::execute_from_command_line_with_pending_settings_and_cargo_context;
	#[cfg(feature = "commands-shell")]
	use reinhardt::commands::execute_from_command_line_with_pending_settings_and_cargo_context_and_shell;
	use std::path::PathBuf;
	use std::process;

	#[tokio::main]
	pub(super) async fn main() {
		// SAFETY: Called at program start before any spawned tasks.
		unsafe {
			std::env::set_var(
				"REINHARDT_SETTINGS_MODULE",
				"examples_tutorial_basis.config.settings",
			);
		}
		let cargo_context = CargoCheckContext::from_launcher(
			PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
			Some(env!("CARGO_PKG_NAME").to_owned()),
			Some("manage".to_owned()),
		);

		// The `createsuperuser` management command resolves the registered
		// `SuperuserCreator` from the framework's inventory at dispatch
		// time. Since reinhardt-web#4522, any `#[user] + #[model]` struct
		// (including the tutorial's minimal `User`) auto-registers via
		// `inventory::submit!`, so no manual `register_superuser_creator`
		// call is required here.

		#[cfg(feature = "commands-shell")]
		let result = execute_from_command_line_with_pending_settings_and_cargo_context_and_shell(
			get_settings,
			get_shell_config(),
			cargo_context,
		)
		.await;
		#[cfg(not(feature = "commands-shell"))]
		let result = execute_from_command_line_with_pending_settings_and_cargo_context(
			get_settings,
			cargo_context,
		)
		.await;

		if let Err(e) = result {
			eprintln!("Error: {e}");
			process::exit(1);
		}
	}
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	reinhardt::commands::shell_runtime_hook();
	native::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
