//! Integration tests for the static files pipeline
//!
//! Tests cover: file collection, hash calculation, finders,
//! manifest storage, processing pipeline, and dependency resolution.

use reinhardt_utils::staticfiles::processing::Processor;
use reinhardt_utils::staticfiles::processing::minify::{CssMinifier, JsMinifier};
use reinhardt_utils::staticfiles::{
	DependencyGraph, FileSystemStorage, HashedFileStorage, ManifestStaticFilesStorage,
	MemoryStorage, PathResolver, ProcessingConfig, ProcessingPipeline, StaticFilesConfig,
	StaticFilesFinder, Storage, StorageRegistry,
};
use rstest::rstest;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a temporary directory tree with some static files.
async fn setup_static_dir() -> TempDir {
	let dir = TempDir::new().expect("failed to create tempdir");
	let base = dir.path();

	fs::create_dir_all(base.join("css")).await.unwrap();
	fs::create_dir_all(base.join("js")).await.unwrap();
	fs::create_dir_all(base.join("images")).await.unwrap();

	fs::write(base.join("css/style.css"), b"body { color: red; }")
		.await
		.unwrap();
	fs::write(base.join("css/reset.css"), b"* { margin: 0; }")
		.await
		.unwrap();
	fs::write(base.join("js/app.js"), b"console.log('hello');")
		.await
		.unwrap();
	fs::write(base.join("images/logo.png"), b"\x89PNG\r\n\x1a\n")
		.await
		.unwrap();

	dir
}

// ---------------------------------------------------------------------------
// StaticFilesFinder tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_finder_finds_existing_file() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let file_path = dir.path().join("style.css");
	std::fs::write(&file_path, b"body{}").unwrap();

	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let result = finder.find("style.css");

	// Assert
	assert!(result.is_ok(), "finder should locate existing file");
	assert_eq!(
		result.unwrap().canonicalize().unwrap(),
		file_path.canonicalize().unwrap()
	);
}

#[rstest]
fn test_finder_returns_error_for_missing_file() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let result = finder.find("nonexistent.css");

	// Assert
	assert!(
		result.is_err(),
		"finder should return error for missing file"
	);
}

#[rstest]
fn test_finder_searches_multiple_directories() {
	// Arrange
	let dir1 = TempDir::new().unwrap();
	let dir2 = TempDir::new().unwrap();
	std::fs::write(dir2.path().join("found.css"), b"body{}").unwrap();

	let finder = StaticFilesFinder::new(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);

	// Act
	let result = finder.find("found.css");

	// Assert
	assert!(
		result.is_ok(),
		"finder should locate file in second directory"
	);
}

#[rstest]
fn test_finder_strips_leading_slash_from_path() {
	// Arrange
	let dir = TempDir::new().unwrap();
	std::fs::write(dir.path().join("style.css"), b"body{}").unwrap();
	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let result = finder.find("/style.css");

	// Assert
	assert!(
		result.is_ok(),
		"finder should strip leading slash and find file"
	);
}

#[rstest]
fn test_finder_find_all_returns_all_files() {
	// Arrange
	let dir = TempDir::new().unwrap();
	std::fs::create_dir(dir.path().join("sub")).unwrap();
	std::fs::write(dir.path().join("a.css"), b"body{}").unwrap();
	std::fs::write(dir.path().join("b.js"), b"var x=1;").unwrap();
	std::fs::write(dir.path().join("sub/c.css"), b"p{}").unwrap();

	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let files = finder.find_all();

	// Assert
	assert_eq!(files.len(), 3, "find_all should return all 3 files");
}

#[rstest]
fn test_finder_find_all_empty_directory() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let files = finder.find_all();

	// Assert
	assert!(
		files.is_empty(),
		"find_all should return empty vec for empty directory"
	);
}

#[rstest]
fn test_finder_blocks_path_traversal() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let finder = StaticFilesFinder::new(vec![dir.path().to_path_buf()]);

	// Act
	let result = finder.find("../../etc/passwd");

	// Assert
	assert!(result.is_err(), "finder should block path traversal");
}

