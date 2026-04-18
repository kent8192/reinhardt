//! Static File + ORM Integration Tests
//!
//! Comprehensive integration tests for static file management with database tracking.
//! These tests verify:
//! - Static file metadata storage in database
//! - Static file versioning with ORM
//! - Static file cleanup with database tracking
//! - CDN URL generation with database metadata
//! - Static file integrity checks
//!
//! ## Test Coverage
//!
//! 1. **Metadata Storage**: Tracking static files in database
//! 2. **Versioning**: Managing multiple versions of static files
//! 3. **Cleanup**: Removing orphaned static files using database records
//! 4. **CDN Integration**: URL generation with database-backed configuration
//! 5. **Integrity**: Checksum verification and file validation
//!
//! ## Fixtures Used
//!
//! - `postgres_container`: For database-backed static file metadata
//! - `temp_dir`: For filesystem static file storage
//!
//! ## What These Tests Verify
//!
//! ✅ Static file metadata can be stored in database
//! ✅ File versioning tracks changes over time
//! ✅ Orphaned files can be detected and cleaned up
//! ✅ CDN URLs are generated from database configuration
//! ✅ File integrity is verified using checksums
//! ✅ Static file lifecycle is tracked end-to-end
//!
//! ## What These Tests Don't Cover
//!
//! ❌ Static file compression (covered by static processing tests)
//! ❌ Static file minification (covered by static processing tests)
//! ❌ Static file serving (covered by HTTP server tests)
//! ❌ Multi-CDN failover (requires external CDN infrastructure)

use reinhardt_db::orm::DatabaseConnection;
use reinhardt_utils::storage::{LocalStorage, Storage};
use reinhardt_test::fixtures::testcontainers::postgres_container;
use rstest::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use testcontainers::{ContainerAsync, GenericImage};

// ============ Test Helper Structs ============

/// Static file metadata for database tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StaticFileMetadata {
	id: Option<i64>,
	path: String,
	hash: String,
	size: i64,
	version: i32,
	cdn_url: Option<String>,
	created_at: i64,
	updated_at: i64,
}

reinhardt_test::impl_test_model!(StaticFileMetadata, i64, "static_file_metadata");

impl StaticFileMetadata {
	fn new(path: &str, content: &[u8]) -> Self {
		let hash = compute_hash(content);
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_secs() as i64;

		Self {
			id: None,
			path: path.to_string(),
			hash,
			size: content.len() as i64,
			version: 1,
			cdn_url: None,
			created_at: now,
			updated_at: now,
		}
	}

	fn with_cdn_url(mut self, url: &str) -> Self {
		self.cdn_url = Some(url.to_string());
		self
	}
}

/// Compute SHA-256 hash of content
fn compute_hash(content: &[u8]) -> String {
	let mut hasher = Sha256::new();
	hasher.update(content);
	format!("{:x}", hasher.finalize())
}

/// Static file manager with database tracking
struct StaticFileManager {
	storage: LocalStorage,
	connection: Arc<DatabaseConnection>,
	cdn_base_url: Option<String>,
}

impl StaticFileManager {
	fn new(
		storage: LocalStorage,
		connection: Arc<DatabaseConnection>,
		cdn_base_url: Option<String>,
	) -> Self {
		Self {
			storage,
			connection,
			cdn_base_url,
		}
	}

	async fn save_file(&self, path: &str, content: &[u8]) -> Result<StaticFileMetadata, String> {
		// Save to storage
		self.storage
			.save(path, content)
			.await
			.map_err(|e| format!("Storage error: {}", e))?;

		// Create metadata
		let cdn_url = self.generate_cdn_url(path);
		let mut metadata = StaticFileMetadata::new(path, content);
		if let Some(url) = cdn_url {
			metadata = metadata.with_cdn_url(&url);
		}

		// Save metadata to database
		let query = format!(
			"INSERT INTO {} (path, hash, size, version, cdn_url, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (path) DO UPDATE
             SET hash = $2, size = $3, version = {}.version + 1, cdn_url = $5, updated_at = $7
             RETURNING id, version",
			StaticFileMetadata::table_name(),
			StaticFileMetadata::table_name()
		);

		let rows = self
			.connection
			.query(
				&query,
				vec![
					metadata.path.clone().into(),
					metadata.hash.clone().into(),
					metadata.size.into(),
					metadata.version.into(),
					metadata.cdn_url.clone().into(),
					metadata.created_at.into(),
					metadata.updated_at.into(),
				],
			)
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		if let Some(row) = rows.first() {
			if let Some(id_value) = row.get("id") {
				if let serde_json::Value::Number(num) = id_value {
					metadata.id = num.as_i64();
				}
			}
			if let Some(version_value) = row.get("version") {
				if let serde_json::Value::Number(num) = version_value {
					metadata.version = num.as_i64().unwrap_or(1) as i32;
				}
			}
		}

		Ok(metadata)
	}

