//! Common test helpers for static file functionality
//!
//! Provides helper functions to consolidate duplicate tests for static file handling.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create temporary directories and files for testing
pub struct TestFileSetup {
	/// The temporary directory that holds the test files.
	pub temp_dir: TempDir,
	/// The path to the primary test file.
	pub file_path: PathBuf,
	/// The content of the primary test file.
	pub content: Vec<u8>,
}

impl TestFileSetup {
	/// Creates a test file with the specified filename and content
	pub fn new(filename: &str, content: &[u8]) -> Self {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join(filename);
		fs::write(&file_path, content).unwrap();

		Self {
			temp_dir,
			file_path,
			content: content.to_vec(),
		}
	}

	/// Creates a test file in a nested directory structure
	pub fn with_nested_path(base_path: &str, filename: &str, content: &[u8]) -> Self {
		let temp_dir = TempDir::new().unwrap();
		let full_path = temp_dir.path().join(base_path).join(filename);
		fs::create_dir_all(full_path.parent().unwrap()).unwrap();
		fs::write(&full_path, content).unwrap();

		Self {
			temp_dir,
			file_path: full_path,
			content: content.to_vec(),
		}
	}

	/// Creates multiple test files
	pub fn with_multiple_files(files: &[(&str, &[u8])]) -> Self {
		let temp_dir = TempDir::new().unwrap();

		for (filename, content) in files {
			let file_path = temp_dir.path().join(filename);
			if let Some(parent) = file_path.parent() {
				fs::create_dir_all(parent).unwrap();
			}
			fs::write(&file_path, content).unwrap();
		}

		// Use the first file as the main file
		let first_file = files.first().unwrap();
		let file_path = temp_dir.path().join(first_file.0);
		let content = first_file.1.to_vec();

		Self {
			temp_dir,
			file_path,
			content,
		}
	}
}

/// Common assertions for static file tests
pub mod assertions {
	use reinhardt_utils::staticfiles::handler::StaticError;

	/// Asserts that a file is served successfully
	pub fn assert_file_served_successfully(
		result: Result<reinhardt_utils::staticfiles::handler::StaticFile, StaticError>,
		expected_content: &[u8],
	) {
		assert!(result.is_ok(), "File should be served successfully");
		let static_file = result.unwrap();
		assert_eq!(static_file.content, expected_content);
	}

	/// Asserts that a file not found error occurs
	pub fn assert_file_not_found_error(
		result: Result<reinhardt_utils::staticfiles::handler::StaticFile, StaticError>,
	) {
		assert!(result.is_err(), "Should return error for non-existent file");
		assert!(matches!(result.unwrap_err(), StaticError::NotFound(_)));
	}

	/// Asserts that directory traversal attacks are blocked
	pub fn assert_directory_traversal_blocked(
		result: Result<reinhardt_utils::staticfiles::handler::StaticFile, StaticError>,
	) {
		assert!(result.is_err(), "Directory traversal should be blocked");
	}
}

/// Common helpers for configuration tests
pub mod config_helpers {
	use reinhardt_utils::staticfiles::storage::StaticFilesConfig;
	use std::path::{Path, PathBuf};

	/// Creates a default configuration
	pub fn create_default_config() -> StaticFilesConfig {
		StaticFilesConfig::default()
	}

	/// Creates a custom configuration
	pub fn create_custom_config(
		static_root: PathBuf,
		static_url: String,
		staticfiles_dirs: Vec<PathBuf>,
	) -> StaticFilesConfig {
		StaticFilesConfig {
			static_root,
			static_url,
			staticfiles_dirs,
			media_url: None,
		}
	}

	/// Tests basic configuration properties
	pub fn assert_config_properties(
		config: &StaticFilesConfig,
		expected_root: &Path,
		expected_url: &str,
		expected_dirs_count: usize,
	) {
		assert_eq!(config.static_root, expected_root);
		assert_eq!(config.static_url, expected_url);
		assert_eq!(config.staticfiles_dirs.len(), expected_dirs_count);
	}
}

