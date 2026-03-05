//! Migration numbering system
//!
//! Provides app-specific sequential numbering for migrations (0001, 0002, 0003, ...).

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Global cache for migration numbering
///
/// Key format: "{migrations_dir}:{app_label}"
/// Value: highest migration number for that app
static NUMBERING_CACHE: Lazy<Arc<RwLock<HashMap<String, u32>>>> =
	Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

/// Migration numbering system
pub struct MigrationNumbering;

impl MigrationNumbering {
	/// Get next migration number for an app (cached version)
	///
	/// Uses a global cache to avoid repeated filesystem scans.
	/// Thread-safe using RwLock.
	///
	/// # Arguments
	///
	/// * `migrations_dir` - Path to the migrations directory (e.g., `migrations/`)
	/// * `app_label` - App label (e.g., `"myapp"`)
	///
	/// # Returns
	///
	/// Next migration number as 4-digit zero-padded string (e.g., `"0001"`, `"0002"`)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::MigrationNumbering;
	/// use std::path::Path;
	///
	/// let next_num = MigrationNumbering::next_number_cached(
	///     Path::new("migrations"),
	///     "myapp"
	/// );
	/// assert_eq!(next_num, "0001"); // First migration
	/// ```
	pub fn next_number_cached(migrations_dir: &Path, app_label: &str) -> String {
		let cache_key = format!("{}:{}", migrations_dir.display(), app_label);

		// Try to get from cache
		{
			let cache = NUMBERING_CACHE.read().unwrap();
			if let Some(&cached_num) = cache.get(&cache_key) {
				// Increment and update cache
				drop(cache); // Release read lock
				let next = cached_num + 1;
				NUMBERING_CACHE.write().unwrap().insert(cache_key, next);
				return Self::format_number(next);
			}
		}

		// Cache miss - scan filesystem
		let highest = Self::get_highest_number(migrations_dir, app_label);
		NUMBERING_CACHE.write().unwrap().insert(cache_key, highest);
		Self::format_number(highest + 1)
	}

	/// Get next migration number for an app (non-cached version)
	///
	/// Scans existing migration files in the app's migrations directory
	/// and returns the next sequential number (4-digit zero-padded).
	///
	/// # Arguments
	///
	/// * `migrations_dir` - Path to the migrations directory (e.g., `migrations/`)
	/// * `app_label` - App label (e.g., `"myapp"`)
	///
	/// # Returns
	///
	/// Next migration number as 4-digit zero-padded string (e.g., `"0001"`, `"0002"`)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::MigrationNumbering;
	/// use std::path::Path;
	///
	/// let next_num = MigrationNumbering::next_number(
	///     Path::new("migrations"),
	///     "myapp"
	/// );
	/// assert_eq!(next_num, "0001"); // First migration
	/// ```
	pub fn next_number(migrations_dir: &Path, app_label: &str) -> String {
		let highest = Self::get_highest_number(migrations_dir, app_label);
		Self::format_number(highest + 1)
	}

	/// Invalidate the global cache
	///
	/// Call this when migrations are manually deleted or modified outside
	/// of the normal flow to ensure cache consistency.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::MigrationNumbering;
	///
	/// // After manually deleting migrations
	/// MigrationNumbering::invalidate_cache();
	/// ```
	pub fn invalidate_cache() {
		NUMBERING_CACHE.write().unwrap().clear();
	}

	/// Format a migration number as a zero-padded string
	///
	/// Pads to at least 4 digits, but preserves larger numbers as-is.
	fn format_number(num: u32) -> String {
		if num <= 9999 {
			format!("{:04}", num)
		} else {
			format!("{}", num)
		}
	}

	/// Get highest existing migration number for an app
	///
	/// Scans migration files matching the pattern `NNNN_*.rs` and returns
	/// the highest number found, or 0 if no migrations exist.
	///
	/// # File Name Pattern
	///
	/// Expects migration files in format: `{app_label}/NNNN_*.rs`
	/// - `NNNN`: 4-digit zero-padded number
	/// - `*`: migration name (e.g., `initial`, `add_user_email`)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_db::migrations::MigrationNumbering;
	/// # use std::path::Path;
	/// // Given files:
	/// // migrations/myapp/0001_initial.rs
	/// // migrations/myapp/0002_add_field.rs
	/// // migrations/myapp/0003_remove_field.rs
	///
	/// let highest = MigrationNumbering::get_highest_number(
	///     Path::new("migrations"),
	///     "myapp"
	/// );
	/// assert_eq!(highest, 3);
	/// ```
	pub fn get_highest_number(migrations_dir: &Path, app_label: &str) -> u32 {
		let app_migrations_dir = migrations_dir.join(app_label);

		// If directory doesn't exist, this is the first migration
		if !app_migrations_dir.exists() {
			return 0;
		}

		let mut highest = 0;

		// Scan for migration files matching NNNN_*.rs
		if let Ok(entries) = std::fs::read_dir(&app_migrations_dir) {
			for entry in entries.flatten() {
				let path = entry.path();

				// Only process .rs files
				if path.extension().and_then(|s| s.to_str()) != Some("rs") {
					continue;
				}

				// Extract filename
				if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
					// Parse all leading digits dynamically (supports 4+ digit prefixes, #1334)
					let prefix: String = filename
						.chars()
						.take_while(|c| c.is_ascii_digit())
						.collect();
					if !prefix.is_empty()
						&& let Ok(num) = prefix.parse::<u32>()
					{
						highest = highest.max(num);
					}
				}
			}
		}

