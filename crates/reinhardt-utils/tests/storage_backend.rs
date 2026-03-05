//! Integration tests for reinhardt-utils storage backend (LocalStorage and InMemoryStorage)

use reinhardt_utils::storage::{
	FileMetadata, InMemoryStorage, LocalStorage, Storage, StorageError, StoredFile,
};
use rstest::rstest;
use std::collections::HashSet;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Create a LocalStorage backed by a fresh temporary directory.
fn make_local_storage() -> (LocalStorage, TempDir) {
	let temp_dir = TempDir::new().expect("failed to create temp dir");
	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/media");
	(storage, temp_dir)
}

/// Create an InMemoryStorage for testing.
fn make_memory_storage() -> InMemoryStorage {
	InMemoryStorage::new("test_root", "http://localhost/media")
}

// ===========================================================================
// LocalStorage – write / read / delete / exists
// ===========================================================================

#[rstest]
#[tokio::test]
async fn local_write_and_read_returns_correct_content() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let content = b"hello, reinhardt storage";

	// Act
	storage.save("hello.txt", content).await.unwrap();
	let stored = storage.read("hello.txt").await.unwrap();

	// Assert
	assert_eq!(stored.content, content);
}

#[rstest]
#[tokio::test]
async fn local_save_returns_metadata_with_correct_size() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let content = b"size check";

	// Act
	let metadata = storage.save("size.txt", content).await.unwrap();

	// Assert
	assert_eq!(metadata.size, content.len() as u64);
	assert_eq!(metadata.path, "size.txt");
}

#[rstest]
#[tokio::test]
async fn local_save_computes_checksum() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let metadata = storage.save("cksum.bin", b"data").await.unwrap();

	// Assert
	assert!(
		metadata.checksum.is_some(),
		"checksum should be computed on save"
	);
}

#[rstest]
#[tokio::test]
async fn local_same_content_produces_same_checksum() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let content = b"identical content";

	// Act
	let m1 = storage.save("a.txt", content).await.unwrap();
	let m2 = storage.save("b.txt", content).await.unwrap();

	// Assert
	assert_eq!(m1.checksum, m2.checksum);
}

#[rstest]
#[tokio::test]
async fn local_different_content_produces_different_checksum() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let m1 = storage.save("x.txt", b"foo").await.unwrap();
	let m2 = storage.save("y.txt", b"bar").await.unwrap();

	// Assert
	assert_ne!(m1.checksum, m2.checksum);
}

#[rstest]
#[tokio::test]
async fn local_exists_returns_false_before_save() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.exists("nonexistent.txt").await.unwrap();

	// Assert
	assert!(!result);
}

#[rstest]
#[tokio::test]
async fn local_exists_returns_true_after_save() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("present.txt", b"here").await.unwrap();

	// Act
	let result = storage.exists("present.txt").await.unwrap();

	// Assert
	assert!(result);
}

#[rstest]
#[tokio::test]
async fn local_delete_removes_file() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("temp.txt", b"bye").await.unwrap();
	assert!(storage.exists("temp.txt").await.unwrap());

	// Act
	storage.delete("temp.txt").await.unwrap();

	// Assert
	assert!(!storage.exists("temp.txt").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn local_delete_nonexistent_returns_not_found_error() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.delete("ghost.txt").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

#[rstest]
#[tokio::test]
async fn local_read_nonexistent_returns_not_found_error() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.read("missing.dat").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

#[rstest]
#[tokio::test]
async fn local_save_creates_intermediate_directories() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let nested = "a/b/c/nested.txt";

	// Act
	storage.save(nested, b"deep").await.unwrap();

	// Assert
	assert!(storage.exists(nested).await.unwrap());
}

#[rstest]
#[tokio::test]
async fn local_read_content_after_nested_save() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let path = "sub/dir/file.txt";
	let content = b"nested content";
	storage.save(path, content).await.unwrap();

	// Act
	let stored = storage.read(path).await.unwrap();

	// Assert
	assert_eq!(stored.content, content);
}