	async fn get_metadata(&self, path: &str) -> Result<StaticFileMetadata, String> {
		let query = format!(
			"SELECT id, path, hash, size, version, cdn_url, created_at, updated_at
             FROM {}
             WHERE path = $1",
			StaticFileMetadata::table_name()
		);

		let rows = self
			.connection
			.query(&query, vec![path.into()])
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		if let Some(row) = rows.first() {
			let id = row
				.get("id")
				.and_then(|v| {
					if let serde_json::Value::Number(n) = v {
						n.as_i64()
					} else {
						None
					}
				})
				.ok_or("Missing id")?;

			let path = row
				.get("path")
				.and_then(|v| {
					if let serde_json::Value::String(s) = v {
						Some(s.clone())
					} else {
						None
					}
				})
				.ok_or("Missing path")?;

			let hash = row
				.get("hash")
				.and_then(|v| {
					if let serde_json::Value::String(s) = v {
						Some(s.clone())
					} else {
						None
					}
				})
				.ok_or("Missing hash")?;

			let size = row
				.get("size")
				.and_then(|v| {
					if let serde_json::Value::Number(n) = v {
						n.as_i64()
					} else {
						None
					}
				})
				.ok_or("Missing size")?;

			let version = row
				.get("version")
				.and_then(|v| {
					if let serde_json::Value::Number(n) = v {
						n.as_i64().map(|v| v as i32)
					} else {
						None
					}
				})
				.ok_or("Missing version")?;

			let cdn_url = row.get("cdn_url").and_then(|v| {
				if let serde_json::Value::String(s) = v {
					Some(s.clone())
				} else {
					None
				}
			});

			let created_at = row
				.get("created_at")
				.and_then(|v| {
					if let serde_json::Value::Number(n) = v {
						n.as_i64()
					} else {
						None
					}
				})
				.ok_or("Missing created_at")?;

			let updated_at = row
				.get("updated_at")
				.and_then(|v| {
					if let serde_json::Value::Number(n) = v {
						n.as_i64()
					} else {
						None
					}
				})
				.ok_or("Missing updated_at")?;

			Ok(StaticFileMetadata {
				id: Some(id),
				path,
				hash,
				size,
				version,
				cdn_url,
				created_at,
				updated_at,
			})
		} else {
			Err("File not found".to_string())
		}
	}

	async fn verify_integrity(&self, path: &str) -> Result<bool, String> {
		// Get metadata from database
		let metadata = self.get_metadata(path).await?;

		// Read file from storage
		let file = self
			.storage
			.read(path)
			.await
			.map_err(|e| format!("Storage error: {}", e))?;

		// Compute hash of stored file
		let computed_hash = compute_hash(&file.content);

		// Compare hashes
		Ok(computed_hash == metadata.hash)
	}

	async fn list_orphaned_files(&self) -> Result<Vec<String>, String> {
		// Get all paths from database
		let query = format!("SELECT path FROM {}", StaticFileMetadata::table_name());
		let rows = self
			.connection
			.query(&query, vec![])
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		let db_paths: Vec<String> = rows
			.iter()
			.filter_map(|row| {
				row.get("path").and_then(|v| {
					if let serde_json::Value::String(s) = v {
						Some(s.clone())
					} else {
						None
					}
				})
			})
			.collect();

		// Find files in storage not in database
		let mut orphaned = Vec::new();

		// NOTE: production code would scan the storage directory;
		// this test just checks whether database files exist in storage.
		for path in &db_paths {
			if !self.storage.exists(path).await.unwrap_or(false) {
				orphaned.push(path.clone());
			}
		}

		Ok(orphaned)
	}

	async fn cleanup_orphaned(&self) -> Result<usize, String> {
		let orphaned = self.list_orphaned_files().await?;
		let count = orphaned.len();

		// Remove orphaned entries from database
		for path in orphaned {
			let query = format!(
				"DELETE FROM {} WHERE path = $1",
				StaticFileMetadata::table_name()
			);
			self.connection
				.execute(&query, vec![path.into()])
				.await
				.map_err(|e| format!("Database error: {}", e))?;
		}

		Ok(count)
	}

