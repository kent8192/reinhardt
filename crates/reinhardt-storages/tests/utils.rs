//! Test utility modules for storage backend tests.
//!
//! This module provides test data generation utilities and custom assertions.

#![allow(dead_code)]
#![allow(unreachable_pub)]

use rand::Rng;
use reinhardt_storages::{StorageBackend, StorageError};
use std::fmt;
use std::path::Path;

// ============================================================================
// Test Data
// ============================================================================

/// Test file structure.
#[derive(Debug, Clone)]
pub struct TestFile {
	pub name: String,
	pub content: Vec<u8>,
	pub size: usize,
}

impl TestFile {
	/// Create a new test file.
	pub fn new(name: String, content: Vec<u8>) -> Self {
		let size = content.len();
		Self {
			name,
			content,
			size,
		}
	}

	/// Get the file extension.
	pub fn extension(&self) -> Option<&str> {
		Path::new(&self.name).extension().and_then(|e| e.to_str())
	}
}

/// Generate random bytes of specified size.
pub fn generate_random_bytes(size: usize) -> Vec<u8> {
	let mut bytes = vec![0u8; size];
	rand::thread_rng().fill(&mut bytes[..]);
	bytes
}

/// Generate text content with specified number of lines.
pub fn generate_text_content(lines: usize) -> String {
	(0..lines)
		.map(|i| format!("Line {}: {}", i, "test content".repeat(10)))
		.collect::<Vec<_>>()
		.join("\n")
}

/// Generate binary content containing all byte values.
pub fn generate_binary_content() -> Vec<u8> {
	(0u8..=255).collect::<Vec<_>>()
}

/// Generate unique file name with prefix.
pub fn generate_unique_name(prefix: &str) -> String {
	format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

/// Generate nested path with specified depth.
pub fn generate_nested_path(depth: usize, file_name: &str) -> String {
	let parts: Vec<String> = (0..depth).map(|i| format!("level{}", i)).collect();
	format!("{}/{}", parts.join("/"), file_name)
}

/// Create test file with random content.
pub fn create_test_file(name: String, size: usize) -> TestFile {
	let content = generate_random_bytes(size);
	TestFile::new(name, content)
}

/// Create test file with text content.
pub fn create_text_file(name: String, lines: usize) -> TestFile {
	let content = generate_text_content(lines);
	TestFile::new(name, content.into_bytes())
}

/// Create test file with binary content.
pub fn create_binary_file(name: String) -> TestFile {
	let content = generate_binary_content();
	TestFile::new(name, content)
}

// ============================================================================
// Custom Assertions
// ============================================================================

/// Custom assertion error.
#[derive(Debug)]
pub struct AssertionError {
	pub message: String,
}

impl fmt::Display for AssertionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.message)
	}
}

impl std::error::Error for AssertionError {}

impl AssertionError {
	/// Create a new assertion error.
	pub fn new(message: String) -> Self {
		Self { message }
	}
}

/// Assert that a file exists in storage.
pub async fn assert_storage_exists(
	storage: &dyn StorageBackend,
	name: &str,
) -> Result<(), AssertionError> {
	let exists = storage
		.exists(name)
		.await
		.map_err(|e| AssertionError::new(format!("Failed to check existence: {}", e)))?;

	if !exists {
		return Err(AssertionError::new(format!("File should exist: {}", name)));
	}

	Ok(())
}

/// Assert that a file does NOT exist in storage.
pub async fn assert_storage_not_exists(
	storage: &dyn StorageBackend,
	name: &str,
) -> Result<(), AssertionError> {
	let exists = storage
		.exists(name)
		.await
		.map_err(|e| AssertionError::new(format!("Failed to check existence: {}", e)))?;

	if exists {
		return Err(AssertionError::new(format!(
			"File should not exist: {}",
			name
		)));
	}

	Ok(())
}