#[rstest]
#[tokio::test]
async fn local_overwrite_updates_content() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("over.txt", b"first").await.unwrap();

	// Act
	storage.save("over.txt", b"second").await.unwrap();
	let stored = storage.read("over.txt").await.unwrap();

	// Assert
	assert_eq!(stored.content, b"second");
}

#[rstest]
#[tokio::test]
async fn local_metadata_returns_correct_size() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let content = b"metadata size check";
	storage.save("meta.txt", content).await.unwrap();

	// Act
	let meta = storage.metadata("meta.txt").await.unwrap();

	// Assert
	assert_eq!(meta.size, content.len() as u64);
	assert_eq!(meta.path, "meta.txt");
}

#[rstest]
#[tokio::test]
async fn local_metadata_nonexistent_returns_not_found_error() {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.metadata("no_such_file.txt").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

#[rstest]
#[tokio::test]
async fn local_list_returns_files_in_root() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("alpha.txt", b"a").await.unwrap();
	storage.save("beta.txt", b"b").await.unwrap();
	// Sub-directory file should not appear in root listing
	storage.save("sub/gamma.txt", b"c").await.unwrap();

	// Act
	let files = storage.list("").await.unwrap();
	let names: HashSet<String> = files
		.iter()
		.map(|f| {
			std::path::Path::new(&f.path)
				.file_name()
				.unwrap()
				.to_string_lossy()
				.to_string()
		})
		.collect();

	// Assert
	assert!(names.contains("alpha.txt"), "alpha.txt should be listed");
	assert!(names.contains("beta.txt"), "beta.txt should be listed");
	assert!(
		!names.contains("gamma.txt"),
		"sub-dir file should not be listed at root"
	);
}

#[rstest]
#[tokio::test]
async fn local_list_returns_files_in_subdirectory() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("docs/readme.md", b"readme").await.unwrap();
	storage.save("docs/guide.md", b"guide").await.unwrap();
	storage.save("other.txt", b"other").await.unwrap();

	// Act
	let files = storage.list("docs").await.unwrap();
	let names: HashSet<String> = files
		.iter()
		.map(|f| {
			std::path::Path::new(&f.path)
				.file_name()
				.unwrap()
				.to_string_lossy()
				.to_string()
		})
		.collect();

	// Assert
	assert!(names.contains("readme.md"));
	assert!(names.contains("guide.md"));
	assert!(!names.contains("other.txt"));
}

// ---------------------------------------------------------------------------
// LocalStorage – URL generation
// ---------------------------------------------------------------------------

#[rstest]
#[case(
	"http://localhost/media",
	"test.txt",
	"http://localhost/media/test.txt"
)]
#[case(
	"http://localhost/media/",
	"test.txt",
	"http://localhost/media/test.txt"
)]
#[case(
	"http://example.com/files",
	"img/photo.jpg",
	"http://example.com/files/img/photo.jpg"
)]
fn local_url_combines_base_url_and_path(
	#[case] base_url: &str,
	#[case] file_path: &str,
	#[case] expected: &str,
) {
	// Arrange
	let storage = LocalStorage::new("/tmp/unused", base_url);

	// Act
	let url = storage.url(file_path);

	// Assert
	assert_eq!(url, expected);
}

// ---------------------------------------------------------------------------
// LocalStorage – path traversal prevention
// ---------------------------------------------------------------------------

#[rstest]
#[case("../escape.txt")]
#[case("/etc/passwd")]
#[case("/absolute/path")]
#[case("sub/../escape.txt")]
#[case("..")]
#[case(".")]
#[case("")]
#[tokio::test]
async fn local_save_rejects_dangerous_paths(#[case] dangerous_path: &str) {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.save(dangerous_path, b"data").await;

	// Assert
	assert!(
		matches!(result, Err(StorageError::InvalidPath(_))),
		"expected InvalidPath error for path '{}'",
		dangerous_path
	);
}

#[rstest]
#[case("../escape.txt")]
#[case("/etc/passwd")]
#[case("sub/../escape.txt")]
#[tokio::test]
async fn local_read_rejects_dangerous_paths(#[case] dangerous_path: &str) {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.read(dangerous_path).await;

	// Assert
	assert!(
		matches!(result, Err(StorageError::InvalidPath(_))),
		"expected InvalidPath error for path '{}'",
		dangerous_path
	);
}

