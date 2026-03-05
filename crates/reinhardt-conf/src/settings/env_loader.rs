//! .env file loading functionality
//!
//! Provides Django-environ compatible .env file parsing and loading.

use std::env;
use std::fs;
use std::path::PathBuf;

use super::env::{EnvError, validate_env_var_name};

/// Environment file loader
pub struct EnvLoader {
	/// Path to the .env file
	path: Option<PathBuf>,

	/// Whether to overwrite existing environment variables
	overwrite: bool,

	/// Whether to enable variable interpolation
	interpolate: bool,
}

impl EnvLoader {
	/// Create a new EnvLoader
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	/// use std::path::PathBuf;
	///
	/// let loader = EnvLoader::new()
	///     .path(PathBuf::from(".env.test"))
	///     .interpolate(true);
	///
	// Loader is configured and ready to load .env files
	// Can call loader.load_optional() to load the file
	/// ```
	pub fn new() -> Self {
		Self {
			path: None,
			overwrite: false,
			interpolate: false,
		}
	}
	/// Set the path to the .env file
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	/// use std::path::PathBuf;
	///
	/// let loader = EnvLoader::new()
	///     .path(PathBuf::from(".env.production"));
	///
	/// // Loader is configured to load from .env.production
	/// ```
	pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
		self.path = Some(path.into());
		self
	}
	/// Enable overwriting existing environment variables
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	///
	/// let loader = EnvLoader::new()
	///     .overwrite(true);
	///
	/// // When loading .env, existing env vars will be overwritten
	/// ```
	pub fn overwrite(mut self, enabled: bool) -> Self {
		self.overwrite = enabled;
		self
	}
	/// Enable variable interpolation ($VAR expansion)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	///
	/// let loader = EnvLoader::new()
	///     .interpolate(true);
	///
	/// // Variables like $HOME or ${USER} will be expanded
	/// ```
	pub fn interpolate(mut self, enabled: bool) -> Self {
		self.interpolate = enabled;
		self
	}
	/// Load environment variables from the .env file
	///
	/// # Thread Safety
	///
	/// This method calls `env::set_var` internally, which is not thread-safe.
	/// It MUST only be called during single-threaded application startup,
	/// before any worker threads are spawned.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	/// use std::io::Write;
	///
	/// let temp_dir = tempfile::tempdir().unwrap();
	/// let env_path = temp_dir.path().join(".env");
	/// let mut file = std::fs::File::create(&env_path).unwrap();
	/// writeln!(file, "TEST_KEY=test_value").unwrap();
	///
	/// let loader = EnvLoader::new().path(env_path);
	/// loader.load().expect("Failed to load .env");
	///
	/// assert_eq!(std::env::var("TEST_KEY").unwrap(), "test_value");
	/// ```
	pub fn load(&self) -> Result<(), EnvError> {
		let path = match &self.path {
			Some(p) => p.clone(),
			None => self.find_env_file()?,
		};

		if !path.exists() {
			return Err(EnvError::IoError(std::io::Error::new(
				std::io::ErrorKind::NotFound,
				format!(".env file not found: {}", path.display()),
			)));
		}

		let content = fs::read_to_string(&path)?;
		self.parse_and_set(&content)?;

		Ok(())
	}
	/// Try to load the .env file, but don't fail if it doesn't exist
	///
	/// # Thread Safety
	///
	/// This method calls `env::set_var` internally, which is not thread-safe.
	/// It MUST only be called during single-threaded application startup,
	/// before any worker threads are spawned.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::env_loader::EnvLoader;
	/// use std::path::PathBuf;
	///
	/// let loader = EnvLoader::new()
	///     .path(PathBuf::from(".env.optional"));
	///
	/// // Returns Ok(true) if loaded, Ok(false) if not found
	/// let loaded = loader.load_optional().unwrap();
	/// // Won't panic if file doesn't exist
	/// ```
	pub fn load_optional(&self) -> Result<bool, EnvError> {
		let path = match &self.path {
			Some(p) => p.clone(),
			None => match self.find_env_file() {
				Ok(p) => p,
				Err(_) => return Ok(false),
			},
		};

		if !path.exists() {
			return Ok(false);
		}

		let content = fs::read_to_string(&path)?;
		self.parse_and_set(&content)?;

		Ok(true)
	}

	/// Maximum number of parent directories to traverse when searching for .env files.
	/// Prevents unbounded traversal to the filesystem root in deeply nested directories.
	const MAX_TRAVERSAL_DEPTH: usize = 10;

	/// Project root marker files that stop .env file traversal.
	const ROOT_MARKERS: &[&str] = &[".git", "Cargo.toml", "Cargo.lock"];

	/// Find the .env file in current or parent directories.
	///
	/// Traversal stops at:
	/// - A directory containing a `.env` file (found)
	/// - A project root marker (`.git`, `Cargo.toml`, `Cargo.lock`)
	/// - The maximum traversal depth ([`Self::MAX_TRAVERSAL_DEPTH`])
	/// - The filesystem root
	fn find_env_file(&self) -> Result<PathBuf, EnvError> {
		let mut current = env::current_dir()?;

		for _depth in 0..Self::MAX_TRAVERSAL_DEPTH {
			let env_path = current.join(".env");
			if env_path.exists() {
				return Ok(env_path);
			}

			// Stop at project root markers to avoid loading unintended .env files
			if Self::ROOT_MARKERS
				.iter()
				.any(|marker| current.join(marker).exists())
			{
				return Err(EnvError::IoError(std::io::Error::new(
					std::io::ErrorKind::NotFound,
					format!(
						".env file not found (stopped at project root: {})",
						current.display()
					),
				)));
			}

			match current.parent() {
				Some(parent) => current = parent.to_path_buf(),
				None => {
					return Err(EnvError::IoError(std::io::Error::new(
						std::io::ErrorKind::NotFound,
						".env file not found in current or parent directories",
					)));
				}
			}
		}

		Err(EnvError::IoError(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			format!(
				".env file not found within {} parent directories",
				Self::MAX_TRAVERSAL_DEPTH
			),
		)))
	}

	/// Parse .env file content and set environment variables
	fn parse_and_set(&self, content: &str) -> Result<(), EnvError> {
		for (line_num, line) in content.lines().enumerate() {
			let trimmed = line.trim();

			// Skip empty lines and comments
			if trimmed.is_empty() || trimmed.starts_with('#') {
				continue;
			}

			// Handle export prefix
			let line_content = if trimmed.starts_with("export ") {
				trimmed.trim_start_matches("export ").trim()
			} else {
				trimmed
			};

			// Parse key=value
			if let Some((key, value)) = line_content.split_once('=') {
				let key = key.trim();
				validate_env_var_name(key)?;
				let mut value = value.trim().to_string();

				// Remove quotes if present
				if (value.starts_with('"') && value.ends_with('"'))
					|| (value.starts_with('\'') && value.ends_with('\''))
				{
					value = value[1..value.len() - 1].to_string();
				}

				// Handle variable interpolation
				if self.interpolate {
					value = self.expand_variables(&value);
				}

				// Handle escaped characters
				value = self.unescape(&value);

				// Set or skip based on overwrite setting
				if self.overwrite || env::var(key).is_err() {
					// SAFETY: `env::set_var` is not thread-safe per POSIX and Rust 2024
					// edition marks it as unsafe. This call is safe because:
					// 1. EnvLoader is designed to run during single-threaded application
					//    startup (before any worker threads are spawned).
					// 2. Callers MUST NOT invoke `parse_and_set` from multi-threaded
					//    contexts. The public API (`load`, `load_optional`) documents
					//    this startup-only constraint.
					// 3. If env mutation is needed after startup, callers should store
					//    values in a thread-safe map (e.g., `RwLock<HashMap>`) instead.
					unsafe {
						env::set_var(key, value);
					}
				}
			} else {
				return Err(EnvError::InvalidFormat(format!(
					"Invalid line format at line {}: {}",
					line_num + 1,
					line
				)));
			}
		}

		Ok(())
	}

	/// Expand variables in the format $VAR or ${VAR}
	fn expand_variables(&self, value: &str) -> String {
		let mut result = String::new();
		let mut chars = value.chars().peekable();

		while let Some(ch) = chars.next() {
			if ch == '$' {
				if chars.peek() == Some(&'{') {
					// ${VAR} format
					chars.next(); // consume '{'
					let var_name: String = chars.by_ref().take_while(|&c| c != '}').collect();

					if let Ok(var_value) = env::var(&var_name) {
						result.push_str(&var_value);
					}
				} else {
					// $VAR format - collect variable name
					let mut var_name = String::new();
					while let Some(&next_ch) = chars.peek() {
						if next_ch.is_alphanumeric() || next_ch == '_' {
							var_name.push(next_ch);
							chars.next();
						} else {
							break;
						}
					}

					if let Ok(var_value) = env::var(&var_name) {
						result.push_str(&var_value);
					}
				}
			} else if ch == '\\' && chars.peek() == Some(&'$') {
				// Escaped dollar sign
				chars.next();
				result.push('$');
			} else {
				result.push(ch);
			}
		}

		result
	}

	/// Unescape common escape sequences
	fn unescape(&self, value: &str) -> String {
		value
			.replace("\\n", "\n")
			.replace("\\r", "\r")
			.replace("\\t", "\t")
			.replace("\\\\", "\\")
	}
}