/// Common helpers for integration tests
pub mod integration_helpers {
	use super::*;
	use reinhardt_utils::staticfiles::handler::StaticFileHandler;
	use reinhardt_utils::staticfiles::storage::{StaticFilesConfig, StaticFilesFinder};

	/// Setup for integration tests
	pub struct IntegrationTestSetup {
		/// Temporary directories used by the test (kept alive for the test duration).
		pub temp_dirs: Vec<TempDir>,
		/// Static files configuration for the test.
		pub config: StaticFilesConfig,
		/// Finder instance for locating static files across configured directories.
		pub finder: StaticFilesFinder,
		/// Handler instance for serving static files.
		pub handler: StaticFileHandler,
	}

	impl Default for IntegrationTestSetup {
		fn default() -> Self {
			Self::new()
		}
	}

	impl IntegrationTestSetup {
		/// Creates a new integration test setup
		pub fn new() -> Self {
			let temp_dir = TempDir::new().unwrap();
			let config = StaticFilesConfig {
				static_root: temp_dir.path().to_path_buf(),
				static_url: "/static/".to_string(),
				staticfiles_dirs: vec![temp_dir.path().to_path_buf()],
				media_url: None,
			};

			let finder = StaticFilesFinder::new(config.staticfiles_dirs.clone());
			let handler = StaticFileHandler::new(temp_dir.path().to_path_buf());

			Self {
				temp_dirs: vec![temp_dir],
				config,
				finder,
				handler,
			}
		}

		/// Creates setup with multiple directories
		pub fn with_multiple_dirs() -> Self {
			let temp_dir1 = TempDir::new().unwrap();
			let temp_dir2 = TempDir::new().unwrap();

			let config = StaticFilesConfig {
				static_root: temp_dir1.path().to_path_buf(),
				static_url: "/static/".to_string(),
				staticfiles_dirs: vec![
					temp_dir1.path().to_path_buf(),
					temp_dir2.path().to_path_buf(),
				],
				media_url: None,
			};

			let finder = StaticFilesFinder::new(config.staticfiles_dirs.clone());
			let handler = StaticFileHandler::new(temp_dir1.path().to_path_buf());

			Self {
				temp_dirs: vec![temp_dir1, temp_dir2],
				config,
				finder,
				handler,
			}
		}