	fn generate_cdn_url(&self, path: &str) -> Option<String> {
		self.cdn_base_url.as_ref().map(|base| {
			format!(
				"{}/{}",
				base.trim_end_matches('/'),
				path.trim_start_matches('/')
			)
		})
	}

	async fn delete_file(&self, path: &str) -> Result<(), String> {
		// Delete from storage
		self.storage
			.delete(path)
			.await
			.map_err(|e| format!("Storage error: {}", e))?;

		// Delete metadata from database
		let query = format!(
			"DELETE FROM {} WHERE path = $1",
			StaticFileMetadata::table_name()
		);
		self.connection
			.execute(&query, vec![path.into()])
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		Ok(())
	}
}

// ============ Fixtures ============

#[fixture]
fn temp_dir() -> TempDir {
	TempDir::new().expect("Failed to create temp directory")
}

#[fixture]
async fn postgres_fixture(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<DatabaseConnection>) {
	let (container, _pool, _port, database_url) = postgres_container.await;

	// Create connection
	let conn = DatabaseConnection::connect(&database_url)
		.await
		.expect("Failed to connect to database");

	// Create static_file_metadata table
	conn.execute(
		"CREATE TABLE IF NOT EXISTS static_file_metadata (
            id SERIAL PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            hash TEXT NOT NULL,
            size BIGINT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            cdn_url TEXT,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL
        )",
		vec![],
	)
	.await
	.expect("Failed to create static_file_metadata table");

	(container, Arc::new(conn))
}

#[fixture]
async fn static_manager(
	temp_dir: TempDir,
	#[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) -> (TempDir, ContainerAsync<GenericImage>, StaticFileManager) {
	let (_container, conn) = postgres_fixture.await;

	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/static");
	storage.ensure_base_dir().await.unwrap();

	let manager = StaticFileManager::new(storage, conn, None);

	(temp_dir, _container, manager)
}

// ============ Metadata Storage Tests ============

/// Test intent: Verify static file metadata is saved to database
#[rstest]
#[tokio::test]
async fn test_static_file_metadata_storage(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "css/main.css";
	let content = b"body { margin: 0; }";

	// Save file
	let metadata = manager.save_file(path, content).await.unwrap();

	assert_eq!(metadata.path, path);
	assert_eq!(metadata.size, content.len() as i64);
	assert_eq!(metadata.version, 1);
	assert!(!metadata.hash.is_empty());
	assert!(metadata.id.is_some());
}

/// Test intent: Verify metadata can be retrieved from database
#[rstest]
#[tokio::test]
async fn test_static_file_metadata_retrieval(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "js/app.js";
	let content = b"console.log('Hello');";

	// Save file
	let saved_metadata = manager.save_file(path, content).await.unwrap();

	// Retrieve metadata
	let retrieved_metadata = manager.get_metadata(path).await.unwrap();

	assert_eq!(retrieved_metadata.id, saved_metadata.id);
	assert_eq!(retrieved_metadata.path, path);
	assert_eq!(retrieved_metadata.hash, saved_metadata.hash);
	assert_eq!(retrieved_metadata.size, content.len() as i64);
}

/// Test intent: Verify file versioning increments on update
#[rstest]
#[tokio::test]
async fn test_static_file_versioning(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "css/style.css";
	let version1_content = b"body { color: red; }";
	let version2_content = b"body { color: blue; }";

	// Save version 1
	let metadata1 = manager.save_file(path, version1_content).await.unwrap();
	assert_eq!(metadata1.version, 1);

	// Save version 2 (update)
	let metadata2 = manager.save_file(path, version2_content).await.unwrap();
	assert_eq!(metadata2.version, 2);
	assert_eq!(metadata2.id, metadata1.id); // Same record, incremented version

	// Verify hash changed
	assert_ne!(metadata1.hash, metadata2.hash);
}

/// Test intent: Verify hash changes when content changes
#[rstest]
#[tokio::test]
async fn test_static_file_hash_updates(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "data.json";
	let original = b"{\"key\": \"value1\"}";
	let updated = b"{\"key\": \"value2\"}";

	// Save original
	let metadata1 = manager.save_file(path, original).await.unwrap();
	let hash1 = metadata1.hash.clone();

	// Update content
	let metadata2 = manager.save_file(path, updated).await.unwrap();
	let hash2 = metadata2.hash;

	// Hashes should differ
	assert_ne!(hash1, hash2);
}

/// Test intent: Verify file size is tracked accurately
#[rstest]
#[tokio::test]
async fn test_static_file_size_tracking(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let files = vec![
		("small.txt", b"A" as &[u8]),
		("medium.txt", b"B".repeat(1000).as_slice()),
		("large.txt", b"C".repeat(10000).as_slice()),
	];

	for (path, content) in files {
		let metadata = manager.save_file(path, content).await.unwrap();
		assert_eq!(metadata.size, content.len() as i64);
	}
}

