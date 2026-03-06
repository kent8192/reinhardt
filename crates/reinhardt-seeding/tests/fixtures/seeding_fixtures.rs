//! Specialized test fixtures for reinhardt-seeding integration tests.
//!
//! These fixtures wrap reinhardt-test DatabaseFixture with seeding-specific
//! functionality like temporary fixture directories and data loaders.

use reinhardt_seeding::FixtureLoader;
use std::path::PathBuf;
use tempfile::TempDir;

/// Seeding test fixture that combines database setup with seeding utilities.
///
/// This fixture provides:
/// - Database connection via reinhardt-test
/// - Fixture loader for loading fixture files
/// - Temporary directory for test fixture files
/// - Automatic cleanup on drop
pub struct SeedingFixture {
	/// Temporary directory for test fixture files (auto-cleanup)
	pub temp_dir: TempDir,
	/// Path to fixture files directory
	pub fixture_path: PathBuf,
	/// Fixture loader instance
	pub loader: FixtureLoader,
}

impl SeedingFixture {
	/// Create a new seeding fixture.
	///
	/// This sets up a temporary directory for fixture files and initializes
	/// the fixture loader. The temporary directory is automatically cleaned
	/// up when the fixture is dropped.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_seeding::tests::fixtures::SeedingFixture;
	///
	/// let fixture = SeedingFixture::new();
	/// // Use fixture.temp_dir for temporary files
	/// // Use fixture.loader to load fixtures
	/// // Automatic cleanup on drop
	/// ```
	pub fn new() -> Self {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let fixture_path = temp_dir.path().join("fixtures");

		std::fs::create_dir_all(&fixture_path).expect("Failed to create fixtures dir");

		Self {
			temp_dir,
			fixture_path,
			loader: FixtureLoader::new(),
		}
	}

	/// Write fixture content to a file in the temporary directory.
	///
	/// # Arguments
	///
	/// * `filename` - Name of the fixture file
	/// * `content` - Content to write to the file
	///
	/// # Examples
	///
	/// ```ignore
	/// let fixture = SeedingFixture::new();
	/// fixture.write_fixture("users.json", r#"
	///   [{"model": "auth.User", "pk": 1, "fields": {"username": "test"}}]
	/// "#);
	/// ```
	pub fn write_fixture(&self, filename: &str, content: &str) -> PathBuf {
		let file_path = self.fixture_path.join(filename);
		std::fs::write(&file_path, content).expect("Failed to write fixture file");
		file_path
	}

	/// Get the path to a fixture file.
	///
	/// # Arguments
	///
	/// * `filename` - Name of the fixture file
	///
	/// # Returns
	///
	/// Full path to the fixture file in the temporary directory.
	pub fn fixture_file(&self, filename: &str) -> PathBuf {
		self.fixture_path.join(filename)
	}
}

impl Default for SeedingFixture {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest::rstest]
	fn test_seeding_fixture_creation() {
		let fixture = SeedingFixture::new();
		assert!(fixture.temp_dir.path().exists());
		assert!(fixture.fixture_path.exists());
	}

	#[rstest::rstest]
	fn test_seeding_fixture_write_and_read() {
		let fixture = SeedingFixture::new();
		let content = r#"[{"model": "test", "pk": 1}]"#;
		fixture.write_fixture("test.json", content);

		let path = fixture.fixture_file("test.json");
		assert!(path.exists());

		let read_content = std::fs::read_to_string(&path).unwrap();
		assert_eq!(read_content, content);
	}

	#[rstest::rstest]
	fn test_seeding_fixture_default() {
		let fixture = SeedingFixture::default();
		assert!(fixture.fixture_path.exists());
	}
}