// ---------------------------------------------------------------------------
// MemoryStorage tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_memory_storage_save_and_open() {
	// Arrange
	let storage = MemoryStorage::new("/static/");

	// Act
	let url = storage.save("style.css", b"body{}").await.unwrap();
	let content = storage.open("style.css").await.unwrap();

	// Assert
	assert_eq!(url, "/static/style.css");
	assert_eq!(content, b"body{}");
}

#[rstest]
#[tokio::test]
async fn test_memory_storage_exists() {
	// Arrange
	let storage = MemoryStorage::new("/static/");
	storage.save("app.js", b"var x=1;").await.unwrap();

	// Act
	let exists_saved = storage.exists("app.js");
	let exists_missing = storage.exists("missing.js");

	// Assert
	assert!(exists_saved, "saved file should exist");
	assert!(!exists_missing, "missing file should not exist");
}

#[rstest]
#[tokio::test]
async fn test_memory_storage_delete() {
	// Arrange
	let storage = MemoryStorage::new("/static/");
	storage.save("temp.css", b"temp{}").await.unwrap();
	assert!(storage.exists("temp.css"));

	// Act
	storage.delete("temp.css").await.unwrap();

	// Assert
	assert!(!storage.exists("temp.css"), "deleted file should not exist");
}

#[rstest]
#[tokio::test]
async fn test_memory_storage_open_missing_returns_error() {
	// Arrange
	let storage = MemoryStorage::new("/static/");

	// Act
	let result = storage.open("nonexistent.css").await;

	// Assert
	assert!(result.is_err(), "opening missing file should return error");
}

#[rstest]
#[tokio::test]
async fn test_memory_storage_url_format() {
	// Arrange
	let storage = MemoryStorage::new("/assets/");

	// Act
	let url = storage.url("style.css");

	// Assert
	assert_eq!(url, "/assets/style.css");
}

// ---------------------------------------------------------------------------
// FileSystemStorage tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_filesystem_storage_save_and_open() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = FileSystemStorage::new(dir.path(), "/static/");

	// Act
	storage
		.save("style.css", b"body { color: blue; }")
		.await
		.unwrap();
	let content = storage.open("style.css").await.unwrap();

	// Assert
	assert_eq!(content, b"body { color: blue; }");
}

#[rstest]
#[tokio::test]
async fn test_filesystem_storage_creates_parent_directories() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = FileSystemStorage::new(dir.path(), "/static/");

	// Act
	let result = storage.save("subdir/nested/style.css", b"body{}").await;

	// Assert
	assert!(result.is_ok(), "save should create parent directories");
	assert!(dir.path().join("subdir/nested/style.css").exists());
}

#[rstest]
#[tokio::test]
async fn test_filesystem_storage_exists() {
	// Arrange
	let dir = TempDir::new().unwrap();
	std::fs::write(dir.path().join("existing.css"), b"body{}").unwrap();
	let storage = FileSystemStorage::new(dir.path(), "/static/");

	// Act
	let exists = storage.exists("existing.css");
	let not_exists = storage.exists("missing.css");

	// Assert
	assert!(exists, "existing file should be found");
	assert!(!not_exists, "missing file should not be found");
}

#[rstest]
#[tokio::test]
async fn test_filesystem_storage_delete() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let file_path = dir.path().join("to_delete.css");
	std::fs::write(&file_path, b"body{}").unwrap();
	let storage = FileSystemStorage::new(dir.path(), "/static/");

	// Act
	storage.delete("to_delete.css").await.unwrap();

	// Assert
	assert!(!file_path.exists(), "file should be deleted");
}

#[rstest]
#[tokio::test]
async fn test_filesystem_storage_blocks_path_traversal() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = FileSystemStorage::new(dir.path(), "/static/");

	// Act - attempt path traversal
	let result = storage.open("../../etc/passwd").await;

	// Assert
	assert!(result.is_err(), "path traversal should be blocked");
}