		/// Creates a test file
		pub fn create_test_file(&self, filename: &str, content: &[u8]) -> PathBuf {
			let file_path = self.temp_dirs[0].path().join(filename);
			if let Some(parent) = file_path.parent() {
				fs::create_dir_all(parent).unwrap();
			}
			fs::write(&file_path, content).unwrap();
			file_path
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_utils::staticfiles::handler::StaticError;

	#[test]
	fn test_file_setup_builders_create_exact_tree_and_cleanup_on_drop() {
		// Arrange
		let single = TestFileSetup::new("readme.txt", b"single");
		let nested = TestFileSetup::with_nested_path("assets/css", "site.css", b"body {}\n");
		let multiple = TestFileSetup::with_multiple_files(&[
			("images/logo.svg", b"<svg/>"),
			("scripts/app.js", b"console.log('ready');"),
		]);

		// Act
		let single_content = fs::read(&single.file_path).unwrap();
		let nested_content = fs::read(&nested.file_path).unwrap();
		let multiple_content = fs::read(&multiple.file_path).unwrap();

		// Assert
		assert_eq!(single.file_path, single.temp_dir.path().join("readme.txt"));
		assert_eq!(single.content, b"single");
		assert_eq!(single_content, b"single");
		assert_eq!(
			nested.file_path,
			nested.temp_dir.path().join("assets/css/site.css")
		);
		assert_eq!(nested.content, b"body {}\n");
		assert_eq!(nested_content, b"body {}\n");
		assert_eq!(
			multiple.file_path,
			multiple.temp_dir.path().join("images/logo.svg")
		);
		assert_eq!(multiple.content, b"<svg/>");
		assert_eq!(multiple_content, b"<svg/>");
		assert_eq!(
			fs::read(multiple.temp_dir.path().join("scripts/app.js")).unwrap(),
			b"console.log('ready');"
		);

		let cleanup_setup = TestFileSetup::new("cleanup.txt", b"remove me");
		let cleanup_path = cleanup_setup.temp_dir.path().to_path_buf();
		drop(cleanup_setup);
		assert_eq!(cleanup_path.exists(), false);
	}

	#[tokio::test]
	async fn static_integration_helpers_find_serve_and_reject_traversal() {
		// Arrange
		let setup = integration_helpers::IntegrationTestSetup::with_multiple_dirs();
		let first_path = setup.create_test_file("nested/app.css", b"body { color: teal; }");
		let second_path = setup.temp_dirs[1].path().join("images/logo.svg");
		fs::create_dir_all(second_path.parent().unwrap()).unwrap();
		fs::write(&second_path, b"<svg viewBox=\"0 0 1 1\"/>").unwrap();

		// Act
		let found_first = setup.finder.find("nested/app.css").unwrap();
		let found_second = setup.finder.find("/images/logo.svg").unwrap();
		let served = setup.handler.serve("nested/app.css").await.unwrap();
		let missing = setup.handler.serve("missing.css").await.unwrap_err();
		let traversal = setup.handler.serve("../secret.txt").await.unwrap_err();

		// Assert
		assert_eq!(setup.config.static_root, setup.temp_dirs[0].path());
		assert_eq!(setup.config.static_url, "/static/");
		assert_eq!(setup.config.staticfiles_dirs.len(), 2);
		assert_eq!(
			fs::read(&found_first).unwrap(),
			fs::read(&first_path).unwrap()
		);
		assert_eq!(found_first.file_name().unwrap(), "app.css");
		assert_eq!(
			fs::read(&found_second).unwrap(),
			fs::read(&second_path).unwrap()
		);
		assert_eq!(found_second.file_name().unwrap(), "logo.svg");
		assert_eq!(fs::read(&served.path).unwrap(), b"body { color: teal; }");
		assert_eq!(served.path.file_name().unwrap(), "app.css");
		assert_eq!(served.content, b"body { color: teal; }");
		assert_eq!(served.mime_type, "text/css");
		match missing {
			StaticError::NotFound(path) => assert_eq!(path, "missing.css"),
			other => panic!("expected not found error, got {other:?}"),
		}
		match traversal {
			StaticError::DirectoryTraversal(path) => assert_eq!(path, "../secret.txt"),
			other => panic!("expected traversal error, got {other:?}"),
		}
		match setup.finder.find("../secret.txt") {
			Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
			Ok(path) => panic!(
				"expected traversal lookup to fail, found {}",
				path.display()
			),
		}
	}

	#[tokio::test]
	async fn static_helpers_build_configs_and_assert_single_directory_results() {
		// Arrange
		let default_config = config_helpers::create_default_config();
		let setup = integration_helpers::IntegrationTestSetup::default();
		let root = setup.temp_dirs[0].path().to_path_buf();
		let custom_config = config_helpers::create_custom_config(
			root.clone(),
			"/assets/".into(),
			vec![root.clone()],
		);
		let created = setup.create_test_file("index.html", b"<h1>static</h1>");

		// Act
		let found = setup.finder.find("index.html").unwrap();
		let served = setup.handler.serve("index.html").await;
		let missing = setup.handler.serve("missing.html").await;
		let traversal = setup.handler.serve("../private.html").await;

		// Assert
		assert_eq!(default_config.static_root, PathBuf::from("static"));
		assert_eq!(default_config.static_url, "/static/");
		assert_eq!(default_config.staticfiles_dirs, Vec::<PathBuf>::new());
		assert_eq!(default_config.media_url, None);
		config_helpers::assert_config_properties(&custom_config, &root, "/assets/", 1);
		assert_eq!(custom_config.staticfiles_dirs, vec![root]);
		assert_eq!(fs::read(found).unwrap(), fs::read(created).unwrap());
		assertions::assert_file_served_successfully(served, b"<h1>static</h1>");
		assertions::assert_file_not_found_error(missing);
		assertions::assert_directory_traversal_blocked(traversal);
	}
}
