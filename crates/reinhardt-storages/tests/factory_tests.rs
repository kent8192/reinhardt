//! Factory pattern tests for creating storage backends.

use reinhardt_storages::{StorageBackend, StorageConfig, StorageError, create_storage};
use rstest::rstest;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// Backend Creation Tests
// ============================================================================

mod backend_creation_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_create_local_backend() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = create_storage(config)
			.await
			.expect("Failed to create local backend");

		// Verify backend works
		backend
			.save("test.txt", b"Hello, factory!")
			.await
			.expect("Failed to save");

		let content = backend.open("test.txt").await.expect("Failed to read");

		assert_eq!(content, b"Hello, factory!");

		// Cleanup
		backend.delete("test.txt").await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_backend_returns_arc() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend: Arc<dyn StorageBackend> = create_storage(config)
			.await
			.expect("Failed to create backend");

		// Arc should be cloneable
		let backend_clone = Arc::clone(&backend);

		// Both clones should work
		backend
			.save("test1.txt", b"content1")
			.await
			.expect("Failed to save");

		backend_clone
			.save("test2.txt", b"content2")
			.await
			.expect("Failed to save");

		// Cleanup
		backend.delete("test1.txt").await.ok();
		backend.delete("test2.txt").await.ok();
	}

	#[rstest]
	#[tokio::test]
	#[cfg(feature = "s3")]
	async fn test_create_s3_backend() {
		let config = StorageConfig::S3(reinhardt_storages::config::S3Config {
			bucket: "test-bucket".to_string(),
			region: Some("us-east-1".to_string()),
			endpoint: Some("http://localhost:4566".to_string()),
			prefix: None,
		});

		// This will fail without LocalStack running, but we're testing the factory pattern
		let result = create_storage(config).await;
		// We expect either success or network error (not config error)
		match result {
			Ok(_) => {
				// Success - backend created
			}
			Err(StorageError::NetworkError(_)) => {
				// Expected if LocalStack is not running
			}
			Err(StorageError::ConfigError(_)) => {
				panic!("Should not get ConfigError for valid config");
			}
			Err(e) => {
				panic!("Unexpected error: {:?}", e);
			}
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_invalid_base_path_error() {
		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig {
			base_path: "/nonexistent/path/that/does/not/exist".to_string(),
		});

		let result = create_storage(config).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::ConfigError(_))));
	}

	#[rstest]
	#[tokio::test]
	#[cfg(feature = "s3")]
	async fn test_s3_config_with_prefix() {
		let config = StorageConfig::S3(reinhardt_storages::config::S3Config {
			bucket: "test-bucket".to_string(),
			region: None,
			endpoint: Some("http://localhost:4566".to_string()),
			prefix: Some("test-prefix".to_string()),
		});

		let result = create_storage(config).await;
		// We expect either success or network error
		match result {
			Ok(_) => {
				// Success
			}
			Err(StorageError::NetworkError(_)) => {
				// Expected if LocalStack is not running
			}
			Err(e) => {
				panic!("Unexpected error: {:?}", e);
			}
		}
	}
}

// ============================================================================
// Arc Behavior Tests
// ============================================================================

