//! Test fixtures for storage backend tests.
//!
//! This module provides reusable fixtures for testing storage components.
//! All fixtures are designed to work with rstest and can be composed together.

#![allow(dead_code)]
#![allow(unreachable_pub)]

use aws_config::Region;
use aws_sdk_s3::config::Credentials;
use reinhardt_storages::{StorageBackend, StorageConfig};
use rstest::fixture;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use testcontainers::{
	ContainerAsync, GenericImage,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

// ============================================================================
// Test Data (inline to avoid super::utils dependency issues)
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
}

/// Generate text content with specified number of lines.
fn generate_text_content(lines: usize) -> String {
	(0..lines)
		.map(|i| format!("Line {}: test content", i))
		.collect::<Vec<_>>()
		.join("\n")
}

/// Generate binary content containing all byte values.
fn generate_binary_content() -> Vec<u8> {
	(0u8..=255).collect::<Vec<_>>()
}

/// Generate unique file name with prefix.
pub fn generate_unique_name(prefix: &str) -> String {
	format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

/// Create test file with text content.
fn create_text_file(name: String, lines: usize) -> TestFile {
	let content = generate_text_content(lines);
	TestFile::new(name, content.into_bytes())
}

/// Create test file with binary content.
fn create_binary_file(name: String) -> TestFile {
	let content = generate_binary_content();
	TestFile::new(name, content)
}

// ============================================================================
// Common Fixtures
// ============================================================================

/// Empty file fixture (0 bytes).
#[fixture]
pub fn empty_file() -> TestFile {
	TestFile::new("empty.txt".to_string(), vec![])
}

/// Small file fixture (< 1KB).
#[fixture]
pub fn small_file() -> TestFile {
	create_text_file("small.txt".to_string(), 10)
}

/// Medium file fixture (around 1KB).
#[fixture]
pub fn medium_file() -> TestFile {
	create_text_file("medium.txt".to_string(), 100)
}

/// Large file fixture (around 100KB).
#[fixture]
pub fn large_file() -> TestFile {
	create_text_file("large.txt".to_string(), 10000)
}

/// Binary file fixture with all byte values.
#[fixture]
pub fn binary_file() -> TestFile {
	create_binary_file("binary.bin".to_string())
}

/// Collection of test files with varying sizes.
#[fixture]
pub fn test_files() -> Vec<TestFile> {
	vec![
		TestFile::new("empty.txt".to_string(), vec![]),
		create_text_file("small.txt".to_string(), 10),
		create_text_file("medium.txt".to_string(), 100),
		create_binary_file("binary.bin".to_string()),
	]
}

/// Unique file name fixture.
#[fixture]
pub fn unique_file_name() -> String {
	generate_unique_name("test")
}

// ============================================================================
// Local Storage Fixtures
// ============================================================================

/// Test directory wrapper for local storage.
pub struct LocalTestDir {
	temp_dir: TempDir,
	backend: Arc<dyn StorageBackend>,
}

impl LocalTestDir {
	/// Create a new test directory with storage backend.
	pub async fn new() -> Self {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = reinhardt_storages::create_storage(config)
			.await
			.expect("Failed to create local backend");

		Self { temp_dir, backend }
	}

	/// Get the storage backend.
	pub fn backend(&self) -> Arc<dyn StorageBackend> {
		Arc::clone(&self.backend)
	}

	/// Get the temp directory path.
	pub fn path(&self) -> &Path {
		self.temp_dir.path()
	}
}

/// Local storage backend fixture.
///
/// Note: This fixture uses `keep()` to prevent automatic cleanup when TempDir
/// goes out of scope. The temp directory will persist after the test.
/// This is acceptable for test code since OS cleans up temp directories.
#[fixture]
pub async fn local_backend() -> Arc<dyn StorageBackend> {
	// Use keep() to prevent automatic cleanup when TempDir is dropped.
	// This ensures the directory stays alive for the entire test.
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path_str = temp_dir.path().to_str().unwrap().to_string();
	let _ = temp_dir.keep();

	let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig {
		base_path: base_path_str,
	});

	reinhardt_storages::create_storage(config)
		.await
		.expect("Failed to create local backend")
}