/// Assert that file content matches expected value.
pub async fn assert_content_matches(
	storage: &dyn StorageBackend,
	name: &str,
	expected: &[u8],
) -> Result<(), AssertionError> {
	let actual = storage
		.open(name)
		.await
		.map_err(|e| AssertionError::new(format!("Failed to open file: {}", e)))?;

	if actual != expected {
		return Err(AssertionError::new(format!(
			"Content mismatch for file: {} (expected {} bytes, got {} bytes)",
			name,
			expected.len(),
			actual.len()
		)));
	}

	Ok(())
}

/// Assert that presigned URL is valid.
pub fn assert_presigned_url(url: &str) -> Result<(), AssertionError> {
	if !url.starts_with("http://") && !url.starts_with("https://") {
		return Err(AssertionError::new(format!(
			"Presigned URL should start with http:// or https://: {}",
			url
		)));
	}

	// Check for AWS signature query params
	if !url.contains("X-Amz") {
		return Err(AssertionError::new(format!(
			"Presigned URL should contain AWS signature params: {}",
			url
		)));
	}

	Ok(())
}

/// Assert that local file URL is valid.
pub fn assert_file_url(url: &str) -> Result<(), AssertionError> {
	if !url.starts_with("file://") {
		return Err(AssertionError::new(format!(
			"Local URL should start with file://: {}",
			url
		)));
	}

	Ok(())
}

/// Assert that file size matches expected value.
pub async fn assert_file_size(
	storage: &dyn StorageBackend,
	name: &str,
	expected: u64,
) -> Result<(), AssertionError> {
	let actual = storage
		.size(name)
		.await
		.map_err(|e| AssertionError::new(format!("Failed to get file size: {}", e)))?;

	if actual != expected {
		return Err(AssertionError::new(format!(
			"Size mismatch for file: {} (expected {} bytes, got {} bytes)",
			name, expected, actual
		)));
	}

	Ok(())
}

/// Assert that operation returns NotFound error.
pub async fn assert_not_found<F, Fut>(f: F) -> Result<(), AssertionError>
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future<Output = Result<(), StorageError>>,
{
	match f().await {
		Err(StorageError::NotFound(msg)) => {
			if !msg.is_empty() {
				Ok(())
			} else {
				Err(AssertionError::new(
					"NotFound error should have a message".to_string(),
				))
			}
		}
		Ok(_) => Err(AssertionError::new(
			"Expected NotFound error, but operation succeeded".to_string(),
		)),
		Err(e) => Err(AssertionError::new(format!(
			"Expected NotFound error, got: {:?}",
			e
		))),
	}
}

/// Assert that operation returns PermissionDenied error.
pub async fn assert_permission_denied<F, Fut>(f: F) -> Result<(), AssertionError>
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future<Output = Result<(), StorageError>>,
{
	match f().await {
		Err(StorageError::PermissionDenied(msg)) => {
			if !msg.is_empty() {
				Ok(())
			} else {
				Err(AssertionError::new(
					"PermissionDenied error should have a message".to_string(),
				))
			}
		}
		Ok(_) => Err(AssertionError::new(
			"Expected PermissionDenied error, but operation succeeded".to_string(),
		)),
		Err(e) => Err(AssertionError::new(format!(
			"Expected PermissionDenied error, got: {:?}",
			e
		))),
	}
}