impl Default for EnvLoader {
	fn default() -> Self {
		Self::new()
	}
}
/// Load .env file from the specified path
///
/// # Examples
///
/// ```rust
/// use reinhardt_conf::settings::env_loader::load_env;
/// use std::io::Write;
///
/// let temp_dir = tempfile::tempdir().unwrap();
/// let env_path = temp_dir.path().join(".env");
/// let mut file = std::fs::File::create(&env_path).unwrap();
/// writeln!(file, "LOAD_ENV_KEY=loaded").unwrap();
///
/// load_env(env_path).expect("Failed to load .env");
/// assert_eq!(std::env::var("LOAD_ENV_KEY").unwrap(), "loaded");
/// ```
pub fn load_env(path: impl Into<PathBuf>) -> Result<(), EnvError> {
	EnvLoader::new().path(path).load()
}
/// Load .env file from current or parent directories
///
/// # Examples
///
/// ```rust
/// use reinhardt_conf::settings::env_loader::EnvLoader;
/// use std::io::Write;
///
/// let temp_dir = tempfile::tempdir().unwrap();
/// let env_path = temp_dir.path().join(".env");
/// let mut file = std::fs::File::create(&env_path).unwrap();
/// writeln!(file, "AUTO_LOAD_KEY=auto").unwrap();
///
/// // Change to temp directory for auto-discovery
/// let original_dir = std::env::current_dir().unwrap();
/// std::env::set_current_dir(temp_dir.path()).unwrap();
///
/// let loader = EnvLoader::new();
/// loader.load().expect("Failed to auto-load .env");
///
/// assert_eq!(std::env::var("AUTO_LOAD_KEY").unwrap(), "auto");
///
/// // Restore original directory
/// std::env::set_current_dir(original_dir).unwrap();
/// ```
pub fn load_env_auto() -> Result<(), EnvError> {
	EnvLoader::new().load()
}
/// Load .env file optionally (don't fail if not found)
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_loader::load_env_optional;
/// use std::path::PathBuf;
///
/// // Returns true if loaded, false if not found
/// let loaded = load_env_optional(PathBuf::from(".env.test")).unwrap();
/// // Will not panic if file doesn't exist
/// ```
pub fn load_env_optional(path: impl Into<PathBuf>) -> Result<bool, EnvError> {
	EnvLoader::new().path(path).load_optional()
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs::File;
	use std::io::Write;
	use tempfile::TempDir;

	#[test]
	fn test_parse_simple_env() {
		let content = r#"
# Comment
KEY1=value1
KEY2=value2
        "#;

		let loader = EnvLoader::new();
		loader.parse_and_set(content).unwrap();

		assert_eq!(env::var("KEY1").unwrap(), "value1");
		assert_eq!(env::var("KEY2").unwrap(), "value2");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("KEY1");
			env::remove_var("KEY2");
		}
	}

	#[test]
	fn test_parse_quoted_values() {
		let content = r#"
QUOTED_SINGLE='single quoted'
QUOTED_DOUBLE="double quoted"
        "#;

		let loader = EnvLoader::new();
		loader.parse_and_set(content).unwrap();

		assert_eq!(env::var("QUOTED_SINGLE").unwrap(), "single quoted");
		assert_eq!(env::var("QUOTED_DOUBLE").unwrap(), "double quoted");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("QUOTED_SINGLE");
			env::remove_var("QUOTED_DOUBLE");
		}
	}

	#[test]
	fn test_parse_export() {
		let content = r#"
export EXPORTED_VAR="exported value"
        "#;

		let loader = EnvLoader::new();
		loader.parse_and_set(content).unwrap();

		assert_eq!(env::var("EXPORTED_VAR").unwrap(), "exported value");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("EXPORTED_VAR");
		}
	}

	#[test]
	fn test_variable_expansion() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("BASE_VAR", "base");
		}

		let content = r#"
