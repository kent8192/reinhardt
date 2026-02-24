//! Reinhardt Project Management CLI for database-integration example
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).

use reinhardt::commands::execute_from_command_line;
use reinhardt::core::tokio;
use std::process;

#[tokio::main]
async fn main() {
	// Set settings module environment variable
	// SAFETY: This is safe because we're setting it before any other code runs
	unsafe {
		std::env::set_var(
			"REINHARDT_SETTINGS_MODULE",
			"database_integration.config.settings",
		);
	}

	// Router registration is now automatic via #[routes] attribute macro
	// in src/config/urls.rs

	// Execute command from command line
	if let Err(e) = execute_from_command_line().await {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}
