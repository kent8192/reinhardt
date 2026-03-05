//! Reinhardt Project Management CLI for examples-multi-auth
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).

use examples_multi_auth::config;
use reinhardt::commands::execute_from_command_line;
use std::process;

#[tokio::main]
async fn main() {
	// Set settings module environment variable
	// SAFETY: This is safe because we're setting it before any other code runs
	unsafe {
		std::env::set_var(
			"REINHARDT_SETTINGS_MODULE",
			"examples_multi_auth.config.settings",
		);
	}

	// Ensure config module is loaded (triggers #[routes] macro)
	let _ = &config::urls::routes;

	// Router registration is now automatic via #[routes] attribute macro
	if let Err(e) = execute_from_command_line().await {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}