/// Local test directory fixture.
#[fixture]
pub async fn local_temp_dir() -> LocalTestDir {
	LocalTestDir::new().await
}

// ============================================================================
// S3 Storage Fixtures (using LocalStack)
// ============================================================================

/// S3 test container wrapper.
pub struct S3TestContainer {
	/// LocalStack container
	#[allow(dead_code)]
	container: ContainerAsync<GenericImage>,
	/// S3 endpoint URL
	pub endpoint: String,
	/// S3 bucket name
	pub bucket: String,
	/// AWS region
	pub region: String,
}

impl S3TestContainer {
	/// Create a new S3 test container with LocalStack.
	///
	/// This starts a LocalStack container and creates a test bucket.
	pub async fn new() -> Self {
		// Use LocalStack image
		let container = GenericImage::new("localstack/localstack", "latest")
			.with_exposed_port(4566.tcp())
			.with_wait_for(WaitFor::message_on_stdout("Ready."))
			.start()
			.await
			.expect("Failed to start LocalStack container");

		let port = container
			.get_host_port_ipv4(4566)
			.await
			.expect("Failed to get host port");
		let endpoint = format!("http://localhost:{}", port);
		let bucket = "test-bucket".to_string();
		let region = "us-east-1".to_string();

		// Create the bucket using AWS SDK
		let credentials = Credentials::new("test", "test", None, None, "test");
		let s3_config = aws_sdk_s3::Config::builder()
			.behavior_version_latest()
			.region(Region::new(region.clone()))
			.endpoint_url(&endpoint)
			.credentials_provider(credentials)
			.force_path_style(true)
			.build();
		let s3_client = aws_sdk_s3::Client::from_conf(s3_config);

		s3_client
			.create_bucket()
			.bucket(&bucket)
			.send()
			.await
			.expect("Failed to create test bucket");

		Self {
			container,
			endpoint,
			bucket,
			region,
		}
	}

	/// Get the S3 endpoint URL.
	pub fn endpoint(&self) -> &str {
		&self.endpoint
	}

	/// Get the bucket name.
	pub fn bucket(&self) -> &str {
		&self.bucket
	}

	/// Get the region.
	pub fn region(&self) -> &str {
		&self.region
	}

	/// Create S3 storage backend from this container.
	pub async fn create_backend(&self) -> Arc<dyn StorageBackend> {
		// SAFETY: Setting environment variables for test-only AWS credentials.
		// These tests run serially and the env vars are required by aws-config
		// defaults() credential chain used in S3Storage::new().
		unsafe {
			std::env::set_var("AWS_ACCESS_KEY_ID", "test");
			std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
		}

		let config = StorageConfig::S3(reinhardt_storages::config::S3Config {
			bucket: self.bucket.clone(),
			region: Some(self.region.clone()),
			endpoint: Some(self.endpoint.clone()),
			prefix: None,
		});

		reinhardt_storages::create_storage(config)
			.await
			.expect("Failed to create S3 backend")
	}

	/// Create S3 storage backend with prefix from this container.
	pub async fn create_backend_with_prefix(&self, prefix: &str) -> Arc<dyn StorageBackend> {
		// SAFETY: Setting environment variables for test-only AWS credentials.
		// These tests run serially and the env vars are required by aws-config
		// defaults() credential chain used in S3Storage::new().
		unsafe {
			std::env::set_var("AWS_ACCESS_KEY_ID", "test");
			std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
		}

		let config = StorageConfig::S3(reinhardt_storages::config::S3Config {
			bucket: self.bucket.clone(),
			region: Some(self.region.clone()),
			endpoint: Some(self.endpoint.clone()),
			prefix: Some(prefix.to_string()),
		});

		reinhardt_storages::create_storage(config)
			.await
			.expect("Failed to create S3 backend with prefix")
	}
}

/// S3 backend fixture using LocalStack container.
///
/// This fixture creates a new LocalStack container for each test.
/// Note: The container is leaked to keep it alive for the duration of the test.
#[fixture]
pub async fn s3_backend() -> Arc<dyn StorageBackend> {
	// Leak the container to keep it alive for the entire test.
	// Without this, the container would be stopped when it goes out of scope,
	// causing network errors when the backend tries to communicate with S3.
	let container = Box::leak(Box::new(S3TestContainer::new().await));
	container.create_backend().await
}