// ---------------------------------------------------------------------------
// HashedFileStorage tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_hashed_storage_produces_hashed_name() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = HashedFileStorage::new(dir.path(), "/static/");

	// Act
	let hashed_name = storage
		.save("style.css", b"body { color: red; }")
		.await
		.unwrap();

	// Assert
	assert!(
		hashed_name.ends_with(".css"),
		"hashed name should retain extension"
	);
	assert!(
		hashed_name.contains('.'),
		"hashed name should contain dot before extension"
	);
	// original name != hashed name (hash injected)
	assert_ne!(hashed_name, "style.css");
}

#[rstest]
#[tokio::test]
async fn test_hashed_storage_same_content_same_hash() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = HashedFileStorage::new(dir.path(), "/static/");
	let content = b"body { color: red; }";

	// Act
	let name1 = storage.save("a.css", content).await.unwrap();
	// Use a fresh storage so there's no name collision
	let dir2 = TempDir::new().unwrap();
	let storage2 = HashedFileStorage::new(dir2.path(), "/static/");
	let name2 = storage2.save("b.css", content).await.unwrap();

	// Assert - both should embed the same hash fragment
	let hash1 = name1.trim_start_matches("a.").trim_end_matches(".css");
	let hash2 = name2.trim_start_matches("b.").trim_end_matches(".css");
	assert_eq!(
		hash1, hash2,
		"same content should produce same hash fragment"
	);
}

#[rstest]
#[tokio::test]
async fn test_hashed_storage_get_hashed_path() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = HashedFileStorage::new(dir.path(), "/static/");
	storage.save("style.css", b"body{}").await.unwrap();

	// Act
	let hashed = storage.get_hashed_path("style.css");

	// Assert
	assert!(
		hashed.is_some(),
		"get_hashed_path should return value after save"
	);
	assert!(hashed.unwrap().ends_with(".css"));
}

#[rstest]
#[tokio::test]
async fn test_hashed_storage_url_returns_hashed_url() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = HashedFileStorage::new(dir.path(), "/static/");
	storage.save("app.js", b"var x=1;").await.unwrap();

	// Act
	let url = storage.url("app.js");

	// Assert
	assert!(
		url.starts_with("/static/"),
		"url should start with base_url"
	);
	assert!(url.ends_with(".js"), "url should end with .js");
	assert_ne!(url, "/static/app.js", "url should include hash");
}

#[rstest]
#[tokio::test]
async fn test_hashed_storage_save_with_dependencies_updates_css_refs() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = HashedFileStorage::new(dir.path(), "/static/");

	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	files.insert("logo.png".into(), b"\x89PNG\r\n\x1a\n".to_vec());
	files.insert(
		"style.css".into(),
		b"body { background: url('logo.png'); }".to_vec(),
	);

	// Act
	let count = storage.save_with_dependencies(files).await.unwrap();

	// Assert
	assert_eq!(count, 2, "should have processed 2 files");
	// Verify CSS file was updated with hashed reference
	let hashed_css_path = storage.get_hashed_path("style.css").unwrap();
	let hashed_logo = storage.get_hashed_path("logo.png").unwrap();
	let css_content = tokio::fs::read(dir.path().join(&hashed_css_path))
		.await
		.unwrap();
	let css_str = String::from_utf8(css_content).unwrap();
	assert!(
		css_str.contains(&hashed_logo),
		"CSS should contain hashed logo path"
	);
}

// ---------------------------------------------------------------------------
// ManifestStaticFilesStorage tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_manifest_storage_saves_and_loads_manifest() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = ManifestStaticFilesStorage::new(dir.path(), "/static/");
	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	files.insert("style.css".into(), b"body{}".to_vec());
	files.insert("app.js".into(), b"var x=1;".to_vec());

	// Act
	storage.save_with_dependencies(files).await.unwrap();

	// Reload manifest into fresh storage
	let storage2 = ManifestStaticFilesStorage::new(dir.path(), "/static/");
	storage2.load_manifest().await.unwrap();

	// Assert
	let path_in_new = storage2.get_hashed_path("style.css");
	assert!(
		path_in_new.is_some(),
		"loaded manifest should contain style.css mapping"
	);
}