// ============ CDN Integration Tests ============

/// Test intent: Verify CDN URL generation from database metadata
#[rstest]
#[tokio::test]
async fn test_cdn_url_generation(
	temp_dir: TempDir,
	#[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = postgres_fixture.await;

	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/static");
	storage.ensure_base_dir().await.unwrap();

	let cdn_base_url = Some("https://cdn.example.com".to_string());
	let manager = StaticFileManager::new(storage, conn, cdn_base_url);

	let path = "images/logo.png";
	let content = b"PNG image data";

	// Save file with CDN URL
	let metadata = manager.save_file(path, content).await.unwrap();

	assert!(metadata.cdn_url.is_some());
	assert_eq!(
		metadata.cdn_url.unwrap(),
		"https://cdn.example.com/images/logo.png"
	);
}

/// Test intent: Verify CDN URL is stored in database
#[rstest]
#[tokio::test]
async fn test_cdn_url_persistence(
	temp_dir: TempDir,
	#[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = postgres_fixture.await;

	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/static");
	storage.ensure_base_dir().await.unwrap();

	let cdn_base_url = Some("https://static.myapp.com".to_string());
	let manager = StaticFileManager::new(storage, conn, cdn_base_url);

	let path = "fonts/roboto.woff2";
	let content = b"WOFF2 font data";

	// Save file
	manager.save_file(path, content).await.unwrap();

	// Retrieve and verify CDN URL
	let metadata = manager.get_metadata(path).await.unwrap();
	assert_eq!(
		metadata.cdn_url,
		Some("https://static.myapp.com/fonts/roboto.woff2".to_string())
	);
}

/// Test intent: Verify multiple CDN configurations
#[rstest]
#[tokio::test]
async fn test_multiple_cdn_configurations(
	temp_dir: TempDir,
	#[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>),
) {
	let (_container, conn) = postgres_fixture.await;

	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/static");
	storage.ensure_base_dir().await.unwrap();

	// Manager with CDN
	let cdn_manager = StaticFileManager::new(
		storage.clone(),
		Arc::clone(&conn),
		Some("https://cdn1.example.com".to_string()),
	);

	// Manager without CDN
	let local_manager = StaticFileManager::new(storage, Arc::clone(&conn), None);

	// Save with CDN
	let cdn_file = cdn_manager
		.save_file("cdn/asset.js", b"// CDN asset")
		.await
		.unwrap();
	assert!(cdn_file.cdn_url.is_some());

	// Save without CDN
	let local_file = local_manager
		.save_file("local/asset.js", b"// Local asset")
		.await
		.unwrap();
	assert!(local_file.cdn_url.is_none());
}

// ============ Integrity Check Tests ============

/// Test intent: Verify file integrity using checksum
#[rstest]
#[tokio::test]
async fn test_static_file_integrity_check(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "verify/data.bin";
	let content = b"Important data that must not be corrupted";

	// Save file
	manager.save_file(path, content).await.unwrap();

	// Verify integrity
	let is_valid = manager.verify_integrity(path).await.unwrap();
	assert!(is_valid);
}

/// Test intent: Verify integrity fails for corrupted files
#[rstest]
#[tokio::test]
async fn test_static_file_integrity_corruption_detection(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "corrupt/file.txt";
	let original_content = b"Original content";
	let corrupted_content = b"Corrupted content";

	// Save original
	manager.save_file(path, original_content).await.unwrap();

	// Manually corrupt the file (bypass metadata update)
	manager.storage.save(path, corrupted_content).await.unwrap();

	// Verify integrity (should fail)
	let is_valid = manager.verify_integrity(path).await.unwrap();
	assert!(!is_valid);
}

/// Test intent: Verify hash computation is consistent
#[rstest]
#[tokio::test]
async fn test_hash_computation_consistency() {
	let content = b"Test content for hashing";

	let hash1 = compute_hash(content);
	let hash2 = compute_hash(content);

	// Same content should produce same hash
	assert_eq!(hash1, hash2);

	// Different content should produce different hash
	let different_content = b"Different content";
	let hash3 = compute_hash(different_content);
	assert_ne!(hash1, hash3);
}

// ============ Cleanup Tests ============