/// S3 backend fixture with path prefix.
///
/// Note: The container is leaked to keep it alive for the duration of the test.
#[fixture]
pub async fn s3_backend_with_prefix() -> Arc<dyn StorageBackend> {
	// Leak the container to keep it alive for the entire test.
	let container = Box::leak(Box::new(S3TestContainer::new().await));
	container.create_backend_with_prefix("test-prefix").await
}

/// S3 test container fixture.
#[fixture]
pub async fn s3_container() -> S3TestContainer {
	S3TestContainer::new().await
}

// ============================================================================
// Tests for Fixtures
// ============================================================================

#[cfg(test)]
mod tests {
	use super::*;

	// Common fixture tests
	#[test]
	fn test_empty_file_fixture() {
		let file = empty_file();
		assert_eq!(file.name, "empty.txt");
		assert_eq!(file.size, 0);
		assert!(file.content.is_empty());
	}

	#[test]
	fn test_small_file_fixture() {
		let file = small_file();
		assert_eq!(file.name, "small.txt");
		assert!(file.size > 0);
		assert!(file.size < 1024);
	}

	#[test]
	fn test_medium_file_fixture() {
		let file = medium_file();
		assert_eq!(file.name, "medium.txt");
		assert!(file.size > 100);
	}

	#[test]
	fn test_large_file_fixture() {
		let file = large_file();
		assert_eq!(file.name, "large.txt");
		assert!(file.size > 10000);
	}

	#[test]
	fn test_binary_file_fixture() {
		let file = binary_file();
		assert_eq!(file.name, "binary.bin");
		assert_eq!(file.size, 256); // All byte values
	}

	#[test]
	fn test_test_files_fixture() {
		let files = test_files();
		assert_eq!(files.len(), 4);
		assert_eq!(files[0].name, "empty.txt");
		assert_eq!(files[1].name, "small.txt");
		assert_eq!(files[2].name, "medium.txt");
		assert_eq!(files[3].name, "binary.bin");
	}

	#[test]
	fn test_unique_file_name_fixture() {
		let name1 = unique_file_name();
		let name2 = unique_file_name();
		assert_ne!(name1, name2);
		assert!(name1.starts_with("test-"));
	}

	// Local fixture tests
	#[tokio::test]
	async fn test_local_test_dir_creation() {
		let test_dir = LocalTestDir::new().await;
		assert!(test_dir.path().exists());
		assert!(test_dir.path().is_dir());
	}

	#[tokio::test]
	async fn test_local_backend_fixture() {
		let backend = local_backend().await;
		// Just verify it creates successfully
		drop(backend);
	}

	#[tokio::test]
	async fn test_local_backend_save_and_read() {
		let backend = local_backend().await;

		backend
			.save("test.txt", b"Hello, world!")
			.await
			.expect("Failed to save");

		let content = backend.open("test.txt").await.expect("Failed to read");

		assert_eq!(content, b"Hello, world!");
	}

	// S3 fixture tests
	#[tokio::test]
	async fn test_s3_container_creation() {
		let container = S3TestContainer::new().await;
		assert!(container.endpoint().contains("localhost"));
		assert_eq!(container.bucket(), "test-bucket");
		assert_eq!(container.region(), "us-east-1");
	}

	#[tokio::test]
	async fn test_s3_backend_fixture() {
		let backend = s3_backend().await;

		// Test basic save and read
		backend
			.save("test.txt", b"Hello, S3!")
			.await
			.expect("Failed to save");

		let content = backend.open("test.txt").await.expect("Failed to read");

		assert_eq!(content, b"Hello, S3!");

		// Cleanup
		backend.delete("test.txt").await.ok();
	}

	#[tokio::test]
	async fn test_s3_backend_with_prefix_fixture() {
		let backend = s3_backend_with_prefix().await;

		// Test that prefix is applied
		let path = backend
			.save("test.txt", b"Hello, S3 with prefix!")
			.await
			.expect("Failed to save");

		assert!(path.contains("test-prefix"));
		assert!(path.contains("test.txt"));

		// Cleanup
		backend.delete("test.txt").await.ok();
	}
}
