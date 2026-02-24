//! Testing utilities for settings
//!
//! Provides helpers for testing configuration in unit and integration tests.
//!
//! This module is part of reinhardt-conf crate.

use super::env_loader::load_env_optional;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test environment helper
///
/// Provides isolated environment for testing settings
pub struct TestEnv {
	temp_dir: TempDir,
	original_env: HashMap<String, Option<String>>,
	modified_keys: Vec<String>,
}

impl TestEnv {
	/// Create a new test environment
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let mut test_env = TestEnv::new().unwrap();
	/// test_env.set_var("TEST_KEY", "test_value");
	/// assert_eq!(std::env::var("TEST_KEY").unwrap(), "test_value");
	// Environment is cleaned up when test_env is dropped
	/// ```
	pub fn new() -> std::io::Result<Self> {
		Ok(Self {
			temp_dir: TempDir::new()?,
			original_env: HashMap::new(),
			modified_keys: Vec::new(),
		})
	}
	/// Get the temporary directory path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let test_env = TestEnv::new().unwrap();
	/// let temp_path = test_env.path();
	/// assert!(temp_path.exists());
	/// ```
	pub fn path(&self) -> &Path {
		self.temp_dir.path()
	}
	/// Set an environment variable for this test
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let mut test_env = TestEnv::new().unwrap();
	/// test_env.set_var("DB_HOST", "localhost");
	/// assert_eq!(std::env::var("DB_HOST").unwrap(), "localhost");
	/// ```
	pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) {
		let key = key.into();

		// Store original value if not already stored
		if !self.original_env.contains_key(&key) {
			self.original_env.insert(key.clone(), env::var(&key).ok());
		}

		if !self.modified_keys.contains(&key) {
			self.modified_keys.push(key.clone());
		}

		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// TestEnv is designed for use in tests with #[serial] to ensure exclusive access.
		unsafe {
			env::set_var(&key, value.into());
		}
	}
	/// Remove an environment variable for this test
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let mut test_env = TestEnv::new().unwrap();
	/// test_env.set_var("TEMP_VAR", "value");
	/// test_env.remove_var("TEMP_VAR");
	/// assert!(std::env::var("TEMP_VAR").is_err());
	/// ```
	pub fn remove_var(&mut self, key: impl Into<String>) {
		let key = key.into();

		// Store original value if not already stored
		if !self.original_env.contains_key(&key) {
			self.original_env.insert(key.clone(), env::var(&key).ok());
		}

		if !self.modified_keys.contains(&key) {
			self.modified_keys.push(key.clone());
		}

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// TestEnv is designed for use in tests with #[serial] to ensure exclusive access.
		unsafe {
			env::remove_var(&key);
		}
	}
	/// Create a .env file in the temporary directory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let test_env = TestEnv::new().unwrap();
	/// let env_file = test_env.create_env_file("DEBUG=true\nPORT=8080").unwrap();
	/// assert!(env_file.exists());
	/// ```
	pub fn create_env_file(&self, content: &str) -> std::io::Result<PathBuf> {
		let env_path = self.temp_dir.path().join(".env");
		std::fs::write(&env_path, content)?;
		Ok(env_path)
	}
	/// Create a config file (TOML) in the temporary directory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let test_env = TestEnv::new().unwrap();
	/// let config = test_env.create_config_file("app.toml", "[database]\nhost = \"localhost\"").unwrap();
	/// assert!(config.exists());
	/// ```
	pub fn create_config_file(&self, filename: &str, content: &str) -> std::io::Result<PathBuf> {
		let config_path = self.temp_dir.path().join(filename);
		std::fs::write(&config_path, content)?;
		Ok(config_path)
	}
	/// Load environment from a .env file in the temp directory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestEnv;
	///
	/// let test_env = TestEnv::new().unwrap();
	/// test_env.create_env_file("API_KEY=secret123").unwrap();
	/// test_env.load_env().unwrap();
	// Environment variables from .env are now loaded
	/// ```
	pub fn load_env(&self) -> Result<(), super::env::EnvError> {
		let env_path = self.temp_dir.path().join(".env");
		if env_path.exists() {
			load_env_optional(&env_path)?;
		}
		Ok(())
	}
}

impl Drop for TestEnv {
	fn drop(&mut self) {
		// Restore original environment variables
		for key in &self.modified_keys {
			if let Some(original) = self.original_env.get(key) {
				// SAFETY: Restoring environment variables is unsafe in multi-threaded programs.
				// TestEnv is designed for use in tests with #[serial] to ensure exclusive access.
				unsafe {
					match original {
						Some(val) => env::set_var(key, val),
						None => env::remove_var(key),
					}
				}
			}
		}
	}
}

impl Default for TestEnv {
	fn default() -> Self {
		Self::new().expect("Failed to create test environment")
	}
}

/// Settings builder for testing
pub struct TestSettingsBuilder {
	env_vars: HashMap<String, String>,
	config_content: Option<String>,
}