mod arc_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_backend_is_send() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = create_storage(config)
			.await
			.expect("Failed to create backend");

		// This test just verifies that Arc<dyn StorageBackend> is Send
		// by moving it into another async block
		let handle = tokio::spawn(async move {
			backend
				.save("test.txt", b"Send test")
				.await
				.expect("Failed to save");
			backend
		});

		let backend = handle.await.expect("Task failed");

		// Cleanup
		backend.delete("test.txt").await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_backend_is_sync() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = create_storage(config)
			.await
			.expect("Failed to create backend");

		// This test verifies that Arc<dyn StorageBackend> is Sync
		// by sharing references across threads
		let backend1 = Arc::clone(&backend);
		let backend2 = Arc::clone(&backend);

		let handle1 = tokio::spawn(async move {
			backend1
				.save("test1.txt", b"Sync test 1")
				.await
				.expect("Failed to save");
		});

		let handle2 = tokio::spawn(async move {
			backend2
				.save("test2.txt", b"Sync test 2")
				.await
				.expect("Failed to save");
		});

		handle1.await.expect("Task 1 failed");
		handle2.await.expect("Task 2 failed");

		// Verify both files were written
		assert!(backend.exists("test1.txt").await.expect("Failed to check"));
		assert!(backend.exists("test2.txt").await.expect("Failed to check"));

		// Cleanup
		backend.delete("test1.txt").await.ok();
		backend.delete("test2.txt").await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_arc_clone_independence() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = create_storage(config)
			.await
			.expect("Failed to create backend");

		let clone1 = Arc::clone(&backend);
		let clone2 = Arc::clone(&backend);

		// Save using different clones
		clone1
			.save("file1.txt", b"Clone 1")
			.await
			.expect("Failed to save");
		clone2
			.save("file2.txt", b"Clone 2")
			.await
			.expect("Failed to save");

		// All clones should see the same files
		let content1 = backend.open("file1.txt").await.expect("Failed to read");
		let content2 = clone1.open("file2.txt").await.expect("Failed to read");

		assert_eq!(content1, b"Clone 1");
		assert_eq!(content2, b"Clone 2");

		// Cleanup
		backend.delete("file1.txt").await.ok();
		backend.delete("file2.txt").await.ok();
	}
}

// ============================================================================
// Feature Gate Tests
// ============================================================================

#[cfg(feature = "local")]
mod local_feature_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_local_feature_enabled() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let result = create_storage(config).await;
		assert!(result.is_ok(), "Local feature should be enabled");
	}
}

#[cfg(feature = "s3")]
mod s3_feature_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_s3_feature_enabled() {
		let config = StorageConfig::S3(reinhardt_storages::config::S3Config {
			bucket: "test-bucket".to_string(),
			region: None,
			endpoint: Some("http://localhost:4566".to_string()),
			prefix: None,
		});

		// Should not fail at config level (may fail at connection level)
		let result = create_storage(config).await;
		match result {
			Ok(_) => {
				// Success
			}
			Err(StorageError::NetworkError(_)) => {
				// Expected if LocalStack is not running
			}
			Err(StorageError::ConfigError(_)) => {
				panic!("Should not get ConfigError - S3 feature should be enabled");
			}
			Err(e) => {
				panic!("Unexpected error: {:?}", e);
			}
		}
	}
}

// ============================================================================
// Factory Error Tests
// ============================================================================

mod factory_error_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_factory_with_nonexistent_directory() {
		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig {
			base_path: "/this/path/definitely/does/not/exist".to_string(),
		});

		let result = create_storage(config).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::ConfigError(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_factory_with_file_instead_of_directory() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let file_path = temp_dir.path().join("not_a_dir.txt");

		// Create a file instead of directory
		std::fs::write(&file_path, b"test").expect("Failed to create file");

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig {
			base_path: file_path.to_str().unwrap().to_string(),
		});

		let result = create_storage(config).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::ConfigError(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_factory_creates_working_backend() {
		let temp_dir = TempDir::new().expect("Failed to create temp dir");
		let base_path = temp_dir.path().to_str().unwrap().to_string();

		let config = StorageConfig::Local(reinhardt_storages::config::LocalConfig { base_path });

		let backend = create_storage(config)
			.await
			.expect("Failed to create backend");

		// Test all operations work
		let test_cases: Vec<(&str, &[u8])> = vec![
			("file1.txt", b"content1"),
			("file2.txt", b"content2"),
			("path/to/file3.txt", b"content3"),
		];

		for (name, content) in &test_cases {
			backend.save(name, *content).await.expect("Failed to save");

			let read_content = backend.open(name).await.expect("Failed to read");
			assert_eq!(read_content, *content);

			let exists = backend.exists(name).await.expect("Failed to check");
			assert!(exists);

			let size = backend.size(name).await.expect("Failed to get size");
			assert_eq!(size, content.len() as u64);
		}

		// Cleanup
		for (name, _) in &test_cases {
			backend.delete(name).await.ok();
		}
	}
}
