//! Integration tests for S3 storage backend using LocalStack.

mod fixtures;
mod utils;

use fixtures::{
	S3TestContainer, generate_unique_name, s3_backend, s3_backend_with_prefix, s3_container,
};
use reinhardt_storages::{StorageBackend, StorageError};
use rstest::rstest;
use std::sync::Arc;
use utils::{assert_presigned_url, assert_storage_exists, assert_storage_not_exists};

// ============================================================================
// CRUD Tests
// ============================================================================

mod crud_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_save_file(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_save.txt";
		let content = b"Hello, S3!";

		let path = s3_backend
			.save(name, content)
			.await
			.expect("Failed to save file");

		assert_eq!(path, name);
		assert_storage_exists(&*s3_backend, name)
			.await
			.expect("File should exist");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_open_file(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_open.txt";
		let content = b"Hello, S3 Storage!";

		s3_backend
			.save(name, content)
			.await
			.expect("Failed to save file");

		let read_content = s3_backend.open(name).await.expect("Failed to open file");

		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete_file(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_delete.txt";
		s3_backend
			.save(name, b"temporary")
			.await
			.expect("Failed to save");

		assert_storage_exists(&*s3_backend, name)
			.await
			.expect("File should exist before delete");

		s3_backend
			.delete(name)
			.await
			.expect("Failed to delete file");

		assert_storage_not_exists(&*s3_backend, name)
			.await
			.expect("File should not exist after delete");
	}

	#[rstest]
	#[tokio::test]
	async fn test_exists_true(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_exists_true.txt";
		s3_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let exists = s3_backend
			.exists(name)
			.await
			.expect("Failed to check exists");
		assert!(exists);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_exists_false(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_nonexistent.txt";
		let exists = s3_backend
			.exists(name)
			.await
			.expect("Failed to check exists");
		assert!(!exists);
	}

	#[rstest]
	#[tokio::test]
	async fn test_roundtrip(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_roundtrip.bin";
		let content = vec![0u8, 1, 2, 3, 255, 254, 253];

		s3_backend
			.save(name, &content)
			.await
			.expect("Failed to save");

		let read_content = s3_backend.open(name).await.expect("Failed to read");

		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_overwrite_file(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_overwrite.txt";
		let content1 = b"Original content";
		let content2 = b"New content";

		s3_backend
			.save(name, content1)
			.await
			.expect("Failed to save original");

		s3_backend
			.save(name, content2)
			.await
			.expect("Failed to overwrite");

		let read_content = s3_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content2.to_vec());

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_file(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_empty.txt";
		let content: Vec<u8> = vec![];

		s3_backend
			.save(name, &content)
			.await
			.expect("Failed to save empty file");

		let read_content = s3_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_binary_data(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_binary.bin";
		let content: Vec<u8> = (0u8..=255).collect();

		s3_backend
			.save(name, &content)
			.await
			.expect("Failed to save binary data");

		let read_content = s3_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_nested_path(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "path/to/nested/file.txt";
		let content = b"Nested file content";

		s3_backend
			.save(name, content)
			.await
			.expect("Failed to save nested file");

		assert_storage_exists(&*s3_backend, name)
			.await
			.expect("Nested file should exist");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}
}

// ============================================================================
// URL Tests
// ============================================================================

mod url_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_presigned_url_format(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_url.txt";
		s3_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let url = s3_backend.url(name, 3600).await.expect("Failed to get URL");

		assert_presigned_url(&url).expect("URL should be valid presigned URL");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_presigned_url_custom_expiry(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_expiry.txt";
		s3_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let url = s3_backend.url(name, 7200).await.expect("Failed to get URL");

		assert_presigned_url(&url).expect("URL should be valid presigned URL");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_url_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent.txt";

		let result = s3_backend.url(name, 3600).await;
		assert!(result.is_err(), "Should return error for nonexistent file");
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_url_contains_bucket(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_bucket_in_url.txt";
		s3_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let url = s3_backend.url(name, 3600).await.expect("Failed to get URL");

		// URL should contain the bucket name
		assert!(
			url.contains("test-bucket"),
			"URL should contain bucket name"
		);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}
}

// ============================================================================
// Prefix Tests
// ============================================================================

mod prefix_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_prefix_applied_to_save(
		#[future(awt)] s3_backend_with_prefix: Arc<dyn StorageBackend>,
	) {
		let name = "test_prefix_save.txt";
		let content = b"Prefix test content";

		let path = s3_backend_with_prefix
			.save(name, content)
			.await
			.expect("Failed to save with prefix");

		assert!(
			path.contains("test-prefix"),
			"Path should contain prefix: {}",
			path
		);
		assert!(path.contains(name), "Path should contain filename");

		// Cleanup
		s3_backend_with_prefix.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_prefix_applied_to_open(
		#[future(awt)] s3_backend_with_prefix: Arc<dyn StorageBackend>,
	) {
		let name = "test_prefix_open.txt";
		let content = b"Prefix open test";

		s3_backend_with_prefix
			.save(name, content)
			.await
			.expect("Failed to save with prefix");

		let read_content = s3_backend_with_prefix
			.open(name)
			.await
			.expect("Failed to read with prefix");

		assert_eq!(read_content, content);

		// Cleanup
		s3_backend_with_prefix.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_prefix_trailing_slash(#[future(awt)] s3_container: S3TestContainer) {
		// Create a new backend with trailing slash in prefix
		let container = s3_container;
		let backend = container
			.create_backend_with_prefix("trailing-slash/")
			.await;

		let name = "test_trailing.txt";
		let content = b"Trailing slash test";

		let path = backend.save(name, content).await.expect("Failed to save");

		assert!(
			path.contains("trailing-slash"),
			"Path should contain prefix"
		);

		// Cleanup
		backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_prefix_with_nested_path(
		#[future(awt)] s3_backend_with_prefix: Arc<dyn StorageBackend>,
	) {
		let name = "nested/path/file.txt";
		let content = b"Prefix with nested path";

		let path = s3_backend_with_prefix
			.save(name, content)
			.await
			.expect("Failed to save");

		assert!(path.contains("test-prefix"), "Path should contain prefix");
		assert!(
			path.contains("nested"),
			"Path should contain nested directories"
		);

		// Cleanup
		s3_backend_with_prefix.delete(name).await.ok();
	}
}

// ============================================================================
// Metadata Tests
// ============================================================================

mod metadata_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_file_size(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_size.txt";
		let content = b"Hello, S3 Size!";

		s3_backend
			.save(name, content)
			.await
			.expect("Failed to save");

		let size = s3_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size, content.len() as u64);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_modified_time(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_time.txt";

		s3_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let modified_time = s3_backend
			.get_modified_time(name)
			.await
			.expect("Failed to get modified time");

		assert!(modified_time.timestamp() > 0, "Should have valid timestamp");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_updates_after_overwrite(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_size_update.txt";
		let content1 = b"Small";
		let content2 = b"This is much larger content";

		s3_backend
			.save(name, content1)
			.await
			.expect("Failed to save original");

		let size1 = s3_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size1, content1.len() as u64);

		s3_backend
			.save(name, content2)
			.await
			.expect("Failed to overwrite");

		let size2 = s3_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size2, content2.len() as u64);
		assert!(size2 > size1, "Size should increase after overwrite");

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_file_size(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "test_empty_size.txt";
		let content: Vec<u8> = vec![];

		s3_backend
			.save(name, &content)
			.await
			.expect("Failed to save");

		let size = s3_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size, 0);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}
}

// ============================================================================
// Error Tests
// ============================================================================

mod error_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_open_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_open.txt";

		let result = s3_backend.open(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_delete.txt";

		let result = s3_backend.delete(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_size.txt";

		let result = s3_backend.size(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_url_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_url.txt";

		let result = s3_backend.url(name, 3600).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_special_characters_in_name(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "file with spaces & symbols!.txt";
		let content = b"Special chars";

		s3_backend
			.save(name, content)
			.await
			.expect("Failed to save");

		let read_content = s3_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_unicode_filename(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "ファイル.txt"; // Japanese filename
		let content = "Unicode content".as_bytes();

		s3_backend
			.save(name, content)
			.await
			.expect("Failed to save unicode filename");

		let read_content = s3_backend
			.open(name)
			.await
			.expect("Failed to read unicode filename");

		assert_eq!(read_content, content);

		// Cleanup
		s3_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_modified_time_not_found(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_time.txt";

		let result = s3_backend.get_modified_time(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}
}

// ============================================================================
// Multiple Operations Tests
// ============================================================================

mod multiple_operations_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_multiple_files(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let files: Vec<(&str, &[u8])> = vec![
			("file1.txt", b"content1"),
			("file2.txt", b"content2"),
			("file3.txt", b"content3"),
		];

		// Save all files
		for (name, content) in &files {
			s3_backend
				.save(name, *content)
				.await
				.expect("Failed to save");
		}

		// Verify all files exist
		for (name, _) in &files {
			assert_storage_exists(&*s3_backend, name)
				.await
				.expect("File should exist");
		}

		// Read and verify all files
		for (name, content) in &files {
			let read_content = s3_backend.open(name).await.expect("Failed to read");
			assert_eq!(read_content, *content);
		}

		// Cleanup
		for (name, _) in &files {
			s3_backend.delete(name).await.ok();
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_unique_names(#[future(awt)] s3_backend: Arc<dyn StorageBackend>) {
		let name1 = generate_unique_name("test");
		let name2 = generate_unique_name("test");

		assert_ne!(name1, name2);

		s3_backend
			.save(&name1, b"content1")
			.await
			.expect("Failed to save first");

		s3_backend
			.save(&name2, b"content2")
			.await
			.expect("Failed to save second");

		assert_storage_exists(&*s3_backend, &name1)
			.await
			.expect("First file should exist");
		assert_storage_exists(&*s3_backend, &name2)
			.await
			.expect("Second file should exist");

		// Cleanup
		s3_backend.delete(&name1).await.ok();
		s3_backend.delete(&name2).await.ok();
	}
}
