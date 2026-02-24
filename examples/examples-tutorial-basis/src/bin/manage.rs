//! Reinhardt Project Management CLI for examples-tutorial-basis
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).

use reinhardt::commands::execute_from_command_line;
use reinhardt::core::tokio;
use std::process;

// Import the lib crate to ensure inventory registrations (e.g., #[routes]) are linked into the binary.
// Without this, inventory::iter will not find any registered URL patterns.
use examples_tutorial_basis as _;

#[tokio::main]
async fn main() {
	// Set settings module environment variable
	// SAFETY: This is safe because we're setting it before any other code runs
	unsafe {
		std::env::set_var(
			"REINHARDT_SETTINGS_MODULE",
			"examples_tutorial_basis.config.settings",
		);
	}

	// Execute command from command line
	if let Err(e) = execute_from_command_line().await {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}