/// Assert that operation returns ConfigError error.
pub async fn assert_config_error<F, Fut>(f: F) -> Result<(), AssertionError>
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future<Output = Result<(), StorageError>>,
{
	match f().await {
		Err(StorageError::ConfigError(msg)) => {
			if !msg.is_empty() {
				Ok(())
			} else {
				Err(AssertionError::new(
					"ConfigError should have a message".to_string(),
				))
			}
		}
		Ok(_) => Err(AssertionError::new(
			"Expected ConfigError, but operation succeeded".to_string(),
		)),
		Err(e) => Err(AssertionError::new(format!(
			"Expected ConfigError, got: {:?}",
			e
		))),
	}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
	use super::*;

	// Test data tests
	#[test]
	fn test_generate_random_bytes() {
		let bytes = generate_random_bytes(100);
		assert_eq!(bytes.len(), 100);
	}

	#[test]
	fn test_generate_text_content() {
		let content = generate_text_content(5);
		let lines: Vec<&str> = content.lines().collect();
		assert_eq!(lines.len(), 5);
	}

	#[test]
	fn test_generate_binary_content() {
		let content = generate_binary_content();
		assert_eq!(content.len(), 256);
		assert_eq!(content[0], 0);
		assert_eq!(content[255], 255);
	}

	#[test]
	fn test_generate_unique_name() {
		let name1 = generate_unique_name("test");
		let name2 = generate_unique_name("test");
		assert_ne!(name1, name2);
		assert!(name1.starts_with("test-"));
		assert!(name2.starts_with("test-"));
	}

	#[test]
	fn test_generate_nested_path() {
		let path = generate_nested_path(3, "file.txt");
		assert_eq!(path, "level0/level1/level2/file.txt");
	}

	#[test]
	fn test_test_file_extension() {
		let file = TestFile::new("test.txt".to_string(), vec![]);
		assert_eq!(file.extension(), Some("txt"));

		let file2 = TestFile::new("test".to_string(), vec![]);
		assert_eq!(file2.extension(), None);

		let file3 = TestFile::new("path/to/file.json".to_string(), vec![]);
		assert_eq!(file3.extension(), Some("json"));
	}

	#[test]
	fn test_create_test_file() {
		let file = create_test_file("test.bin".to_string(), 1000);
		assert_eq!(file.name, "test.bin");
		assert_eq!(file.size, 1000);
		assert_eq!(file.content.len(), 1000);
	}

	#[test]
	fn test_create_text_file() {
		let file = create_text_file("test.txt".to_string(), 10);
		assert_eq!(file.name, "test.txt");
		assert!(file.size > 0);
		let content = String::from_utf8(file.content).unwrap();
		assert!(content.lines().count() >= 10);
	}

	#[test]
	fn test_create_binary_file() {
		let file = create_binary_file("binary.bin".to_string());
		assert_eq!(file.name, "binary.bin");
		assert_eq!(file.size, 256);
	}

	// Assertion tests
	#[test]
	fn test_assert_presigned_url_valid() {
		let url = "https://bucket.s3.amazonaws.com/key?X-Amz-Algorithm=AWS4-HMAC-SHA256";
		assert!(assert_presigned_url(url).is_ok());
	}

	#[test]
	fn test_assert_presigned_url_invalid() {
		let url = "ftp://invalid.com";
		assert!(assert_presigned_url(url).is_err());
	}

	#[test]
	fn test_assert_file_url_valid() {
		let url = "file:///path/to/file.txt";
		assert!(assert_file_url(url).is_ok());
	}

	#[test]
	fn test_assert_file_url_invalid() {
		let url = "http://example.com";
		assert!(assert_file_url(url).is_err());
	}

	#[tokio::test]
	async fn test_assert_config_error_message() {
		// Test with a function that returns ConfigError
		async fn returns_config_error() -> Result<(), StorageError> {
			Err(StorageError::ConfigError("test error".to_string()))
		}

		let result = assert_config_error(returns_config_error).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_assert_config_error_empty_message() {
		async fn returns_empty_config_error() -> Result<(), StorageError> {
			Err(StorageError::ConfigError(String::new()))
		}

		let result = assert_config_error(returns_empty_config_error).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_assert_not_found_error() {
		async fn returns_not_found() -> Result<(), StorageError> {
			Err(StorageError::NotFound("test.txt".to_string()))
		}

		let result = assert_not_found(returns_not_found).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_assert_permission_denied_error() {
		async fn returns_permission_denied() -> Result<(), StorageError> {
			Err(StorageError::PermissionDenied("access denied".to_string()))
		}

		let result = assert_permission_denied(returns_permission_denied).await;
		assert!(result.is_ok());
	}
}
