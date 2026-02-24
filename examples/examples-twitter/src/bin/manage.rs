//! Reinhardt Project Management CLI for examples-twitter
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).

// Server-side implementation
#[cfg(server)]
mod server {
	use examples_twitter::config; // Explicitly reference config module
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
				"examples_twitter.config.settings",
			);
		}

		// Ensure config module is loaded (triggers #[routes] macro)
		let _ = &config::urls::routes;

		// Router registration is now automatic via #[routes] attribute macro
		// in src/config/urls.rs

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
	// This binary is not used in WASM builds
	panic!("manage binary should not be run in WASM context");
}
