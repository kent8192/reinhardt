//! Tailwind CSS Configuration
//!
//! Defines `TailwindConfig`, the central configuration struct for Tailwind CSS integration.
//! Provides a builder-style API for specifying the Tailwind version, content paths,
//! input/output CSS paths, and other build options.

use std::path::PathBuf;

/// Default Tailwind CSS version to use when none is specified.
const DEFAULT_TAILWIND_VERSION: &str = "4.1.0";

/// Default output path for the generated CSS file.
const DEFAULT_OUTPUT_PATH: &str = "static/css/tailwind.css";

/// Configuration for the Tailwind CSS build pipeline.
///
/// Use the builder pattern to construct a configuration:
///
/// ```ignore
/// use reinhardt_pages::tailwind::TailwindConfig;
///
/// let config = TailwindConfig::new()
///     .with_version("4.1.0")
///     .with_content_paths(vec!["src/**/*.rs".to_string()])
///     .with_output_path("static/css/tailwind.css".to_string())
///     .with_minify(true);
/// ```
#[derive(Debug, Clone)]
pub struct TailwindConfig {
	/// Tailwind CSS standalone CLI version (e.g., "4.1.0").
	version: String,
	/// Glob patterns for content files to scan for class names.
	content_paths: Vec<String>,
	/// Optional path to a custom input CSS file.
	input_css: Option<String>,
	/// Output path for the generated CSS file.
	output_path: String,
	/// Directory to cache the downloaded Tailwind CLI binary.
	cache_dir: PathBuf,
	/// Whether to minify the output CSS.
	minify: bool,
}

impl TailwindConfig {
	/// Create a new `TailwindConfig` with default values.
	///
	/// Defaults:
	/// - Version: `"4.1.0"`
	/// - Output path: `"static/css/tailwind.css"`
	/// - Cache directory: OS-specific cache directory or `/tmp/reinhardt-tailwind`
	/// - Minify: `false`
	pub fn new() -> Self {
		let cache_dir =
			dirs_cache_dir().unwrap_or_else(|| PathBuf::from("/tmp/reinhardt-tailwind"));
		Self {
			version: DEFAULT_TAILWIND_VERSION.to_string(),
			content_paths: Vec::new(),
			input_css: None,
			output_path: DEFAULT_OUTPUT_PATH.to_string(),
			cache_dir,
			minify: false,
		}
	}

	/// Set the Tailwind CSS version.
	pub fn with_version(mut self, version: &str) -> Self {
		self.version = version.to_string();
		self
	}

	/// Set the content paths (glob patterns) to scan for class names.
	///
	/// These patterns are passed to the Tailwind CLI and used by
	/// the class scanner to find source files.
	///
	/// ## Example
	///
	/// ```ignore
	/// let config = TailwindConfig::new()
	///     .with_content_paths(vec![
	///         "src/**/*.rs".to_string(),
	///         "templates/**/*.html".to_string(),
	///     ]);
	/// ```
	pub fn with_content_paths(mut self, paths: Vec<String>) -> Self {
		self.content_paths = paths;
		self
	}

	/// Set the input CSS file path.
	///
	/// This file can contain Tailwind directives like `@import "tailwindcss"`.
	/// If not set, Tailwind v4 will use its default configuration.
	pub fn with_input_css(mut self, path: String) -> Self {
		self.input_css = Some(path);
		self
	}

	/// Set the output CSS file path.
	pub fn with_output_path(mut self, path: String) -> Self {
		self.output_path = path;
		self
	}

	/// Set the cache directory for the Tailwind CLI binary.
	pub fn with_cache_dir(mut self, path: PathBuf) -> Self {
		self.cache_dir = path;
		self
	}

	/// Set whether to minify the output CSS.
	pub fn with_minify(mut self, minify: bool) -> Self {
		self.minify = minify;
		self
	}

	/// Returns the configured Tailwind CSS version.
	pub fn version(&self) -> &str {
		&self.version
	}

	/// Returns the configured content paths.
	pub fn content_paths(&self) -> &[String] {
		&self.content_paths
	}

	/// Returns the configured input CSS file path, if any.
	pub fn input_css(&self) -> Option<&str> {
		self.input_css.as_deref()
	}

	/// Returns the configured output CSS file path.
	pub fn output_path(&self) -> &str {
		&self.output_path
	}

	/// Returns the configured cache directory.
	pub fn cache_dir(&self) -> &PathBuf {
		&self.cache_dir
	}

	/// Returns whether output CSS should be minified.
	pub fn minify(&self) -> bool {
		self.minify
	}

	/// Validates the configuration and returns an error if invalid.
	pub fn validate(&self) -> Result<(), super::TailwindError> {
		if self.version.is_empty() {
			return Err(super::TailwindError::InvalidConfig(
				"version must not be empty".to_string(),
			));
		}
		if self.output_path.is_empty() {
			return Err(super::TailwindError::InvalidConfig(
				"output_path must not be empty".to_string(),
			));
		}
		Ok(())
	}
}

impl Default for TailwindConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Returns the OS-specific cache directory for Tailwind CLI binaries.
fn dirs_cache_dir() -> Option<PathBuf> {
	// Use a simple approach: $HOME/.cache/reinhardt-tailwind on Unix,
	// or fall back to /tmp
	std::env::var("HOME").ok().map(|home| {
		PathBuf::from(home)
			.join(".cache")
			.join("reinhardt-tailwind")
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_default_config() {
		// Arrange & Act
		let config = TailwindConfig::new();

		// Assert
		assert_eq!(config.version(), DEFAULT_TAILWIND_VERSION);
		assert_eq!(config.output_path(), DEFAULT_OUTPUT_PATH);
		assert!(config.content_paths().is_empty());
		assert!(config.input_css().is_none());
		assert!(!config.minify());
	}

	#[rstest]
	fn test_builder_methods() {
		// Arrange & Act
		let config = TailwindConfig::new()
			.with_version("3.4.0")
			.with_content_paths(vec!["src/**/*.rs".to_string()])
			.with_input_css("input.css".to_string())
			.with_output_path("dist/output.css".to_string())
			.with_minify(true);

		// Assert
		assert_eq!(config.version(), "3.4.0");
		assert_eq!(config.content_paths(), &["src/**/*.rs"]);
		assert_eq!(config.input_css(), Some("input.css"));
		assert_eq!(config.output_path(), "dist/output.css");
		assert!(config.minify());
	}

	#[rstest]
	fn test_validate_empty_version() {
		// Arrange
		let config = TailwindConfig::new().with_version("");

		// Act
		let result = config.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_empty_output_path() {
		// Arrange
		let config = TailwindConfig::new().with_output_path(String::new());

		// Act
		let result = config.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_valid_config() {
		// Arrange
		let config = TailwindConfig::new()
			.with_version("4.1.0")
			.with_content_paths(vec!["src/**/*.rs".to_string()])
			.with_output_path("out.css".to_string());

		// Act
		let result = config.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_default_trait() {
		// Arrange & Act
		let config = TailwindConfig::default();

		// Assert
		assert_eq!(config.version(), DEFAULT_TAILWIND_VERSION);
	}

	#[rstest]
	fn test_cache_dir_override() {
		// Arrange
		let custom_dir = PathBuf::from("/tmp/test-cache");

		// Act
		let config = TailwindConfig::new().with_cache_dir(custom_dir.clone());

		// Assert
		assert_eq!(config.cache_dir(), &custom_dir);
	}
}
