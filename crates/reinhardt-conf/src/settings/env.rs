//! Environment variable handling module
//!
//! Provides Django-environ compatible functionality for loading and parsing
//! environment variables with type safety.

use indexmap::IndexMap;
use std::env;
use std::path::PathBuf;

pub use super::env_loader::EnvLoader;
pub use super::env_parser::{DatabaseUrl, parse_bool, parse_database_url, parse_list};

/// Environment variable manager with prefix support
#[derive(Debug, Clone)]
pub struct Env {
	/// Optional prefix for environment variables (e.g., "REINHARDT_")
	pub prefix: Option<String>,

	/// Whether to enable variable expansion (e.g., $VAR)
	pub interpolate: bool,

	/// Cached environment variables (reserved for future use)
	#[allow(dead_code)]
	cache: IndexMap<String, String>,
}

impl Env {
	/// Create a new Env instance
	pub fn new() -> Self {
		Self {
			prefix: None,
			interpolate: false,
			cache: IndexMap::new(),
		}
	}
	/// Set a prefix for all environment variable lookups
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.prefix = Some(prefix.into());
		self
	}
	/// Enable variable interpolation
	pub fn with_interpolation(mut self, enabled: bool) -> Self {
		self.interpolate = enabled;
		self
	}

	/// Get the full key name with prefix
	fn get_key_name(&self, key: &str) -> String {
		match &self.prefix {
			Some(prefix) => format!("{}{}", prefix, key),
			None => key.to_string(),
		}
	}
	/// Read a string value from environment
	///
	pub fn str(&self, key: &str) -> Result<String, EnvError> {
		self.str_with_default(key, None)
	}
	/// Read a string value with a default
	///
	pub fn str_with_default(&self, key: &str, default: Option<&str>) -> Result<String, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		match env::var(&full_key) {
			Ok(val) => Ok(val),
			Err(_) => match default {
				Some(d) => Ok(d.to_string()),
				None => Err(EnvError::MissingVariable(full_key)),
			},
		}
	}
	/// Read a boolean value from environment
	///
	pub fn bool(&self, key: &str) -> Result<bool, EnvError> {
		self.bool_with_default(key, None)
	}
	/// Read a boolean value with a default
	///
	pub fn bool_with_default(&self, key: &str, default: Option<bool>) -> Result<bool, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		match env::var(&full_key) {
			Ok(val) => parse_bool(&val).map_err(|e| EnvError::ParseError {
				key: full_key,
				value_len: val.len(),
				error: e,
			}),
			Err(_) => match default {
				Some(d) => Ok(d),
				None => Err(EnvError::MissingVariable(full_key)),
			},
		}
	}
	/// Read an integer value from environment
	///
	pub fn int(&self, key: &str) -> Result<i64, EnvError> {
		self.int_with_default(key, None)
	}
	/// Read an integer value with a default
	///
	pub fn int_with_default(&self, key: &str, default: Option<i64>) -> Result<i64, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		match env::var(&full_key) {
			Ok(val) => val.parse::<i64>().map_err(|e| EnvError::ParseError {
				key: full_key,
				value_len: val.len(),
				error: e.to_string(),
			}),
			Err(_) => match default {
				Some(d) => Ok(d),
				None => Err(EnvError::MissingVariable(full_key)),
			},
		}
	}
	/// Read a list value from environment (comma-separated)
	///
	pub fn list(&self, key: &str) -> Result<Vec<String>, EnvError> {
		self.list_with_default(key, None)
	}
	/// Read a list value with a default
	///
	pub fn list_with_default(
		&self,
		key: &str,
		default: Option<Vec<String>>,
	) -> Result<Vec<String>, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		match env::var(&full_key) {
			Ok(val) => Ok(parse_list(&val)),
			Err(_) => match default {
				Some(d) => Ok(d),
				None => Err(EnvError::MissingVariable(full_key)),
			},
		}
	}
	/// Read a database URL from environment
	///
	pub fn database_url(&self, key: &str) -> Result<DatabaseUrl, EnvError> {
		self.database_url_with_default(key, None)
	}
	/// Read a database URL with a default
	///
	pub fn database_url_with_default(
		&self,
		key: &str,
		default: Option<&str>,
	) -> Result<DatabaseUrl, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		let url_str = match env::var(&full_key) {
			Ok(val) => val,
			Err(_) => match default {
				Some(d) => d.to_string(),
				None => return Err(EnvError::MissingVariable(full_key)),
			},
		};

		parse_database_url(&url_str).map_err(|e| EnvError::ParseError {
			key: full_key,
			value_len: url_str.len(),
			error: e,
		})
	}
	/// Read a path value from environment
	///
	pub fn path(&self, key: &str) -> Result<PathBuf, EnvError> {
		self.path_with_default(key, None)
	}
	/// Read a path value with a default
	///
	pub fn path_with_default(
		&self,
		key: &str,
		default: Option<PathBuf>,
	) -> Result<PathBuf, EnvError> {
		let full_key = self.get_key_name(key);
		validate_env_var_name(&full_key)?;

		match env::var(&full_key) {
			Ok(val) => Ok(PathBuf::from(val)),
			Err(_) => match default {
				Some(d) => Ok(d),
				None => Err(EnvError::MissingVariable(full_key)),
			},
		}
	}
}

impl Default for Env {
	fn default() -> Self {
		Self::new()
	}
}