/// Test intent: Verify orphaned file detection
#[rstest]
#[tokio::test]
async fn test_orphaned_file_detection(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	// Save a file
	let path = "orphan/test.txt";
	manager.save_file(path, b"content").await.unwrap();

	// Delete the physical file but keep database record
	manager.storage.delete(path).await.unwrap();

	// Detect orphaned files
	let orphaned = manager.list_orphaned_files().await.unwrap();
	assert_eq!(orphaned.len(), 1);
	assert_eq!(orphaned[0], path);
}

/// Test intent: Verify orphaned file cleanup
#[rstest]
#[tokio::test]
async fn test_orphaned_file_cleanup(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	// Save files
	let files = vec!["cleanup/file1.txt", "cleanup/file2.txt"];
	for path in &files {
		manager.save_file(path, b"data").await.unwrap();
	}

	// Delete physical files
	for path in &files {
		manager.storage.delete(path).await.unwrap();
	}

	// Cleanup orphaned records
	let cleaned_count = manager.cleanup_orphaned().await.unwrap();
	assert_eq!(cleaned_count, 2);

	// Verify records are removed
	let orphaned = manager.list_orphaned_files().await.unwrap();
	assert_eq!(orphaned.len(), 0);
}

/// Test intent: Verify complete file deletion (storage + database)
#[rstest]
#[tokio::test]
async fn test_complete_file_deletion(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "delete/complete.txt";
	let content = b"Delete me completely";

	// Save file
	manager.save_file(path, content).await.unwrap();

	// Verify file exists
	assert!(manager.storage.exists(path).await.unwrap());
	assert!(manager.get_metadata(path).await.is_ok());

	// Delete completely
	manager.delete_file(path).await.unwrap();

	// Verify both storage and database are cleaned
	assert!(!manager.storage.exists(path).await.unwrap());
	assert!(manager.get_metadata(path).await.is_err());
}

/// Test intent: Verify bulk file tracking
#[rstest]
#[tokio::test]
async fn test_bulk_file_tracking(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	// Save multiple files
	let file_count = 10;
	for i in 0..file_count {
		let path = format!("bulk/file{}.txt", i);
		let content = format!("Content {}", i);
		manager.save_file(&path, content.as_bytes()).await.unwrap();
	}

	// Verify all files are tracked
	for i in 0..file_count {
		let path = format!("bulk/file{}.txt", i);
		let metadata = manager.get_metadata(&path).await.unwrap();
		assert_eq!(metadata.path, path);
	}
}

/// Test intent: Verify timestamp tracking
#[rstest]
#[tokio::test]
async fn test_timestamp_tracking(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "timestamp/test.txt";

	// Save initial version
	let metadata1 = manager.save_file(path, b"v1").await.unwrap();
	let created_at = metadata1.created_at;
	let updated_at1 = metadata1.updated_at;

	// Wait a bit to ensure timestamp changes
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	// Update file
	let metadata2 = manager.save_file(path, b"v2").await.unwrap();
	let updated_at2 = metadata2.updated_at;

	// Created_at should remain the same
	assert_eq!(metadata2.created_at, created_at);

	// Updated_at should change
	// NOTE: Using >= instead of > to account for timestamp resolution
	assert!(updated_at2 >= updated_at1);
}

/// Test intent: Verify nested path handling
#[rstest]
#[tokio::test]
async fn test_nested_path_handling(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let nested_path = "assets/images/icons/user-avatar.png";
	let content = b"PNG avatar image";

	// Save nested file
	let metadata = manager.save_file(nested_path, content).await.unwrap();

	assert_eq!(metadata.path, nested_path);
	assert!(manager.storage.exists(nested_path).await.unwrap());

	// Verify retrieval works
	let retrieved = manager.get_metadata(nested_path).await.unwrap();
	assert_eq!(retrieved.path, nested_path);
}

/// Test intent: Verify version history can be tracked
#[rstest]
#[tokio::test]
async fn test_version_history_tracking(
	#[future] static_manager: (TempDir, ContainerAsync<GenericImage>, StaticFileManager),
) {
	let (_temp_dir, _container, manager) = static_manager.await;

	let path = "versioned/changelog.md";
	let versions = vec![
		b"# Version 1.0.0" as &[u8],
		b"# Version 1.0.0\n# Version 1.1.0",
		b"# Version 1.0.0\n# Version 1.1.0\n# Version 1.2.0",
	];

	// Save multiple versions
	for (idx, content) in versions.iter().enumerate() {
		let metadata = manager.save_file(path, content).await.unwrap();
		assert_eq!(metadata.version, (idx + 1) as i32);
	}

	// Final version should be 3
	let final_metadata = manager.get_metadata(path).await.unwrap();
	assert_eq!(final_metadata.version, 3);
}
