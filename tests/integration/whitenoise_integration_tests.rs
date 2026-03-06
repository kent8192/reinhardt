//! Integration tests for reinhardt-whitenoise

use reinhardt_whitenoise::config::WhiteNoiseConfig;
use reinhardt_whitenoise::compression::{FileScanner, WhiteNoiseCompressor};
use reinhardt_whitenoise::cache::{FileCache, FileMetadata};
use rstest::rstest;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

#[rstest]
#[tokio::test]
async fn test_end_to_end_compression_pipeline() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path().join("static");
	fs::create_dir(&static_root).unwrap();

	// Create test files
	let css_path = static_root.join("app.css");
	let mut css_file = File::create(&css_path).unwrap();
	writeln!(css_file, "{}", "body { color: red; }".repeat(100)).unwrap();

	let js_path = static_root.join("app.js");
	let mut js_file = File::create(&js_path).unwrap();
	writeln!(js_file, "{}", "console.log('test');".repeat(100)).unwrap();

	// Scan for files
	let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string());
	let scanner = FileScanner::new(config.clone());
	let files = scanner.scan().unwrap();

	assert_eq!(files.len(), 2);

	// Compress files
	let compressor = WhiteNoiseCompressor::new(config);
	let results = compressor.compress_batch(files).await.unwrap();

	assert_eq!(results.len(), 2);

	// Update cache
	let mut cache = FileCache::new();
	for (path, variants) in results {
		let metadata = FileMetadata::from_path(&path).unwrap();
		let relative_path = path.strip_prefix(&static_root).unwrap().to_str().unwrap();
		cache.insert_file(relative_path.to_string(), metadata);
		cache.insert_compressed(relative_path.to_string(), variants);
	}

	// Verify cache
	assert!(cache.get("app.css").is_some());
	assert!(cache.get("app.js").is_some());
	assert!(cache.get_compressed("app.css").unwrap().has_variants());
	assert!(cache.get_compressed("app.js").unwrap().has_variants());
}

#[rstest]
fn test_cache_workflow_with_manifest() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path().join("static");
	fs::create_dir(&static_root).unwrap();

	// Create manifest
	let manifest_path = temp_dir.path().join("manifest.json");
	let manifest = r#"{
		"app.js": "app.abc123def456.js",
		"style.css": "style.1234567890ab.css"
	}"#;
	fs::write(&manifest_path, manifest).unwrap();

	// Create hashed files
	let hashed_js = static_root.join("app.abc123def456.js");
	fs::write(&hashed_js, "console.log('test');").unwrap();

	// Load manifest and resolve
	let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string())
		.with_manifest(manifest_path);

	let mut cache = FileCache::new();
	if let Some(manifest_path) = &config.manifest_path {
		cache.load_manifest(manifest_path).unwrap();
	}

	// Test resolution
	assert_eq!(cache.resolve("app.js"), "app.abc123def456.js");
	assert_eq!(cache.resolve("style.css"), "style.1234567890ab.css");
	assert_eq!(cache.resolve("other.js"), "other.js");

	// Test metadata retrieval
	let metadata = FileMetadata::from_path(&hashed_js).unwrap();
	cache.insert_file("app.abc123def456.js".to_string(), metadata);

	assert!(cache.get("app.abc123def456.js").is_some());
}
