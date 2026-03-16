//! Cache module tests

use reinhardt_whitenoise::cache::{CompressedVariants, FileCache, FileMetadata};
use rstest::rstest;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[rstest]
fn test_file_metadata_from_path() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("test.css");
	let mut file = File::create(&file_path).unwrap();
	writeln!(file, "body {{ color: red; }}").unwrap();

	let metadata = FileMetadata::from_path(&file_path).unwrap();
	assert!(metadata.size > 0);
	assert!(!metadata.etag.is_empty());
	assert!(metadata.mime_type.contains("css"));
	assert_eq!(metadata.path, file_path);
}

#[rstest]
fn test_file_metadata_etag_format() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("test.txt");
	std::fs::write(&file_path, "test content").unwrap();

	let metadata = FileMetadata::from_path(&file_path).unwrap();
	// ETag format: {timestamp_hex}-{size_hex}
	assert!(metadata.etag.contains('-'));
	let parts: Vec<&str> = metadata.etag.split('-').collect();
	assert_eq!(parts.len(), 2);
}

#[rstest]
fn test_compressed_variants_builder() {
	let variants = CompressedVariants::new()
		.with_gzip(std::path::PathBuf::from("app.js.gz"))
		.with_brotli(std::path::PathBuf::from("app.js.br"));

	assert!(variants.gzip.is_some());
	assert!(variants.brotli.is_some());
	assert!(variants.has_variants());
}

#[rstest]
fn test_compressed_variants_empty() {
	let variants = CompressedVariants::new();
	assert!(!variants.has_variants());
}

#[rstest]
fn test_file_cache_operations() {
	let temp_dir = TempDir::new().unwrap();
	let file_path = temp_dir.path().join("test.css");
	File::create(&file_path).unwrap();

	let mut cache = FileCache::new();
	let metadata = FileMetadata::from_path(&file_path).unwrap();

	cache.insert_file("test.css".to_string(), metadata);

	assert!(cache.get("test.css").is_some());
	assert!(cache.get("nonexistent.css").is_none());
}

#[rstest]
fn test_file_cache_compressed_variants() {
	let mut cache = FileCache::new();
	let variants = CompressedVariants::new().with_gzip(std::path::PathBuf::from("app.js.gz"));

	cache.insert_compressed("app.js".to_string(), variants);

	assert!(cache.get_compressed("app.js").is_some());
	assert!(cache.get_compressed("app.js").unwrap().gzip.is_some());
}

#[rstest]
fn test_file_cache_resolve_manifest() {
	let mut cache = FileCache::new();
	cache
		.manifest
		.insert("app.js".to_string(), "app.abc123def456.js".to_string());

	assert_eq!(cache.resolve("app.js"), "app.abc123def456.js");
	assert_eq!(cache.resolve("other.js"), "other.js");
}

#[rstest]
fn test_file_cache_load_manifest() {
	let temp_dir = TempDir::new().unwrap();
	let manifest_path = temp_dir.path().join("manifest.json");
	let manifest_content = r#"{
		"app.js": "app.abc123def456.js",
		"style.css": "style.1234567890ab.css"
	}"#;
	std::fs::write(&manifest_path, manifest_content).unwrap();

	let mut cache = FileCache::new();
	cache.load_manifest(&manifest_path).unwrap();

	assert_eq!(cache.resolve("app.js"), "app.abc123def456.js");
	assert_eq!(cache.resolve("style.css"), "style.1234567890ab.css");
}

#[rstest]
fn test_file_cache_load_invalid_manifest() {
	let temp_dir = TempDir::new().unwrap();
	let manifest_path = temp_dir.path().join("manifest.json");
	std::fs::write(&manifest_path, "invalid json").unwrap();

	let mut cache = FileCache::new();
	assert!(cache.load_manifest(&manifest_path).is_err());
}