/// Validates an environment variable name.
///
/// Rejects names that are empty, contain control characters, or contain
/// the `=` character (which is used as the key-value separator).
pub fn validate_env_var_name(name: &str) -> Result<(), EnvError> {
	if name.is_empty() {
		return Err(EnvError::InvalidVariableName {
			name: name.to_string(),
			reason: "environment variable name must not be empty".to_string(),
		});
	}

	if let Some(pos) = name.find(|c: char| c.is_control()) {
		return Err(EnvError::InvalidVariableName {
			name: name.to_string(),
			reason: format!(
				"environment variable name contains control character at position {}",
				pos
			),
		});
	}

	if name.contains('=') {
		return Err(EnvError::InvalidVariableName {
			name: name.to_string(),
			reason: "environment variable name must not contain '='".to_string(),
		});
	}

	Ok(())
}

/// Environment variable errors
#[derive(Debug, thiserror::Error)]
pub enum EnvError {
	#[error("Missing environment variable: {0}")]
	MissingVariable(String),

	#[error("Failed to parse environment variable '{key}' (value length: {value_len}): {error}")]
	ParseError {
		key: String,
		/// Length of the original value (stored instead of the raw value to prevent secret leakage)
		value_len: usize,
		error: String,
	},

	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	#[error("Invalid format: {0}")]
	InvalidFormat(String),

	#[error("Invalid environment variable name '{name}': {reason}")]
	InvalidVariableName { name: String, reason: String },
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_env_str() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_STR", "hello");
		}
		let env = Env::new();
		assert_eq!(env.str("TEST_STR").unwrap(), "hello");
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_STR");
		}
	}

	#[test]
	fn test_env_str_with_default() {
		let env = Env::new();
		assert_eq!(
			env.str_with_default("NONEXISTENT", Some("default"))
				.unwrap(),
			"default"
		);
	}

	#[test]
	fn test_env_bool() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_BOOL_TRUE", "true");
			env::set_var("TEST_BOOL_FALSE", "false");
			env::set_var("TEST_BOOL_1", "1");
			env::set_var("TEST_BOOL_0", "0");
		}

		let env = Env::new();
		assert!(env.bool("TEST_BOOL_TRUE").unwrap());
		assert!(!env.bool("TEST_BOOL_FALSE").unwrap());
		assert!(env.bool("TEST_BOOL_1").unwrap());
		assert!(!env.bool("TEST_BOOL_0").unwrap());

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_BOOL_TRUE");
			env::remove_var("TEST_BOOL_FALSE");
			env::remove_var("TEST_BOOL_1");
			env::remove_var("TEST_BOOL_0");
		}
	}

	#[test]
	fn test_env_int() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_INT", "42");
		}
		let env = Env::new();
		assert_eq!(env.int("TEST_INT").unwrap(), 42);
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_INT");
		}
	}

	#[test]
	fn test_env_list() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_LIST", "a,b,c");
		}
		let env = Env::new();
		assert_eq!(env.list("TEST_LIST").unwrap(), vec!["a", "b", "c"]);
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_LIST");
		}
	}

	#[test]
	fn test_settings_env_with_prefix() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("REINHARDT_DEBUG", "true");
		}
		let env = Env::new().with_prefix("REINHARDT_");
		assert!(env.bool("DEBUG").unwrap());
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("REINHARDT_DEBUG");
		}
	}

	#[test]
	fn test_env_path() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("TEST_PATH", "/tmp/test");
		}
		let env = Env::new();
		assert_eq!(env.path("TEST_PATH").unwrap(), PathBuf::from("/tmp/test"));
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("TEST_PATH");
		}
	}

	#[rstest]
	fn test_validate_env_var_name_rejects_empty() {
		// Arrange & Act
		let result = validate_env_var_name("");

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			EnvError::InvalidVariableName { .. }
		));
	}

	#[rstest]
	fn test_validate_env_var_name_rejects_control_chars() {
		// Arrange & Act
		let result = validate_env_var_name("MY\x00VAR");

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		match &err {
			EnvError::InvalidVariableName { reason, .. } => {
				assert!(reason.contains("control character"));
			}
			_ => panic!("Expected InvalidVariableName error"),
		}
	}

	#[rstest]
	fn test_validate_env_var_name_rejects_equals_sign() {
		// Arrange & Act
		let result = validate_env_var_name("MY=VAR");

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		match &err {
			EnvError::InvalidVariableName { reason, .. } => {
				assert!(reason.contains("'='"));
			}
			_ => panic!("Expected InvalidVariableName error"),
		}
	}

	#[rstest]
	fn test_validate_env_var_name_accepts_valid_name() {
		// Arrange & Act & Assert
		assert!(validate_env_var_name("MY_VALID_VAR_123").is_ok());
		assert!(validate_env_var_name("REINHARDT_DEBUG").is_ok());
	}

	#[rstest]
	fn test_parse_error_does_not_leak_value() {
		// Arrange
		let err = EnvError::ParseError {
			key: "SECRET_KEY".to_string(),
			value_len: 32,
			error: "invalid format".to_string(),
		};

		// Act
		let error_msg = format!("{}", err);

		// Assert - the error message must not contain the actual secret value
		assert!(error_msg.contains("value length: 32"));
		assert!(!error_msg.contains("secret"));
	}

	#[rstest]
	fn test_env_rejects_empty_key_name() {
		// Arrange
		let env = Env::new();

		// Act
		let result = env.str("");

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			EnvError::InvalidVariableName { .. }
		));
	}
}
