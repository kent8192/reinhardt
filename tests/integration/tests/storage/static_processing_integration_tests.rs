//! Integration tests for advanced file processing features
//!
//! Tests image optimization, source maps, compression, and their integration
//! with the processing pipeline.

use reinhardt_utils::staticfiles::processing::{
	ProcessingConfig, ProcessingPipeline, Processor,
	bundle::{AssetBundler, BundleConfig},
	compress::{BrotliCompressor, CompressionConfig, GzipCompressor},
	minify::{CssMinifier, JsMinifier},
};
use std::path::PathBuf;
use tempfile::tempdir;

#[cfg(feature = "image-optimization")]
use reinhardt_utils::staticfiles::processing::images::ImageOptimizer;

#[cfg(feature = "source-maps")]
use reinhardt_utils::staticfiles::processing::sourcemap::{
	SourceMap, SourceMapGenerator, SourceMapMerger,
};

// ========== Minification Integration Tests ==========

#[tokio::test]
async fn test_css_minification_integration() {
	let minifier = CssMinifier::new();
	let input = b"/* Comment */\nbody {\n  color: red;\n  margin: 0;\n}\n";
	let result = minifier
		.process(input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	let output = String::from_utf8(result).unwrap();
	assert!(!output.contains("Comment"));
	assert!(output.len() < input.len());
}

#[tokio::test]
async fn test_js_minification_integration() {
	let minifier = JsMinifier::new();
	let input = b"// Comment\nconst x = 1;\nconst y = 2;\n";
	let result = minifier
		.process(input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	let output = String::from_utf8(result).unwrap();
	assert!(!output.contains("// Comment"));
}

// ========== Bundling Integration Tests ==========

#[tokio::test]
async fn test_bundle_with_dependencies() {
	let mut bundler = AssetBundler::new();
	bundler.add_file(PathBuf::from("main.js"), b"import './utils.js';".to_vec());
	bundler.add_file(
		PathBuf::from("utils.js"),
		b"export const fn = () => {};".to_vec(),
	);
	bundler.add_dependency(PathBuf::from("main.js"), PathBuf::from("utils.js"));

	let result = bundler.bundle().unwrap();
	let output = String::from_utf8(result).unwrap();

	// utils.js should come before main.js
	let utils_pos = output.find("utils.js").unwrap();
	let main_pos = output.find("main.js").unwrap();
	assert!(utils_pos < main_pos);
}

#[tokio::test]
async fn test_bundle_config_usage() {
	let mut config = BundleConfig::new(PathBuf::from("bundle.js"))
		.with_minify(true)
		.with_source_map(true);

	config.add_file(PathBuf::from("a.js"));
	config.add_file(PathBuf::from("b.js"));

	assert_eq!(config.files.len(), 2);
	assert!(config.minify);
	assert!(config.source_map);
}

// ========== Compression Integration Tests ==========

#[tokio::test]
async fn test_gzip_compression() {
	let compressor = GzipCompressor::new();
	let input = b"Hello, World! This is test data for gzip compression. It should be smaller after compression.";
	let result = compressor
		.process(input, &PathBuf::from("test.txt"))
		.await
		.unwrap();

	assert!(!result.is_empty());
	// Gzip adds header, so very short inputs might be larger
	// For longer inputs, result should be smaller
}

#[tokio::test]
async fn test_brotli_compression() {
	let compressor = BrotliCompressor::new();
	let input = b"Hello, World! This is test data for brotli compression. It should be smaller after compression.";
	let result = compressor
		.process(input, &PathBuf::from("test.txt"))
		.await
		.unwrap();

	assert!(!result.is_empty());
}

#[tokio::test]
async fn test_compression_config() {
	let config = CompressionConfig::new()
		.with_gzip(true)
		.with_brotli(true)
		.with_min_size(1024)
		.add_extension("txt".to_string());

	// Should compress large files
	assert!(config.should_compress(&PathBuf::from("large.js"), 2000));

	// Should not compress small files
	assert!(!config.should_compress(&PathBuf::from("small.js"), 500));

	// Should compress configured extensions
	assert!(config.should_compress(&PathBuf::from("file.txt"), 2000));
}

#[tokio::test]
async fn test_gzip_levels() {
	let input = b"Test data for compression level testing. This text repeats. Test data for compression level testing.";

	let compressor_fast = GzipCompressor::with_level(1);
	let result_fast = compressor_fast
		.process(input, &PathBuf::from("test.txt"))
		.await
		.unwrap();

	let compressor_best = GzipCompressor::with_level(9);
	let result_best = compressor_best
		.process(input, &PathBuf::from("test.txt"))
		.await
		.unwrap();

	// Both should produce output
	assert!(!result_fast.is_empty());
	assert!(!result_best.is_empty());
}

// ========== Image Optimization Tests (Feature Gated) ==========

#[cfg(feature = "image-optimization")]
#[tokio::test]
async fn test_image_optimizer_creation() {
	let optimizer = ImageOptimizer::new(85);
	assert!(optimizer.can_process(&PathBuf::from("test.png")));
	assert!(optimizer.can_process(&PathBuf::from("test.jpg")));
	assert!(optimizer.can_process(&PathBuf::from("test.webp")));
}

#[cfg(feature = "image-optimization")]
#[tokio::test]
async fn test_image_optimizer_process_png() {
	let optimizer = ImageOptimizer::new(85);
	let input = b"fake png data";
	let result = optimizer
		.process(input, &PathBuf::from("test.png"))
		.await
		.unwrap();
	assert!(!result.is_empty());
}

// ========== Source Map Tests (Feature Gated) ==========

#[cfg(feature = "source-maps")]
#[test]
fn test_source_map_creation() {
	let map = SourceMap::new("app.min.js".to_string());
	assert_eq!(map.version, 3);
	assert_eq!(map.file, "app.min.js");
}

#[cfg(feature = "source-maps")]
#[test]
fn test_source_map_serialization() {
	let mut map = SourceMap::new("app.min.js".to_string());
	map.add_source("src/app.js".to_string());
	map.add_source_content("const x = 1;".to_string());

	let json = map.to_json().unwrap();

	// Deserialize to verify exact structure
	let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
	assert_eq!(
		parsed["version"].as_i64(),
		Some(3),
		"SourceMap version should be 3"
	);
	assert_eq!(
		parsed["file"].as_str(),
		Some("app.min.js"),
		"SourceMap file should be 'app.min.js'"
	);
	assert!(
		parsed.get("sources").is_some(),
		"SourceMap should contain 'sources' field"
	);
	assert!(
		parsed["sources"].is_array(),
		"SourceMap 'sources' should be an array"
	);
}

#[cfg(feature = "source-maps")]
#[test]
fn test_source_map_generator() {
	let generator = SourceMapGenerator::new();
	let map = generator.generate_for_file(
		&PathBuf::from("dist/app.min.js"),
		&PathBuf::from("src/app.js"),
		"const x = 1;",
	);

	assert_eq!(map.file, "app.min.js");
	assert_eq!(map.sources.len(), 1);
	assert!(map.sources_content.is_some());
}

#[cfg(feature = "source-maps")]
#[test]
fn test_source_map_comment_generation() {
	let generator = SourceMapGenerator::new();
	let comment = generator.generate_comment("app.min.js.map");
	assert_eq!(comment, "//# sourceMappingURL=app.min.js.map");
}

#[cfg(feature = "source-maps")]
#[test]
fn test_source_map_merger() {
	let mut merger = SourceMapMerger::new();

	let mut map1 = SourceMap::new("app1.min.js".to_string());
	map1.add_source("src/app1.js".to_string());
	merger.add_map(map1);

	let mut map2 = SourceMap::new("app2.min.js".to_string());
	map2.add_source("src/app2.js".to_string());
	merger.add_map(map2);

	let merged = merger.merge("bundle.min.js".to_string());
	assert_eq!(merged.file, "bundle.min.js");
	assert_eq!(merged.sources.len(), 2);
}

// ========== Processing Pipeline Integration Tests ==========

#[tokio::test]
async fn test_processing_pipeline_creation() {
	let config = ProcessingConfig::new(PathBuf::from("dist"))
		.with_minification(true)
		.with_image_optimization(false);

	let pipeline = ProcessingPipeline::new(config);
	assert!(pipeline.config().minify);
}

#[tokio::test]
async fn test_pipeline_css_processing() {
	let config = ProcessingConfig::new(PathBuf::from("dist")).with_minification(true);
	let pipeline = ProcessingPipeline::new(config);

	let input = b"body { color: red; }";
	let result = pipeline
		.process_file(input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	assert!(!result.is_empty());
}

#[tokio::test]
async fn test_pipeline_js_processing() {
	let config = ProcessingConfig::new(PathBuf::from("dist")).with_minification(true);
	let pipeline = ProcessingPipeline::new(config);

	let input = b"const x = 1;";
	let result = pipeline
		.process_file(input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	assert!(!result.is_empty());
}

#[tokio::test]
async fn test_pipeline_unprocessed_file() {
	let config = ProcessingConfig::new(PathBuf::from("dist"));
	let pipeline = ProcessingPipeline::new(config);

	let input = b"plain text";
	let result = pipeline
		.process_file(input, &PathBuf::from("readme.txt"))
		.await
		.unwrap();

	// Should return input unchanged
	assert_eq!(result, input);
}

// ========== End-to-End Workflow Tests ==========

#[tokio::test]
async fn test_minify_and_bundle_workflow() {
	let css_minifier = CssMinifier::new();
	let js_minifier = JsMinifier::new();

	// Minify files first
	let css_input = b"body { color: red; }";
	let css_minified = css_minifier
		.process(css_input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	let js_input = b"const x = 1;";
	let js_minified = js_minifier
		.process(js_input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	// Bundle minified files
	let mut bundler = AssetBundler::new();
	bundler.add_file(PathBuf::from("style.css"), css_minified);
	bundler.add_file(PathBuf::from("app.js"), js_minified);

	let bundle = bundler.bundle().unwrap();
	let output = String::from_utf8(bundle).unwrap();

	assert!(output.contains("style.css"));
	assert!(output.contains("app.js"));
}

#[tokio::test]
async fn test_full_processing_pipeline_with_compression() {
	let temp_dir = tempdir().unwrap();
	let output_dir = temp_dir.path().to_path_buf();

	// Create processing config with all features
	let config = ProcessingConfig::new(output_dir)
		.with_minification(true)
		.with_source_maps(true)
		.with_image_optimization(true);

	let pipeline = ProcessingPipeline::new(config);

	// Process CSS
	let css_input = b"/* comment */ body { color: red; }";
	let css_result = pipeline
		.process_file(css_input, &PathBuf::from("style.css"))
		.await
		.unwrap();

	assert!(!css_result.is_empty());

	// Process JS
	let js_input = b"// comment\nconst x = 1;";
	let js_result = pipeline
		.process_file(js_input, &PathBuf::from("app.js"))
		.await
		.unwrap();

	assert!(!js_result.is_empty());

	// Now compress results
	let gzip_compressor = GzipCompressor::new();
	let css_compressed = gzip_compressor
		.process(&css_result, &PathBuf::from("style.css.gz"))
		.await
		.unwrap();

	assert!(!css_compressed.is_empty());
}
