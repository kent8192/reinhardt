//! Path safety utilities for preventing directory traversal attacks
//!
//! Provides functions to safely join user-controlled path components
//! with base directories, preventing path traversal vulnerabilities.

use std::path::{Component, Path, PathBuf};

/// Errors that can occur during safe path operations
#[derive(Debug, thiserror::Error)]
pub enum PathTraversalError {
	#[error("Path traversal detected: input contains parent directory reference")]
	ParentTraversal,
	#[error("Absolute path not allowed in user input")]
	AbsolutePath,
	#[error("Path escapes base directory")]
	EscapesBase,
	#[error("Path contains null byte")]
	NullByte,
	#[error("IO error during path resolution: {0}")]
	Io(#[from] std::io::Error),
}

/// Safely join a base directory with user-provided input, preventing path traversal.
///
/// This function implements a 3-stage defense:
/// 1. Reject `..` components, absolute paths, and null bytes
/// 2. Canonicalize both base and joined paths
/// 3. Verify the result is contained within the base directory
///
/// For non-existent paths, component-by-component resolution is used
/// to canonicalize the existing ancestor and append remaining components.
///
/// # Errors
///
/// Returns `PathTraversalError` if:
/// - The user input contains `..` path components
/// - The user input is an absolute path
/// - The user input contains null bytes
/// - The resolved path escapes the base directory
/// - An IO error occurs during canonicalization
pub fn safe_path_join(base: &Path, user_input: &str) -> Result<PathBuf, PathTraversalError> {
	// Stage 1: Input validation

	// Reject null bytes
	if user_input.contains('\0') {
		return Err(PathTraversalError::NullByte);
	}

	// Reject absolute paths
	if user_input.starts_with('/') || user_input.starts_with('\\') {
		return Err(PathTraversalError::AbsolutePath);
	}

	// On Windows, reject drive-letter absolute paths
	if user_input.len() >= 2
		&& user_input.as_bytes()[0].is_ascii_alphabetic()
		&& user_input.as_bytes()[1] == b':'
	{
		return Err(PathTraversalError::AbsolutePath);
	}

	// Reject any component that is `..` using std::path::Component analysis
	let input_path = Path::new(user_input);
	for component in input_path.components() {
		if matches!(component, Component::ParentDir) {
			return Err(PathTraversalError::ParentTraversal);
		}
	}

	// Also catch encoded or obfuscated `..` that Component might normalize away
	if user_input.contains("..") {
		return Err(PathTraversalError::ParentTraversal);
	}

	// Stage 2: Join and canonicalize
	let joined = base.join(user_input);
	let canonical_base = safe_canonicalize(base)?;
	let canonical_joined = safe_canonicalize(&joined)?;

	// Stage 3: Containment verification
	if !canonical_joined.starts_with(&canonical_base) {
		return Err(PathTraversalError::EscapesBase);
	}

	Ok(canonical_joined)
}

/// Canonicalize a path, handling non-existent paths by resolving existing ancestors
/// and appending remaining (non-existent) components.
fn safe_canonicalize(path: &Path) -> Result<PathBuf, PathTraversalError> {
	// Try direct canonicalization first (works for existing paths)
	if let Ok(canonical) = path.canonicalize() {
		return Ok(canonical);
	}

	// For non-existent paths, find the deepest existing ancestor
	let mut remaining = Vec::new();
	let mut current = path.to_path_buf();

	let resolved = loop {
		if current.exists() {
			break current.canonicalize()?;
		}
		if let Some(file_name) = current.file_name() {
			remaining.push(file_name.to_os_string());
			if let Some(parent) = current.parent() {
				current = parent.to_path_buf();
			} else {
				// Reached root without finding existing ancestor, use path as-is
				break current;
			}
		} else {
			break current;
		}
	};

	// Append non-existent components (in original order)
	let mut result = resolved;
	for component in remaining.into_iter().rev() {
		result.push(component);
	}

	Ok(result)
}

/// Validate that a string contains only safe characters for use as a filename component.
///
/// Allows only alphanumeric characters, hyphens, underscores, and dots.
/// Rejects path separators, null bytes, and any other special characters.
pub fn is_safe_filename_component(input: &str) -> bool {
	!input.is_empty()
		&& !input.contains('\0')
		&& !input.contains('/')
		&& !input.contains('\\')
		&& !input.contains("..")
		&& input
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs;

	/// Create a temporary test directory and return its path
	fn create_test_dir() -> PathBuf {
		let dir = PathBuf::from(format!(
			"/tmp/reinhardt_path_safety_test_{}",
			uuid::Uuid::new_v4()
		));
		fs::create_dir_all(&dir).expect("Failed to create test directory");
		dir
	}

	/// Cleanup test directory
	fn cleanup_test_dir(dir: &Path) {
		if dir.exists() {
			let _ = fs::remove_dir_all(dir);
		}
	}

	// ===================================================================
	// safe_path_join tests
	// ===================================================================

	#[rstest]
	fn test_safe_path_join_normal_path() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "subdir/file.txt");

		// Assert
		assert!(result.is_ok());
		let resolved = result.unwrap();
		assert!(resolved.starts_with(base.canonicalize().unwrap()));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_parent_traversal() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "../../../etc/passwd");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::ParentTraversal)));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_embedded_traversal() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "foo/../../bar");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::ParentTraversal)));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_absolute_path() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "/etc/passwd");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::AbsolutePath)));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_null_byte() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "foo\0/../bar");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::NullByte)));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_double_dot_in_component() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "..hidden");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::ParentTraversal)));
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_allows_single_dot() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "./file.txt");

		// Assert
		assert!(result.is_ok());
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_allows_dotfiles() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, ".gitignore");

		// Assert
		assert!(result.is_ok());
		cleanup_test_dir(&base);
	}

	#[rstest]
	fn test_safe_path_join_rejects_backslash_absolute() {
		// Arrange
		let base = create_test_dir();

		// Act
		let result = safe_path_join(&base, "\\etc\\passwd");

		// Assert
		assert!(matches!(result, Err(PathTraversalError::AbsolutePath)));
		cleanup_test_dir(&base);
	}

	// ===================================================================
	// is_safe_filename_component tests
	// ===================================================================

	#[rstest]
	#[case("valid_filename", true)]
	#[case("file.txt", true)]
	#[case("my-file-123", true)]
	#[case("../etc/passwd", false)]
	#[case("/absolute", false)]
	#[case("has space", false)]
	#[case("", false)]
	#[case("null\0byte", false)]
	#[case("path/sep", false)]
	#[case("back\\slash", false)]
	#[case("..", false)]
	fn test_is_safe_filename_component(#[case] input: &str, #[case] expected: bool) {
		// Act
		let result = is_safe_filename_component(input);

		// Assert
		assert_eq!(result, expected, "Failed for input: {:?}", input);
	}
}
