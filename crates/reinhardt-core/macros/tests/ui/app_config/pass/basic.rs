//! Test that basic #[app_config(...)] usage compiles successfully

use reinhardt_macros::app_config;

mod basic_app {
	use super::*;

	// App config without verbose_name
	#[app_config(name = "basic", label = "basic")]
	pub struct BasicConfig;
}

mod full_app {
	use super::*;

	// App config with verbose_name
	#[app_config(name = "full", label = "full", verbose_name = "Full Application")]
	pub struct FullConfig;
}

fn main() {
	// Compile test only - verify the macro expands without errors
}