#[rstest]
#[tokio::test]
async fn test_manifest_storage_manifest_file_exists() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = ManifestStaticFilesStorage::new(dir.path(), "/static/");
	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	files.insert("style.css".into(), b"body{}".to_vec());

	// Act
	storage.save_with_dependencies(files).await.unwrap();

	// Assert
	let manifest_path = dir.path().join("staticfiles.json");
	assert!(
		manifest_path.exists(),
		"manifest file should be written to disk"
	);
}

#[rstest]
#[tokio::test]
async fn test_manifest_storage_url_uses_hashed_name() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = ManifestStaticFilesStorage::new(dir.path(), "/static/");
	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	files.insert("style.css".into(), b"body{}".to_vec());
	storage.save_with_dependencies(files).await.unwrap();

	// Act
	let url = storage.url("style.css");

	// Assert
	assert!(
		url.starts_with("/static/"),
		"url should start with base_url"
	);
	assert!(url.ends_with(".css"), "url should end with .css");
}

#[rstest]
#[tokio::test]
async fn test_manifest_storage_open_by_original_name() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = ManifestStaticFilesStorage::new(dir.path(), "/static/");
	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	files.insert("app.js".into(), b"var x = 42;".to_vec());
	storage.save_with_dependencies(files).await.unwrap();

	// Act
	let content = storage.open("app.js").await.unwrap();

	// Assert
	assert_eq!(content, b"var x = 42;", "should open file by original name");
}

#[rstest]
#[tokio::test]
async fn test_manifest_storage_load_from_empty_returns_ok() {
	// Arrange
	let dir = TempDir::new().unwrap();
	let storage = ManifestStaticFilesStorage::new(dir.path(), "/static/");

	// Act
	let result = storage.load_manifest().await;

	// Assert
	assert!(result.is_ok(), "loading missing manifest should be OK");
}

// ---------------------------------------------------------------------------
// ProcessingConfig tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_processing_config_default_values() {
	// Arrange & Act
	let config = ProcessingConfig::default();

	// Assert
	assert!(config.minify, "minification should be enabled by default");
	assert!(
		!config.source_maps,
		"source maps should be disabled by default"
	);
	assert!(
		config.optimize_images,
		"image optimization should be enabled by default"
	);
	assert_eq!(
		config.image_quality, 85,
		"default image quality should be 85"
	);
}

#[rstest]
fn test_processing_config_builder_chain() {
	// Arrange & Act
	let config = ProcessingConfig::new(PathBuf::from("/tmp/dist"))
		.with_minification(false)
		.with_source_maps(true)
		.with_image_optimization(false)
		.with_image_quality(95);

	// Assert
	assert!(!config.minify);
	assert!(config.source_maps);
	assert!(!config.optimize_images);
	assert_eq!(config.image_quality, 95);
}

#[rstest]
fn test_processing_config_image_quality_clamped_high() {
	// Arrange & Act
	let config = ProcessingConfig::default().with_image_quality(200);

	// Assert
	assert_eq!(
		config.image_quality, 100,
		"quality should be clamped to 100"
	);
}

#[rstest]
fn test_processing_config_image_quality_clamped_low() {
	// Arrange & Act
	let config = ProcessingConfig::default().with_image_quality(0);

	// Assert
	assert_eq!(
		config.image_quality, 1,
		"quality should be clamped to at least 1"
	);
}

// ---------------------------------------------------------------------------
// ProcessingPipeline tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_pipeline_passes_through_unknown_extension() {
	// Arrange
	let config = ProcessingConfig::new(PathBuf::from("/tmp/dist"));
	let pipeline = ProcessingPipeline::new(config);
	let input = b"<html><body></body></html>";

	// Act
	let result = pipeline
		.process_file(input, &PathBuf::from("index.html"))
		.await
		.unwrap();

	// Assert
	assert_eq!(
		result, input,
		"unknown file type should pass through unchanged"
	);
}

