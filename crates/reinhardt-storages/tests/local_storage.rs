//! Integration tests for LocalStorage backend.

use reinhardt_storages::StorageBackend;
use reinhardt_storages::backends::local::LocalStorage;
use reinhardt_storages::config::LocalConfig;
use tempfile::TempDir;

#[tokio::test]
async fn test_local_storage_save_and_open() {
	// Create temporary directory
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	// Create LocalStorage
	let config = LocalConfig { base_path };
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	// Save a file
	let content = b"Hello, LocalStorage!";
	let name = storage
		.save("test.txt", content)
		.await
		.expect("Failed to save file");

	assert_eq!(name, "test.txt");

	// Open the file
	let read_content = storage.open("test.txt").await.expect("Failed to open file");

	assert_eq!(read_content, content);

	// Cleanup is automatic (TempDir drops)
}

#[tokio::test]
async fn test_local_storage_exists() {
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	let config = LocalConfig { base_path };
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	// File doesn't exist yet
	let exists = storage
		.exists("nonexistent.txt")
		.await
		.expect("Failed to check exists");
	assert!(!exists);

	// Create a file
	storage
		.save("exists.txt", b"test")
		.await
		.expect("Failed to save");

	// File exists now
	let exists = storage
		.exists("exists.txt")
		.await
		.expect("Failed to check exists");
	assert!(exists);
}

#[tokio::test]
async fn test_local_storage_delete() {
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	let config = LocalConfig { base_path };
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	// Create a file
	storage
		.save("delete_me.txt", b"temporary")
		.await
		.expect("Failed to save");

	// Verify it exists
	assert!(
		storage
			.exists("delete_me.txt")
			.await
			.expect("Failed to check exists")
	);

	// Delete it
	storage
		.delete("delete_me.txt")
		.await
		.expect("Failed to delete");

	// Verify it's gone
	assert!(
		!storage
			.exists("delete_me.txt")
			.await
			.expect("Failed to check exists")
	);
}

#[tokio::test]
async fn test_local_storage_size() {
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	let config = LocalConfig { base_path };
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	let content = b"Hello, World!";
	storage
		.save("size_test.txt", content)
		.await
		.expect("Failed to save");

	let size = storage
		.size("size_test.txt")
		.await
		.expect("Failed to get size");
	assert_eq!(size, content.len() as u64);
}

#[tokio::test]
async fn test_local_storage_url() {
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	let config = LocalConfig {
		base_path: base_path.clone(),
	};
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	storage
		.save("url_test.txt", b"test")
		.await
		.expect("Failed to save");

	let url = storage
		.url("url_test.txt", 3600)
		.await
		.expect("Failed to get URL");

	// URL should be a file:// URL
	assert!(url.starts_with("file://"));
	assert!(url.contains("url_test.txt"));
}

#[tokio::test]
async fn test_local_storage_get_modified_time() {
	let temp_dir = TempDir::new().expect("Failed to create temp dir");
	let base_path = temp_dir.path().to_str().unwrap().to_string();

	let config = LocalConfig { base_path };
	let storage = LocalStorage::new(config).expect("Failed to create LocalStorage");

	storage
		.save("time_test.txt", b"test")
		.await
		.expect("Failed to save");

	let modified_time = storage
		.get_modified_time("time_test.txt")
		.await
		.expect("Failed to get modified time");

	// Just verify we got a timestamp
	assert!(modified_time.timestamp() > 0);
}
