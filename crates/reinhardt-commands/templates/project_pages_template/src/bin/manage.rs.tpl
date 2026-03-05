//! Reinhardt Project Management CLI for {{ project_name }}
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).
//!
//! ## Router Registration
//!
//! URL patterns are automatically registered by the framework.
//! No manual registration is required - see `src/config/urls.rs` for the
//! `#[routes]` attribute macro that enables this.

use {{ crate_name }} as _;
use reinhardt::commands::execute_from_command_line;
use std::process;

#[tokio::main]
async fn main() {
	// Set settings module environment variable
	// SAFETY: This is safe because we're setting it before any other code runs
	unsafe {
		std::env::set_var("REINHARDT_SETTINGS_MODULE", "{{ project_name }}.config.settings");
	}

	// Router registration happens automatically inside execute_from_command_line()
	// via the #[routes] attribute macro in src/config/urls.rs
	if let Err(e) = execute_from_command_line().await {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}