EXPANDED=$BASE_VAR/expanded
EXPANDED_BRACES=${BASE_VAR}/expanded
        "#;

		let loader = EnvLoader::new().interpolate(true);
		loader.parse_and_set(content).unwrap();

		assert_eq!(env::var("EXPANDED").unwrap(), "base/expanded");
		assert_eq!(env::var("EXPANDED_BRACES").unwrap(), "base/expanded");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("BASE_VAR");
			env::remove_var("EXPANDED");
			env::remove_var("EXPANDED_BRACES");
		}
	}

	#[test]
	fn test_escaped_dollar() {
		let content = r#"
ESCAPED=\$not_expanded
        "#;

		let loader = EnvLoader::new().interpolate(true);
		loader.parse_and_set(content).unwrap();

		assert_eq!(env::var("ESCAPED").unwrap(), "$not_expanded");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("ESCAPED");
		}
	}

	#[test]
	fn test_load_from_file() {
		let temp_dir = TempDir::new().unwrap();
		let env_path = temp_dir.path().join(".env");

		let mut file = File::create(&env_path).unwrap();
		writeln!(file, "FILE_VAR=file_value").unwrap();

		let loader = EnvLoader::new().path(&env_path);
		loader.load().unwrap();

		assert_eq!(env::var("FILE_VAR").unwrap(), "file_value");

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("FILE_VAR");
		}
	}
}
