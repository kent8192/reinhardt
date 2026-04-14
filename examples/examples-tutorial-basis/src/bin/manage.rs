//! Reinhardt Project Management CLI for examples-tutorial-basis

use examples_tutorial_basis as _;
use reinhardt::commands::execute_from_command_line;
use std::process;

#[tokio::main]
async fn main() {
	// SAFETY: Called at program start before any spawned tasks.
	unsafe {
		std::env::set_var(
			"REINHARDT_SETTINGS_MODULE",
			"examples_tutorial_basis.config.settings",
		);
	}

	if let Err(e) = execute_from_command_line().await {
		eprintln!("Error: {e}");
		process::exit(1);
	}
}
