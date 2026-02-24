//! WASM artifacts registration for collectstatic
//!
//! This module registers the dist-wasm directory containing WASM build artifacts
//! so that collectstatic can automatically discover and collect them to the final
//! distribution directory.

use reinhardt::reinhardt_apps::AppStaticFilesConfig;

inventory::submit! {
	AppStaticFilesConfig {
		app_label: "examples-tutorial-basis-wasm",
		static_dir: "dist-wasm",
		url_prefix: "",
	}
}
