//! Integration tests for LocalStorage backend.

mod fixtures;
mod utils;

use fixtures::{LocalTestDir, TestFile, local_backend, local_temp_dir, small_file};
use reinhardt_storages::{StorageBackend, StorageError};
use rstest::rstest;
use std::sync::Arc;
use utils::{
	assert_file_url, assert_storage_exists, assert_storage_not_exists, generate_nested_path,
};

// ============================================================================
// CRUD Tests
// ============================================================================

mod crud_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_save_file(
		#[future(awt)] local_backend: Arc<dyn StorageBackend>,
		small_file: TestFile,
	) {
		let path = local_backend
			.save(&small_file.name, &small_file.content)
			.await
			.expect("Failed to save file");

		assert_eq!(path, small_file.name);
		assert_storage_exists(&*local_backend, &small_file.name)
			.await
			.expect("File should exist");

		// Cleanup
		local_backend.delete(&small_file.name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_open_file(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_open.txt";
		let content = b"Hello, LocalStorage!";

		local_backend
			.save(name, content)
			.await
			.expect("Failed to save file");

		let read_content = local_backend.open(name).await.expect("Failed to open file");

		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete_file(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_delete.txt";
		local_backend
			.save(name, b"temporary")
			.await
			.expect("Failed to save");

		assert_storage_exists(&*local_backend, name)
			.await
			.expect("File should exist before delete");

		local_backend
			.delete(name)
			.await
			.expect("Failed to delete file");

		assert_storage_not_exists(&*local_backend, name)
			.await
			.expect("File should not exist after delete");
	}

	#[rstest]
	#[tokio::test]
	async fn test_exists_true(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_exists_true.txt";
		local_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let exists = local_backend
			.exists(name)
			.await
			.expect("Failed to check exists");
		assert!(exists);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_exists_false(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_nonexistent.txt";
		let exists = local_backend
			.exists(name)
			.await
			.expect("Failed to check exists");
		assert!(!exists);
	}

	#[rstest]
	#[tokio::test]
	async fn test_roundtrip(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_roundtrip.bin";
		let content = vec![0u8, 1, 2, 3, 255, 254, 253];

		local_backend
			.save(name, &content)
			.await
			.expect("Failed to save");

		let read_content = local_backend.open(name).await.expect("Failed to read");

		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_overwrite_file(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_overwrite.txt";
		let content1 = b"Original content";
		let content2 = b"New content";

		local_backend
			.save(name, content1)
			.await
			.expect("Failed to save original");

		local_backend
			.save(name, content2)
			.await
			.expect("Failed to overwrite");

		let read_content = local_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content2.to_vec());

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_file(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_empty.txt";
		let content: Vec<u8> = vec![];

		local_backend
			.save(name, &content)
			.await
			.expect("Failed to save empty file");

		let read_content = local_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_binary_data(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_binary.bin";
		let content: Vec<u8> = (0u8..=255).collect();

		local_backend
			.save(name, &content)
			.await
			.expect("Failed to save binary data");

		let read_content = local_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}
}

// ============================================================================
// Directory Tests
// ============================================================================

mod directory_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_creates_parent_directories(
		#[future(awt)] local_backend: Arc<dyn StorageBackend>,
	) {
		let name = "parent/child/grandchild/file.txt";
		let content = b"Nested file content";

		let path = local_backend
			.save(name, content)
			.await
			.expect("Failed to save nested file");

		assert_eq!(path, name);
		assert_storage_exists(&*local_backend, name)
			.await
			.expect("File should exist in nested path");

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_nested_path(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = generate_nested_path(3, "file.txt");
		let content = b"Deeply nested file";

		local_backend
			.save(&name, content)
			.await
			.expect("Failed to save");

		assert_storage_exists(&*local_backend, &name)
			.await
			.expect("Nested file should exist");

		// Cleanup
		local_backend.delete(&name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_directory_not_a_file(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let dir_name = "test_dir";

		// Create a directory by saving a file in it
		local_backend
			.save(&format!("{}/file.txt", dir_name), b"content")
			.await
			.expect("Failed to save");

		// Directory should return false for exists (not a file)
		let exists = local_backend
			.exists(dir_name)
			.await
			.expect("Failed to check");
		assert!(!exists, "Directory should not be treated as a file");

		// Cleanup
		local_backend
			.delete(&format!("{}/file.txt", dir_name))
			.await
			.ok();
	}
}

// ============================================================================
// URL Tests
// ============================================================================

mod url_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_file_url_format(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_url.txt";
		local_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let url = local_backend
			.url(name, 3600)
			.await
			.expect("Failed to get URL");

		assert_file_url(&url).expect("URL should be valid file:// URL");
		assert!(url.contains(name), "URL should contain file name");

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_url_uses_absolute_path(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_absolute.txt";
		local_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let url = local_backend
			.url(name, 3600)
			.await
			.expect("Failed to get URL");
		assert_file_url(&url).expect("URL should be valid");

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_url_not_found(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent.txt";

		let result = local_backend.url(name, 3600).await;
		assert!(result.is_err(), "Should return error for nonexistent file");
		assert!(matches!(result, Err(StorageError::NotFound(_))));

		// Cleanup not needed (file doesn't exist)
	}
}

// ============================================================================
// Metadata Tests
// ============================================================================

mod metadata_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_file_size(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_size.txt";
		let content = b"Hello, World!";

		local_backend
			.save(name, content)
			.await
			.expect("Failed to save");

		let size = local_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size, content.len() as u64);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_modified_time(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "test_time.txt";

		local_backend
			.save(name, b"content")
			.await
			.expect("Failed to save");

		let modified_time = local_backend
			.get_modified_time(name)
			.await
			.expect("Failed to get modified time");

		assert!(modified_time.timestamp() > 0, "Should have valid timestamp");

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_updates_after_overwrite(
		#[future(awt)] local_backend: Arc<dyn StorageBackend>,
	) {
		let name = "test_size_update.txt";
		let content1 = b"Small";
		let content2 = b"This is much larger content";

		local_backend
			.save(name, content1)
			.await
			.expect("Failed to save original");

		let size1 = local_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size1, content1.len() as u64);

		local_backend
			.save(name, content2)
			.await
			.expect("Failed to overwrite");

		let size2 = local_backend.size(name).await.expect("Failed to get size");
		assert_eq!(size2, content2.len() as u64);
		assert!(size2 > size1, "Size should increase after overwrite");

		// Cleanup
		local_backend.delete(name).await.ok();
	}
}

// ============================================================================
// Error Tests
// ============================================================================

mod error_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_open_not_found(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_open.txt";

		let result = local_backend.open(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete_not_found(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_delete.txt";

		let result = local_backend.delete(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_not_found(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "nonexistent_size.txt";

		let result = local_backend.size(name).await;
		assert!(result.is_err());
		assert!(matches!(result, Err(StorageError::NotFound(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_unicode_filename(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let name = "ファイル.txt"; // Japanese filename
		let content = "Unicode content".as_bytes();

		local_backend
			.save(name, content)
			.await
			.expect("Failed to save unicode filename");

		let read_content = local_backend
			.open(name)
			.await
			.expect("Failed to read unicode filename");

		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_special_characters_in_name(
		#[future(awt)] local_backend: Arc<dyn StorageBackend>,
	) {
		let name = "file with spaces & symbols!.txt";
		let content = b"Special chars";

		local_backend
			.save(name, content)
			.await
			.expect("Failed to save");

		let read_content = local_backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		local_backend.delete(name).await.ok();
	}
}

// ============================================================================
// Persistence Tests
// ============================================================================

mod persistence_tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_file_persistence(#[future(awt)] local_temp_dir: LocalTestDir) {
		let backend = local_temp_dir.backend();
		let name = "persistent.txt";
		let content = b"Persistent content";

		backend.save(name, content).await.expect("Failed to save");

		// Re-read to verify persistence
		let read_content = backend.open(name).await.expect("Failed to read");
		assert_eq!(read_content, content);

		// Cleanup
		backend.delete(name).await.ok();
	}

	#[rstest]
	#[tokio::test]
	async fn test_multiple_operations(#[future(awt)] local_backend: Arc<dyn StorageBackend>) {
		let files: Vec<(&str, &[u8])> = vec![
			("file1.txt", b"content1"),
			("file2.txt", b"content2"),
			("file3.txt", b"content3"),
		];

		// Save all files
		for (name, content) in &files {
			local_backend
				.save(name, *content)
				.await
				.expect("Failed to save");
		}

		// Verify all files exist
		for (name, _) in &files {
			assert_storage_exists(&*local_backend, name)
				.await
				.expect("File should exist");
		}

		// Read and verify all files
		for (name, content) in &files {
			let read_content = local_backend.open(name).await.expect("Failed to read");
			assert_eq!(read_content, *content);
		}

		// Cleanup
		for (name, _) in &files {
			local_backend.delete(name).await.ok();
		}
	}
}
