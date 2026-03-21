//! Reinhardt Project Management CLI for examples-tutorial-basis

// Server-side implementation
#[cfg(server)]
mod server {
	use examples_tutorial_basis::config;
	use reinhardt::commands::execute_from_command_line;
	use reinhardt::core::tokio;
	use std::process;

	#[tokio::main]
	pub(crate) async fn main() {
		// Set settings module environment variable
		// SAFETY: This is safe because we're setting it before any other code runs
		unsafe {
			std::env::set_var(
				"REINHARDT_SETTINGS_MODULE",
				"examples_tutorial_basis.config.settings",
			);
		}

		// Ensure config module is loaded (triggers #[routes] macro)
		let _ = &config::urls::routes;

		// Execute command from command line
		if let Err(e) = execute_from_command_line().await {
			eprintln!("Error: {}", e);
			process::exit(1);
		}
	}
}

// Entry point for server builds
#[cfg(server)]
fn main() {
	server::main();
}

// Dummy entry point for WASM builds (this binary is server-only)
#[cfg(client)]
fn main() {
	panic!("manage binary should not be run in WASM context");
}