#[rstest]
#[tokio::test]
async fn test_pipeline_minifies_css_file() {
	// Arrange
	let config = ProcessingConfig::new(PathBuf::from("/tmp/dist")).with_minification(true);
	let pipeline = ProcessingPipeline::new(config);
	let input = b"/* comment */\nbody {\n  color: red;\n}";

	// Act
	let result = pipeline
		.process_file(input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	// Assert
	let output = String::from_utf8(result).unwrap();
	assert!(
		!output.contains("comment"),
		"comment should be removed by minification"
	);
	assert!(output.len() < input.len(), "minified CSS should be smaller");
}

#[rstest]
#[tokio::test]
async fn test_pipeline_minifies_js_file() {
	// Arrange
	let config = ProcessingConfig::new(PathBuf::from("/tmp/dist")).with_minification(true);
	let pipeline = ProcessingPipeline::new(config);
	let input = b"// single line comment\nconst x = 1;";

	// Act
	let result = pipeline
		.process_file(input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	// Assert
	let output = String::from_utf8(result).unwrap();
	assert!(
		!output.contains("single line comment"),
		"comment should be removed"
	);
}

#[rstest]
#[tokio::test]
async fn test_pipeline_skips_minification_when_disabled() {
	// Arrange
	let config = ProcessingConfig::new(PathBuf::from("/tmp/dist")).with_minification(false);
	let pipeline = ProcessingPipeline::new(config);
	let input = b"/* comment */ body { color: red; }";

	// Act
	let result = pipeline
		.process_file(input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	// Assert
	assert_eq!(
		result, input,
		"minification disabled: output should equal input"
	);
}

// ---------------------------------------------------------------------------
// CssMinifier / JsMinifier processor tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_css_minifier_can_process_css() {
	// Arrange
	let minifier = CssMinifier::new();

	// Act & Assert
	assert!(minifier.can_process(&PathBuf::from("style.css")));
	assert!(minifier.can_process(&PathBuf::from("STYLE.CSS")));
	assert!(!minifier.can_process(&PathBuf::from("app.js")));
	assert!(!minifier.can_process(&PathBuf::from("index.html")));
}

#[rstest]
#[tokio::test]
async fn test_js_minifier_can_process_js() {
	// Arrange
	let minifier = JsMinifier::new();

	// Act & Assert
	assert!(minifier.can_process(&PathBuf::from("app.js")));
	assert!(minifier.can_process(&PathBuf::from("APP.JS")));
	assert!(!minifier.can_process(&PathBuf::from("style.css")));
}

#[rstest]
#[tokio::test]
async fn test_css_minifier_removes_block_comments() {
	// Arrange
	let minifier = CssMinifier::new();
	let input = b"/* This is a comment */ body { color: red; }";

	// Act
	let result = minifier
		.process(input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	// Assert
	let output = String::from_utf8(result).unwrap();
	assert!(!output.contains("This is a comment"));
	assert!(output.contains("color"));
}

#[rstest]
#[tokio::test]
async fn test_js_minifier_preserves_string_contents() {
	// Arrange
	let minifier = JsMinifier::new();
	let input = b"const url = '// not a comment';";

	// Act
	let result = minifier
		.process(input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	// Assert
	let output = String::from_utf8(result).unwrap();
	assert!(
		output.contains("// not a comment"),
		"string literal should be preserved"
	);
}

// ---------------------------------------------------------------------------
// DependencyGraph tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_dependency_graph_add_and_resolve() {
	// Arrange
	let mut graph = DependencyGraph::new();
	graph.add_file("main.js".into());
	graph.add_file("utils.js".into());
	graph.add_dependency("main.js".into(), "utils.js".into());

	// Act
	let order = graph.resolve_order();

	// Assert
	assert!(order.len() >= 2, "resolve should return at least 2 files");
	let utils_pos = order.iter().position(|x| x == "utils.js").unwrap();
	let main_pos = order.iter().position(|x| x == "main.js").unwrap();
	assert!(
		utils_pos < main_pos,
		"dependency should come before dependent"
	);
}

#[rstest]
fn test_dependency_graph_empty() {
	// Arrange
	let graph = DependencyGraph::new();

	// Act
	let order = graph.resolve_order();

	// Assert
	assert!(order.is_empty(), "empty graph should produce empty order");
}

#[rstest]
fn test_dependency_graph_len() {
	// Arrange
	let mut graph = DependencyGraph::new();
	graph.add_file("a.js".into());
	graph.add_file("b.js".into());

	// Act & Assert
	assert_eq!(graph.len(), 2);
	assert!(!graph.is_empty());
}

// ---------------------------------------------------------------------------
// PathResolver tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_path_resolver_returns_absolute_unchanged() {
	// Arrange
	let absolute = "/tmp/myproject/static";

	// Act
	let resolved = PathResolver::resolve_static_dir(absolute);

	// Assert
	assert_eq!(
		resolved,
		PathBuf::from(absolute),
		"absolute path should be returned as-is"
	);
}

#[rstest]
fn test_path_resolver_relative_does_not_panic() {
	// Arrange & Act
	// Should not panic regardless of environment
	let resolved = PathResolver::resolve_static_dir("nonexistent_static_dir");

	// Assert
	assert!(
		resolved
			.to_string_lossy()
			.contains("nonexistent_static_dir"),
		"resolved path should contain the requested dir name"
	);
}

// ---------------------------------------------------------------------------
// StaticFilesConfig tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_static_files_config_default() {
	// Arrange & Act
	let config = StaticFilesConfig::default();

	// Assert
	assert_eq!(config.static_root, PathBuf::from("static"));
	assert_eq!(config.static_url, "/static/");
	assert!(config.staticfiles_dirs.is_empty());
	assert!(config.media_url.is_none());
}

// ---------------------------------------------------------------------------
// StorageRegistry tests
// ---------------------------------------------------------------------------

#[rstest]
fn test_storage_registry_register_and_get() {
	// Arrange
	let mut registry = StorageRegistry::new();

	// Act
	let _ = registry.register(
		"mem",
		Box::new(|| std::sync::Arc::new(MemoryStorage::new("/static/"))),
	);
	let storage = registry.get("mem");

	// Assert
	assert!(
		storage.is_some(),
		"registered backend should be retrievable"
	);
}

#[rstest]
fn test_storage_registry_missing_returns_none() {
	// Arrange
	let registry = StorageRegistry::new();

	// Act
	let result = registry.get("nonexistent");

	// Assert
	assert!(result.is_none(), "unregistered backend should return None");
}

// ---------------------------------------------------------------------------
// End-to-end pipeline: collect, hash, manifest
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_end_to_end_collect_hash_and_manifest() {
	// Arrange
	let src_dir = setup_static_dir().await;
	let out_dir = TempDir::new().unwrap();

	let finder = StaticFilesFinder::new(vec![src_dir.path().to_path_buf()]);
	let storage = ManifestStaticFilesStorage::new(out_dir.path(), "/static/");

	// Collect files
	let file_names = finder.find_all();
	assert!(!file_names.is_empty(), "should find static files");

	// Build file content map
	let mut files: HashMap<String, Vec<u8>> = HashMap::new();
	for name in &file_names {
		let full_path = finder.find(name).unwrap();
		let content = std::fs::read(&full_path).unwrap();
		files.insert(name.clone(), content);
	}

	// Act
	let count = storage.save_with_dependencies(files).await.unwrap();

	// Assert
	assert_eq!(
		count,
		file_names.len(),
		"all found files should be processed"
	);
	assert!(
		out_dir.path().join("staticfiles.json").exists(),
		"manifest should be written"
	);
	// Verify each file has a hashed path
	for name in &file_names {
		let hashed = storage.get_hashed_path(name);
		assert!(
			hashed.is_some(),
			"file '{}' should have a hashed path",
			name
		);
	}
}
