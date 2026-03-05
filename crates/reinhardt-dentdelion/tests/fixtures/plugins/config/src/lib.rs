// Config Plugin - Test plugin for host configuration API
//
// This plugin demonstrates and tests the host's configuration API
// by calling get-config and set-config during initialization.

wit_bindgen::generate!({
	world: "dentdelion-plugin",
	path: "../../../../wit",
});

use exports::reinhardt::dentdelion::plugin::{Capability, Guest, PluginError, PluginMetadata};

struct ConfigPlugin;

impl Guest for ConfigPlugin {
	fn get_metadata() -> PluginMetadata {
		PluginMetadata {
			name: "config-test".to_string(),
			version: "0.1.0".to_string(),
			description: Some("Test plugin for configuration API".to_string()),
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
		use reinhardt::dentdelion::host::{get_config, log_info, set_config};

		// Log the initialization
		log_info("Config plugin initializing");

		// Log received config length (though we don't parse it for this test)
		log_info(&format!("Received config: {} bytes", config.len()));

		// Test get-config: Try to retrieve a configuration value
		log_info("Testing get-config with key: test_key");
		match get_config("test_key") {
			Some(value) => {
				log_info(&format!(
					"Successfully retrieved config value: {} bytes",
					value.len()
				));
			}
			None => {
				log_info("Config value not found (expected for test)");
			}
		}

		// Test set-config: Set a configuration value to indicate initialization
		log_info("Testing set-config with key: plugin_initialized");
		let value = vec![1u8]; // Simple boolean-like value
		match set_config("plugin_initialized", &value) {
			Ok(_) => {
				log_info("Successfully set plugin_initialized config");
			}
			Err(e) => {
				log_info(&format!("Failed to set config: {} - {}", e.code, e.message));
				return Err(PluginError {
					code: e.code,
					message: format!("Failed to set config: {}", e.message),
					details: e.details,
				});
			}
		}

		log_info("Config plugin initialized successfully");
		Ok(())
	}

	fn on_enable() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_info;
		log_info("Config plugin enabled");
		Ok(())
	}

	fn on_disable() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_info;
		log_info("Config plugin disabled");
		Ok(())
	}

	fn on_unload() -> Result<(), PluginError> {
		use reinhardt::dentdelion::host::log_info;
		log_info("Config plugin unloaded");
		Ok(())
	}
}

export!(ConfigPlugin);
