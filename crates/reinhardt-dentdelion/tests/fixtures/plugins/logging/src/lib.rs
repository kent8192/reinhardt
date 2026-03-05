// Logging Plugin - Test plugin for host logging API
//
// This plugin demonstrates and tests the host's logging API
// by calling log_info, log_debug, log_warn, and log_error during lifecycle events.

wit_bindgen::generate!({
	world: "dentdelion-plugin",
	path: "../../../../../wit",
});

use exports::reinhardt::dentdelion::plugin::{Capability, Guest, PluginError, PluginMetadata};

struct LoggingPlugin;

impl Guest for LoggingPlugin {
	fn get_metadata() -> PluginMetadata {
		PluginMetadata {
			name: "logging-test".to_string(),
			version: "0.1.0".to_string(),
			description: Some("Test plugin for logging API".to_string()),
			authors: vec!["Reinhardt Contributors".to_string()],
			license: Some("BSD-3-Clause".to_string()),
			repository: Some("https://github.com/kent8192/reinhardt-web".to_string()),
			homepage: Some("https://github.com/kent8192/reinhardt-web".to_string()),
		}
	}

	fn get_capabilities() -> Vec<Capability> {
		// This plugin doesn't provide any capabilities
		Vec::new()
	}

	fn on_load(config: Vec<u8>) -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::{log_debug, log_info};

		// Test log_info during plugin load
		log_info("Logging plugin loaded");

		// Test log_debug to demonstrate debug-level logging
		log_debug("Debug message from plugin");

		// Log received config length for verification
		log_debug(&format!("Received config: {} bytes", config.len()));

		Ok(())
	}

	fn on_enable() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_info;

		// Test log_info during plugin enable
		log_info("Logging plugin enabled");

		Ok(())
	}

	fn on_disable() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_warn;

		// Test log_warn during plugin disable
		log_warn("Logging plugin disabled");

		Ok(())
	}

	fn on_unload() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_info;

		// Test log_info during plugin unload
		log_info("Logging plugin unloaded");

		Ok(())
	}
}

export!(LoggingPlugin);
