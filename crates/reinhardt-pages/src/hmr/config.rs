//! HMR configuration.

use std::path::PathBuf;

/// Default debounce interval in milliseconds.
const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Default WebSocket server port for HMR.
const DEFAULT_WS_PORT: u16 = 35729;

/// Configuration for the HMR system.
#[derive(Debug, Clone)]
pub struct HmrConfig {
	/// Directories to watch for file changes.
	pub watch_paths: Vec<PathBuf>,
	/// Debounce interval in milliseconds to coalesce rapid changes.
	pub debounce_ms: u64,
	/// Port for the WebSocket notification server.
	pub ws_port: u16,
	/// Whether HMR is enabled.
	pub enabled: bool,
}

impl Default for HmrConfig {
	fn default() -> Self {
		Self {
			watch_paths: vec![PathBuf::from("src"), PathBuf::from("templates")],
			debounce_ms: DEFAULT_DEBOUNCE_MS,
			ws_port: DEFAULT_WS_PORT,
			enabled: true,
		}
	}
}

impl HmrConfig {
	/// Creates a builder for `HmrConfig`.
	pub fn builder() -> HmrConfigBuilder {
		HmrConfigBuilder::new()
	}
}

/// Builder for `HmrConfig`.
#[derive(Debug, Clone)]
pub struct HmrConfigBuilder {
	watch_paths: Vec<PathBuf>,
	debounce_ms: u64,
	ws_port: u16,
	enabled: bool,
}

impl HmrConfigBuilder {
	/// Creates a new builder with default values.
	fn new() -> Self {
		Self {
			watch_paths: Vec::new(),
			debounce_ms: DEFAULT_DEBOUNCE_MS,
			ws_port: DEFAULT_WS_PORT,
			enabled: true,
		}
	}

	/// Adds a directory path to watch.
	pub fn watch_path(mut self, path: impl Into<PathBuf>) -> Self {
		self.watch_paths.push(path.into());
		self
	}

	/// Sets multiple watch paths at once.
	pub fn watch_paths(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
		self.watch_paths.extend(paths);
		self
	}

	/// Sets the debounce interval in milliseconds.
	pub fn debounce_ms(mut self, ms: u64) -> Self {
		self.debounce_ms = ms;
		self
	}

	/// Sets the WebSocket server port.
	pub fn ws_port(mut self, port: u16) -> Self {
		self.ws_port = port;
		self
	}

	/// Enables or disables HMR.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Builds the `HmrConfig`.
	///
	/// If no watch paths were specified, uses the defaults (`src/` and `templates/`).
	pub fn build(self) -> HmrConfig {
		let watch_paths = if self.watch_paths.is_empty() {
			vec![PathBuf::from("src"), PathBuf::from("templates")]
		} else {
			self.watch_paths
		};

		HmrConfig {
			watch_paths,
			debounce_ms: self.debounce_ms,
			ws_port: self.ws_port,
			enabled: self.enabled,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_default_config() {
		// Arrange & Act
		let config = HmrConfig::default();

		// Assert
		assert_eq!(config.watch_paths.len(), 2);
		assert_eq!(config.watch_paths[0], PathBuf::from("src"));
		assert_eq!(config.watch_paths[1], PathBuf::from("templates"));
		assert_eq!(config.debounce_ms, 300);
		assert_eq!(config.ws_port, 35729);
		assert!(config.enabled);
	}

	#[rstest]
	fn test_builder_defaults() {
		// Arrange & Act
		let config = HmrConfig::builder().build();

		// Assert
		assert_eq!(config.watch_paths.len(), 2);
		assert_eq!(config.debounce_ms, 300);
		assert_eq!(config.ws_port, 35729);
		assert!(config.enabled);
	}

	#[rstest]
	fn test_builder_custom_paths() {
		// Arrange & Act
		let config = HmrConfig::builder()
			.watch_path("custom/src")
			.watch_path("custom/templates")
			.build();

		// Assert
		assert_eq!(config.watch_paths.len(), 2);
		assert_eq!(config.watch_paths[0], PathBuf::from("custom/src"));
		assert_eq!(config.watch_paths[1], PathBuf::from("custom/templates"));
	}

	#[rstest]
	fn test_builder_custom_debounce() {
		// Arrange & Act
		let config = HmrConfig::builder().debounce_ms(500).build();

		// Assert
		assert_eq!(config.debounce_ms, 500);
	}

	#[rstest]
	fn test_builder_custom_port() {
		// Arrange & Act
		let config = HmrConfig::builder().ws_port(8080).build();

		// Assert
		assert_eq!(config.ws_port, 8080);
	}

	#[rstest]
	fn test_builder_disabled() {
		// Arrange & Act
		let config = HmrConfig::builder().enabled(false).build();

		// Assert
		assert!(!config.enabled);
	}

	#[rstest]
	fn test_builder_full_customization() {
		// Arrange & Act
		let config = HmrConfig::builder()
			.watch_path("app/src")
			.watch_path("app/styles")
			.watch_path("app/assets")
			.debounce_ms(150)
			.ws_port(9000)
			.enabled(true)
			.build();

		// Assert
		assert_eq!(config.watch_paths.len(), 3);
		assert_eq!(config.debounce_ms, 150);
		assert_eq!(config.ws_port, 9000);
		assert!(config.enabled);
	}
}