#[rstest]
#[case("../escape.txt")]
#[case("/etc/shadow")]
#[tokio::test]
async fn local_delete_rejects_dangerous_paths(#[case] dangerous_path: &str) {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.delete(dangerous_path).await;

	// Assert
	assert!(
		matches!(result, Err(StorageError::InvalidPath(_))),
		"expected InvalidPath error for path '{}'",
		dangerous_path
	);
}

#[rstest]
#[case("../escape.txt")]
#[case("/etc/passwd")]
#[tokio::test]
async fn local_exists_rejects_dangerous_paths(#[case] dangerous_path: &str) {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.exists(dangerous_path).await;

	// Assert
	assert!(
		matches!(result, Err(StorageError::InvalidPath(_))),
		"expected InvalidPath error for path '{}'",
		dangerous_path
	);
}

#[rstest]
#[case("../escape.txt")]
#[case("/etc/passwd")]
#[tokio::test]
async fn local_metadata_rejects_dangerous_paths(#[case] dangerous_path: &str) {
	// Arrange
	let (storage, _dir) = make_local_storage();

	// Act
	let result = storage.metadata(dangerous_path).await;

	// Assert
	assert!(
		matches!(result, Err(StorageError::InvalidPath(_))),
		"expected InvalidPath error for path '{}'",
		dangerous_path
	);
}

// ---------------------------------------------------------------------------
// LocalStorage – large file handling
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn local_large_file_save_and_read_roundtrip() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	let large_content: Vec<u8> = (0u8..=255).cycle().take(1024 * 256).collect();

	// Act
	let meta = storage.save("large.bin", &large_content).await.unwrap();
	let stored = storage.read("large.bin").await.unwrap();

	// Assert
	assert_eq!(meta.size, large_content.len() as u64);
	assert_eq!(stored.content, large_content);
}

// ---------------------------------------------------------------------------
// LocalStorage – time metadata
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn local_get_modified_time_is_recent() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("time.txt", b"ts").await.unwrap();

	// Act
	let mtime = storage.get_modified_time("time.txt").await.unwrap();
	let now = chrono::Utc::now();

	// Assert
	let diff_secs = (now - mtime).num_seconds().abs();
	assert!(
		diff_secs < 10,
		"modified time should be within 10 s of now, diff={}",
		diff_secs
	);
}

#[rstest]
#[tokio::test]
async fn local_get_created_time_is_recent() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("ctime.txt", b"c").await.unwrap();

	// Act
	let ctime = storage.get_created_time("ctime.txt").await.unwrap();
	let now = chrono::Utc::now();

	// Assert
	let diff_secs = (now - ctime).num_seconds().abs();
	assert!(
		diff_secs < 10,
		"created time should be within 10 s of now, diff={}",
		diff_secs
	);
}

#[rstest]
#[tokio::test]
async fn local_get_accessed_time_is_recent() {
	// Arrange
	let (storage, _dir) = make_local_storage();
	storage.save("atime.txt", b"a").await.unwrap();

	// Act
	let atime = storage.get_accessed_time("atime.txt").await.unwrap();
	let now = chrono::Utc::now();

	// Assert
	let diff_secs = (now - atime).num_seconds().abs();
	assert!(
		diff_secs < 10,
		"accessed time should be within 10 s of now, diff={}",
		diff_secs
	);
}

// ===========================================================================
// InMemoryStorage – write / read / delete / exists
// ===========================================================================

#[rstest]
#[tokio::test]
async fn memory_write_and_read_returns_correct_content() {
	// Arrange
	let storage = make_memory_storage();
	let content = b"in-memory content";

	// Act
	storage.save("file.txt", content).await.unwrap();
	let stored = storage.read("file.txt").await.unwrap();

	// Assert
	assert_eq!(stored.content, content);
}

