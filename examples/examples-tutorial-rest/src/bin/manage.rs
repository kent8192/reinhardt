//! Reinhardt Project Management CLI for examples-tutorial-rest
//!
//! This binary is native-only. The WASM target retains an empty `main`
//! so workspace target checks can skip the Tokio-based management runtime.

#[cfg(not(target_arch = "wasm32"))]
mod native {
	use examples_tutorial_rest as _;
	use examples_tutorial_rest::config::settings::get_settings;
	#[cfg(feature = "commands-shell")]
	use examples_tutorial_rest::config::shell::get_shell_config;
	#[cfg(not(feature = "commands-shell"))]
	use reinhardt::commands::execute_from_command_line_with_pending_settings;
	#[cfg(feature = "commands-shell")]
	use reinhardt::commands::execute_from_command_line_with_pending_settings_and_shell;
	use std::process;

	#[tokio::main]
	pub(super) async fn main() {
		// SAFETY: Called at program start before any spawned tasks.
		unsafe {
			std::env::set_var(
				"REINHARDT_SETTINGS_MODULE",
				"examples_tutorial_rest.config.settings",
			);
		}

		#[cfg(feature = "commands-shell")]
		let result =
			execute_from_command_line_with_pending_settings_and_shell(|| Ok(get_settings()), get_shell_config())
				.await;
		#[cfg(not(feature = "commands-shell"))]
		let result = execute_from_command_line_with_pending_settings(|| Ok(get_settings())).await;

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
