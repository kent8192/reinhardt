//! Test data loader helper.
//!
//! Provides convenient methods for loading test fixture data files.

use std::path::{Path, PathBuf};

/// Test data loader for fixture files.
///
/// Provides methods to load test fixture data from the tests/fixtures/data directory.
pub struct TestDataLoader {
	base_path: PathBuf,
}

impl TestDataLoader {
	/// Create a new test data loader.
	///
	/// Uses the default test fixtures data directory.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_seeding::tests::helpers::TestDataLoader;
	///
	/// let loader = TestDataLoader::new();
	/// let json_data = loader.load_json("valid_users");
	/// ```
	pub fn new() -> Self {
		Self {
			base_path: PathBuf::from("tests/fixtures/data"),
		}
	}

	/// Create a test data loader with a custom base path.
	///
	/// # Arguments
	///
	/// * `base_path` - Custom base directory for test data
	pub fn with_base<P: AsRef<Path>>(base_path: P) -> Self {
		Self {
			base_path: base_path.as_ref().to_path_buf(),
		}
	}

	/// Load JSON test data by name.
	///
	/// # Arguments
	///
	/// * `name` - Name of the test data file (without .json extension)
	///
	/// # Returns
	///
	/// The file contents as a string.
	///
	/// # Panics
	///
	/// Panics if the file cannot be read.
	pub fn load_json(&self, name: &str) -> String {
		let path = self.base_path.join(format!("{}.json", name));
		std::fs::read_to_string(&path)
			.unwrap_or_else(|_| panic!("Failed to load test data: {:?}", path))
	}

	/// Load YAML test data by name.
	///
	/// # Arguments
	///
	/// * `name` - Name of the test data file (without .yaml extension)
	///
	/// # Returns
	///
	/// The file contents as a string.
	///
	/// # Panics
	///
	/// Panics if the file cannot be read.
	pub fn load_yaml(&self, name: &str) -> String {
		let path = self.base_path.join(format!("{}.yaml", name));
		std::fs::read_to_string(&path)
			.unwrap_or_else(|_| panic!("Failed to load test data: {:?}", path))
	}

	/// Get the full path to a test data file.
	///
	/// # Arguments
	///
	/// * `name` - Name of the test data file (with extension)
	///
	/// # Returns
	///
	/// Full path to the test data file.
	pub fn path(&self, name: &str) -> PathBuf {
		self.base_path.join(name)
	}

	/// Check if a test data file exists.
	///
	/// # Arguments
	///
	/// * `name` - Name of the test data file (with extension)
	///
	/// # Returns
	///
	/// `true` if the file exists, `false` otherwise.
	pub fn exists(&self, name: &str) -> bool {
		self.path(name).exists()
	}
}

impl Default for TestDataLoader {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest::rstest]
	fn test_test_data_loader_creation() {
		let loader = TestDataLoader::new();
		assert_eq!(loader.base_path, PathBuf::from("tests/fixtures/data"));
	}

	#[rstest::rstest]
	fn test_test_data_loader_with_base() {
		let loader = TestDataLoader::with_base("/custom/path");
		assert_eq!(loader.base_path, PathBuf::from("/custom/path"));
	}

	#[rstest::rstest]
	fn test_test_data_loader_path() {
		let loader = TestDataLoader::new();
		let path = loader.path("test.json");
		assert!(path.ends_with("tests/fixtures/data/test.json"));
	}

	#[rstest::rstest]
	fn test_test_data_loader_default() {
		let loader = TestDataLoader::default();
		assert_eq!(loader.base_path, PathBuf::from("tests/fixtures/data"));
	}
}