#[rstest]
#[tokio::test]
async fn memory_save_returns_metadata_with_correct_size() {
	// Arrange
	let storage = make_memory_storage();
	let content = b"size data";

	// Act
	let meta = storage.save("sized.txt", content).await.unwrap();

	// Assert
	assert_eq!(meta.size, content.len() as u64);
}

#[rstest]
#[tokio::test]
async fn memory_exists_false_before_save() {
	// Arrange
	let storage = make_memory_storage();

	// Act
	let result = storage.exists("absent.txt").await.unwrap();

	// Assert
	assert!(!result);
}

#[rstest]
#[tokio::test]
async fn memory_exists_true_after_save() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("present.txt", b"here").await.unwrap();

	// Act
	let result = storage.exists("present.txt").await.unwrap();

	// Assert
	assert!(result);
}

#[rstest]
#[tokio::test]
async fn memory_delete_removes_file() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("del.txt", b"gone").await.unwrap();

	// Act
	storage.delete("del.txt").await.unwrap();

	// Assert
	assert!(!storage.exists("del.txt").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn memory_read_nonexistent_returns_not_found_error() {
	// Arrange
	let storage = make_memory_storage();

	// Act
	let result = storage.read("nope.txt").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

#[rstest]
#[tokio::test]
async fn memory_metadata_returns_correct_size() {
	// Arrange
	let storage = make_memory_storage();
	let content = b"meta check";
	storage.save("m.txt", content).await.unwrap();

	// Act
	let meta = storage.metadata("m.txt").await.unwrap();

	// Assert
	assert_eq!(meta.size, content.len() as u64);
}

#[rstest]
#[tokio::test]
async fn memory_metadata_nonexistent_returns_not_found_error() {
	// Arrange
	let storage = make_memory_storage();

	// Act
	let result = storage.metadata("missing.txt").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

#[rstest]
#[tokio::test]
async fn memory_overwrite_updates_content() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("over.txt", b"v1").await.unwrap();

	// Act
	storage.save("over.txt", b"v2").await.unwrap();
	let stored = storage.read("over.txt").await.unwrap();

	// Assert
	assert_eq!(stored.content, b"v2");
}

#[rstest]
#[tokio::test]
async fn memory_list_root_returns_only_top_level_files() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("root_a.txt", b"a").await.unwrap();
	storage.save("root_b.txt", b"b").await.unwrap();
	storage.save("sub/deep.txt", b"c").await.unwrap();

	// Act
	let files = storage.list("").await.unwrap();
	let names: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();

	// Assert
	assert!(names.contains("root_a.txt"));
	assert!(names.contains("root_b.txt"));
	assert!(!names.contains("sub/deep.txt"));
}

#[rstest]
#[tokio::test]
async fn memory_list_subdirectory_returns_direct_children() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("imgs/cat.png", b"cat").await.unwrap();
	storage.save("imgs/dog.png", b"dog").await.unwrap();
	storage.save("docs/readme.md", b"md").await.unwrap();

	// Act
	let files = storage.list("imgs").await.unwrap();
	let paths: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();

	// Assert
	assert!(paths.contains("imgs/cat.png"));
	assert!(paths.contains("imgs/dog.png"));
	assert!(!paths.contains("docs/readme.md"));
}

// ---------------------------------------------------------------------------
// InMemoryStorage – URL generation
// ---------------------------------------------------------------------------

#[rstest]
#[case(
	"http://localhost/media",
	"photo.jpg",
	"http://localhost/media/photo.jpg"
)]
#[case(
	"http://localhost/media/",
	"photo.jpg",
	"http://localhost/media/photo.jpg"
)]
fn memory_url_combines_base_url_and_path(
	#[case] base_url: &str,
	#[case] file_path: &str,
	#[case] expected: &str,
) {
	// Arrange
	let storage = InMemoryStorage::new("root", base_url);

	// Act
	let url = storage.url(file_path);

	// Assert
	assert_eq!(url, expected);
}