		highest
	}

	/// Get all app migration numbers
	///
	/// Scans the migrations directory and returns a map of app labels
	/// to their highest migration numbers.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_db::migrations::MigrationNumbering;
	/// # use std::path::Path;
	/// // Given structure:
	/// // migrations/
	/// //   myapp/0001_initial.rs
	/// //   myapp/0002_add_field.rs
	/// //   other_app/0001_initial.rs
	///
	/// let numbers = MigrationNumbering::get_all_numbers(Path::new("migrations"));
	/// assert_eq!(numbers.get("myapp"), Some(&2));
	/// assert_eq!(numbers.get("other_app"), Some(&1));
	/// ```
	pub fn get_all_numbers(migrations_dir: &Path) -> HashMap<String, u32> {
		let mut result = HashMap::new();

		// If directory doesn't exist, return empty map
		if !migrations_dir.exists() {
			return result;
		}

		// Scan for app directories
		if let Ok(entries) = std::fs::read_dir(migrations_dir) {
			for entry in entries.flatten() {
				let path = entry.path();

				// Only process directories
				if !path.is_dir() {
					continue;
				}

				// Extract app label from directory name
				if let Some(app_label) = path.file_name().and_then(|s| s.to_str()) {
					let highest = Self::get_highest_number(migrations_dir, app_label);
					if highest > 0 {
						result.insert(app_label.to_string(), highest);
					}
				}
			}
		}

		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs;

	#[rstest]
	#[test]
	fn test_next_number_first_migration() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");

		// Act
		let next = MigrationNumbering::next_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(next, "0001");
	}

	#[rstest]
	#[test]
	fn test_next_number_existing_migrations() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");
		let app_dir = migrations_dir.join("myapp");
		fs::create_dir_all(&app_dir).unwrap();
		fs::write(app_dir.join("0001_initial.rs"), "").unwrap();
		fs::write(app_dir.join("0002_add_field.rs"), "").unwrap();
		fs::write(app_dir.join("0003_remove_field.rs"), "").unwrap();

		// Act
		let next = MigrationNumbering::next_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(next, "0004");
	}

	#[rstest]
	#[test]
	fn test_get_highest_number_no_migrations() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");

		// Act
		let highest = MigrationNumbering::get_highest_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(highest, 0);
	}

	#[rstest]
	#[test]
	fn test_get_highest_number_with_migrations() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");
		let app_dir = migrations_dir.join("myapp");
		fs::create_dir_all(&app_dir).unwrap();
		fs::write(app_dir.join("0001_initial.rs"), "").unwrap();
		fs::write(app_dir.join("0005_add_field.rs"), "").unwrap();
		fs::write(app_dir.join("0003_remove_field.rs"), "").unwrap();

		// Act
		let highest = MigrationNumbering::get_highest_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(highest, 5);
	}

	#[rstest]
	#[test]
	fn test_get_highest_number_ignores_non_migration_files() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");
		let app_dir = migrations_dir.join("myapp");
		fs::create_dir_all(&app_dir).unwrap();
		fs::write(app_dir.join("0001_initial.rs"), "").unwrap();
		fs::write(app_dir.join("README.md"), "").unwrap();
		fs::write(app_dir.join("myapp.rs"), "").unwrap();
		fs::write(app_dir.join("invalid_name.rs"), "").unwrap();

		// Act
		let highest = MigrationNumbering::get_highest_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(highest, 1);
	}

	#[rstest]
	#[test]
	fn test_get_all_numbers() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");
		let app1_dir = migrations_dir.join("app1");
		let app2_dir = migrations_dir.join("app2");
		fs::create_dir_all(&app1_dir).unwrap();
		fs::create_dir_all(&app2_dir).unwrap();
		fs::write(app1_dir.join("0001_initial.rs"), "").unwrap();
		fs::write(app1_dir.join("0002_add_field.rs"), "").unwrap();
		fs::write(app2_dir.join("0001_initial.rs"), "").unwrap();

		// Act
		let all_numbers = MigrationNumbering::get_all_numbers(&migrations_dir);

		// Assert
		assert_eq!(all_numbers.get("app1"), Some(&2));
		assert_eq!(all_numbers.get("app2"), Some(&1));
	}

	#[rstest]
	#[test]
	fn test_zero_padding() {
		// Arrange
		let temp_dir = tempfile::tempdir().unwrap();
		let migrations_dir = temp_dir.path().join("migrations");
		let app_dir = migrations_dir.join("myapp");
		fs::create_dir_all(&app_dir).unwrap();
		fs::write(app_dir.join("0099_test.rs"), "").unwrap();

		// Act
		let next = MigrationNumbering::next_number(&migrations_dir, "myapp");

		// Assert
		assert_eq!(next, "0100");
	}
}