impl TestSettingsBuilder {
	/// Create a new test settings builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestSettingsBuilder;
	///
	/// let builder = TestSettingsBuilder::new()
	///     .env("DATABASE_URL", "sqlite::memory:")
	///     .config("[app]\nname = \"test\"");
	/// let test_env = builder.build();
	/// ```
	pub fn new() -> Self {
		Self {
			env_vars: HashMap::new(),
			config_content: None,
		}
	}
	/// Add an environment variable
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestSettingsBuilder;
	///
	/// let builder = TestSettingsBuilder::new()
	///     .env("DEBUG", "true")
	///     .env("PORT", "3000");
	/// ```
	pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.env_vars.insert(key.into(), value.into());
		self
	}
	/// Set config file content (TOML)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestSettingsBuilder;
	///
	/// let builder = TestSettingsBuilder::new()
	///     .config("[database]\nhost = \"localhost\"\nport = 5432");
	/// ```
	pub fn config(mut self, content: impl Into<String>) -> Self {
		self.config_content = Some(content.into());
		self
	}
	/// Build a test environment
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::testing::TestSettingsBuilder;
	///
	/// let test_env = TestSettingsBuilder::new()
	///     .env("APP_NAME", "test_app")
	///     .config("[server]\nport = 8080")
	///     .build();
	///
	/// assert_eq!(std::env::var("APP_NAME").unwrap(), "test_app");
	/// ```
	pub fn build(self) -> TestEnv {
		let mut env = TestEnv::new().unwrap();

		// Set environment variables
		for (key, value) in self.env_vars {
			env.set_var(key, value);
		}

		// Create config file if provided
		if let Some(content) = self.config_content {
			env.create_config_file("config.toml", &content).unwrap();
		}

		env
	}
}

impl Default for TestSettingsBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Assert that an environment variable has a specific value
#[macro_export]
macro_rules! assert_env {
	($key:expr, $expected:expr) => {
		assert_eq!(
			std::env::var($key).ok().as_deref(),
			Some($expected),
			"Environment variable {} does not match",
			$key
		);
	};
}

/// Assert that an environment variable exists
#[macro_export]
macro_rules! assert_env_exists {
	($key:expr) => {
		assert!(
			std::env::var($key).is_ok(),
			"Environment variable {} does not exist",
			$key
		);
	};
}

/// Assert that an environment variable does not exist
#[macro_export]
macro_rules! assert_env_not_exists {
	($key:expr) => {
		assert!(
			std::env::var($key).is_err(),
			"Environment variable {} should not exist",
			$key
		);
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_env_isolation() {
		let mut env = TestEnv::new().unwrap();

		// Set a test variable
		env.set_var("TEST_VAR_ISOLATION", "test_value");
		assert_eq!(env::var("TEST_VAR_ISOLATION").unwrap(), "test_value");

		// Drop the environment
		drop(env);

		// Variable should be cleaned up
		assert!(env::var("TEST_VAR_ISOLATION").is_err());
	}

	#[test]
	fn test_env_restoration() {
		// Set an original value
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_VAR_RESTORE", "original");
		}

		{
			let mut env = TestEnv::new().unwrap();
			env.set_var("TEST_VAR_RESTORE", "modified");
			assert_eq!(env::var("TEST_VAR_RESTORE").unwrap(), "modified");
		}

		// Should be restored to original
		assert_eq!(env::var("TEST_VAR_RESTORE").unwrap(), "original");

		// Cleanup
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_VAR_RESTORE");
		}
	}

	#[test]
	fn test_create_env_file() {
		let env = TestEnv::new().unwrap();

		let content = "TEST_KEY=test_value\nANOTHER_KEY=another_value";
		let path = env.create_env_file(content).unwrap();

		assert!(path.exists());
		let read_content = std::fs::read_to_string(path).unwrap();
		assert_eq!(read_content, content);
	}

	#[test]
	fn test_create_config_file() {
		let env = TestEnv::new().unwrap();

		let content = "[database]\nhost = \"localhost\"\nport = 5432";
		let path = env.create_config_file("config.toml", content).unwrap();

		assert!(path.exists());
		let read_content = std::fs::read_to_string(path).unwrap();
		assert_eq!(read_content, content);
	}

	#[test]
	fn test_settings_builder() {
		let env = TestSettingsBuilder::new()
			.env("BUILDER_TEST_KEY", "builder_value")
			.config("[test]\nvalue = 42")
			.build();

		assert_eq!(env::var("BUILDER_TEST_KEY").unwrap(), "builder_value");

		let config_path = env.path().join("config.toml");
		assert!(config_path.exists());
	}

	#[test]
	fn test_remove_var() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_REMOVE_VAR", "exists");
		}

		{
			let mut env = TestEnv::new().unwrap();
			env.remove_var("TEST_REMOVE_VAR");
			assert!(env::var("TEST_REMOVE_VAR").is_err());
		}

		// Should be restored
		assert_eq!(env::var("TEST_REMOVE_VAR").unwrap(), "exists");

		// Cleanup
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_REMOVE_VAR");
		}
	}

	#[test]
	fn test_macro_assert_env() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("MACRO_TEST_VAR", "macro_value");
		}

		assert_env!("MACRO_TEST_VAR", "macro_value");
		assert_env_exists!("MACRO_TEST_VAR");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("MACRO_TEST_VAR");
		}

		assert_env_not_exists!("MACRO_TEST_VAR");
	}
}
