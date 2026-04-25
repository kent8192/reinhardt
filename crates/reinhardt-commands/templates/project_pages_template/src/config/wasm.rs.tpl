//! WASM artifacts registration for `collectstatic`.
//!
//! Registers the `dist-wasm` directory containing WASM build artifacts so
//! `cargo run --bin manage collectstatic` discovers and copies them into
//! the final distribution directory.

use reinhardt::reinhardt_apps::AppStaticFilesConfig;

inventory::submit! {
	AppStaticFilesConfig {
		app_label: "{{ crate_name }}-wasm",
		static_dir: "dist-wasm",
		url_prefix: "",
	}
}