// ---------------------------------------------------------------------------
// InMemoryStorage – time metadata
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn memory_get_created_time_is_set_on_save() {
	// Arrange
	let storage = make_memory_storage();
	let before = chrono::Utc::now();
	storage.save("ts.txt", b"t").await.unwrap();
	let after = chrono::Utc::now();

	// Act
	let ctime = storage.get_created_time("ts.txt").await.unwrap();

	// Assert
	assert!(ctime >= before, "created_at should be >= before");
	assert!(ctime <= after, "created_at should be <= after");
}

#[rstest]
#[tokio::test]
async fn memory_get_modified_time_updates_on_overwrite() {
	// Arrange
	let storage = make_memory_storage();
	storage.save("mod.txt", b"v1").await.unwrap();
	let mtime1 = storage.get_modified_time("mod.txt").await.unwrap();

	// Act – overwrite
	storage.save("mod.txt", b"v2 longer content").await.unwrap();
	let mtime2 = storage.get_modified_time("mod.txt").await.unwrap();

	// Assert
	assert!(
		mtime2 >= mtime1,
		"modified_at should not decrease after overwrite"
	);
}

#[rstest]
#[tokio::test]
async fn memory_get_created_time_nonexistent_returns_not_found_error() {
	// Arrange
	let storage = make_memory_storage();

	// Act
	let result = storage.get_created_time("nope.txt").await;

	// Assert
	assert!(matches!(result, Err(StorageError::NotFound(_))));
}

// ---------------------------------------------------------------------------
// InMemoryStorage – deconstruct
// ---------------------------------------------------------------------------

#[rstest]
fn memory_deconstruct_returns_expected_fields() {
	// Arrange
	let storage = InMemoryStorage::new("my_location", "http://example.com/media");

	// Act
	let (path, _args, kwargs) = storage.deconstruct();

	// Assert
	assert_eq!(path, "reinhardt_storage.InMemoryStorage");
	assert_eq!(kwargs.get("location").unwrap(), "my_location");
	assert_eq!(kwargs.get("base_url").unwrap(), "http://example.com/media");
}

#[rstest]
fn memory_deconstruct_includes_permission_modes_when_set() {
	// Arrange
	let storage =
		InMemoryStorage::new("loc", "http://x.com/").with_permissions(Some(0o644), Some(0o755));

	// Act
	let (_path, _args, kwargs) = storage.deconstruct();

	// Assert
	assert_eq!(kwargs.get("file_permissions_mode").unwrap(), "0o644");
	assert_eq!(kwargs.get("directory_permissions_mode").unwrap(), "0o755");
}

// ---------------------------------------------------------------------------
// FileMetadata and StoredFile API
// ---------------------------------------------------------------------------

#[rstest]
fn file_metadata_new_sets_path_and_size() {
	// Arrange / Act
	let meta = FileMetadata::new("docs/readme.md".to_string(), 4096);

	// Assert
	assert_eq!(meta.path, "docs/readme.md");
	assert_eq!(meta.size, 4096);
	assert!(meta.checksum.is_none());
	assert!(meta.content_type.is_none());
}

#[rstest]
fn file_metadata_with_checksum_stores_value() {
	// Arrange
	let meta = FileMetadata::new("f.txt".to_string(), 1).with_checksum("abc123".to_string());

	// Act / Assert
	assert_eq!(meta.checksum, Some("abc123".to_string()));
}

#[rstest]
fn file_metadata_with_content_type_stores_value() {
	// Arrange
	let meta =
		FileMetadata::new("img.png".to_string(), 512).with_content_type("image/png".to_string());

	// Act / Assert
	assert_eq!(meta.content_type, Some("image/png".to_string()));
}

#[rstest]
fn stored_file_size_matches_content_length() {
	// Arrange
	let meta = FileMetadata::new("data.bin".to_string(), 100);
	let content = vec![0u8; 42];

	// Act
	let file = StoredFile::new(meta, content);

	// Assert
	assert_eq!(file.size(), 42);
}

#[rstest]
fn stored_file_content_is_accessible() {
	// Arrange
	let meta = FileMetadata::new("hello.txt".to_string(), 5);
	let content = b"hello".to_vec();

	// Act
	let file = StoredFile::new(meta, content.clone());

	// Assert
	assert_eq!(file.content, content);
}
