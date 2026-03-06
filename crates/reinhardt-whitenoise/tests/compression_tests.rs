//! Compression module tests

use reinhardt_whitenoise::compression::parse_accept_encoding;
use reinhardt_whitenoise::compression::{FileScanner, WhiteNoiseCompressor};
use reinhardt_whitenoise::config::WhiteNoiseConfig;
use rstest::rstest;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

// Scanner tests

#[rstest]
fn test_scanner_finds_compressible_files() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path();

	let css_path = static_root.join("app.css");
	let mut css_file = File::create(&css_path).unwrap();
	writeln!(css_file, "{}", "body { color: red; }".repeat(100)).unwrap();

	let js_path = static_root.join("app.js");
	let mut js_file = File::create(&js_path).unwrap();
	writeln!(js_file, "{}", "console.log('test');".repeat(100)).unwrap();

	let img_path = static_root.join("image.png");
	File::create(&img_path).unwrap();

	let config = WhiteNoiseConfig::new(static_root.to_path_buf(), "/static/".to_string());
	let scanner = FileScanner::new(config);

	let files = scanner.scan().unwrap();
	assert_eq!(files.len(), 2);
	assert!(files.iter().any(|p| p.ends_with("app.css")));
	assert!(files.iter().any(|p| p.ends_with("app.js")));
}

#[rstest]
fn test_scanner_respects_min_size() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path();

	let small_path = static_root.join("small.css");
	let mut small_file = File::create(&small_path).unwrap();
	writeln!(small_file, "small").unwrap();

	let large_path = static_root.join("large.css");
	let mut large_file = File::create(&large_path).unwrap();
	writeln!(large_file, "{}", "body { color: red; }".repeat(100)).unwrap();

	let config = WhiteNoiseConfig::new(static_root.to_path_buf(), "/static/".to_string());
	let scanner = FileScanner::new(config);

	let files = scanner.scan().unwrap();
	assert_eq!(files.len(), 1);
	assert!(files[0].ends_with("large.css"));
}

#[rstest]
fn test_scanner_recursive() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path();
	let css_dir = static_root.join("css");
	fs::create_dir(&css_dir).unwrap();

	let css_path = css_dir.join("style.css");
	let mut css_file = File::create(&css_path).unwrap();
	writeln!(css_file, "{}", "body { color: red; }".repeat(100)).unwrap();

	let config = WhiteNoiseConfig::new(static_root.to_path_buf(), "/static/".to_string());
	let scanner = FileScanner::new(config);

	let files = scanner.scan().unwrap();
	assert_eq!(files.len(), 1);
	assert!(files[0].to_str().unwrap().contains("css/style.css"));
}

#[rstest]
fn test_scanner_exclude_extensions() {
	let temp_dir = TempDir::new().unwrap();
	let static_root = temp_dir.path();

	let gz_path = static_root.join("app.js.gz");
	let mut gz_file = File::create(&gz_path).unwrap();
	writeln!(gz_file, "{}", "compressed".repeat(100)).unwrap();

	let config = WhiteNoiseConfig::new(static_root.to_path_buf(), "/static/".to_string());
	let scanner = FileScanner::new(config);

	let files = scanner.scan().unwrap();
	assert_eq!(files.len(), 0);
}

#[rstest]
fn test_scanner_empty_directory() {
	let temp_dir = TempDir::new().unwrap();
	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string());
	let scanner = FileScanner::new(config);

	let files = scanner.scan().unwrap();
	assert_eq!(files.len(), 0);
}

// Compressor tests

#[rstest]
#[tokio::test]
async fn test_compressor_gzip() {
	let temp_dir = TempDir::new().unwrap();
	let test_file = temp_dir.path().join("test.txt");
	let content = "Hello, world! ".repeat(100);
	fs::write(&test_file, &content).unwrap();

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(true, false);
	let compressor = WhiteNoiseCompressor::new(config);

	let variants = compressor.compress(test_file.clone()).await.unwrap();

	assert!(variants.gzip.is_some());
	let gz_path = variants.gzip.unwrap();
	assert!(gz_path.exists());
	assert!(gz_path.to_str().unwrap().ends_with(".txt.gz"));

	let original_size = fs::metadata(&test_file).unwrap().len();
	let compressed_size = fs::metadata(&gz_path).unwrap().len();
	assert!(compressed_size < original_size);
}

#[rstest]
#[tokio::test]
async fn test_compressor_brotli() {
	let temp_dir = TempDir::new().unwrap();
	let test_file = temp_dir.path().join("test.txt");
	let content = "Hello, world! ".repeat(100);
	fs::write(&test_file, &content).unwrap();

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(false, true);
	let compressor = WhiteNoiseCompressor::new(config);

	let variants = compressor.compress(test_file.clone()).await.unwrap();

	assert!(variants.brotli.is_some());
	let br_path = variants.brotli.unwrap();
	assert!(br_path.exists());
	assert!(br_path.to_str().unwrap().ends_with(".txt.br"));

	let original_size = fs::metadata(&test_file).unwrap().len();
	let compressed_size = fs::metadata(&br_path).unwrap().len();
	assert!(compressed_size < original_size);
}

#[rstest]
#[tokio::test]
async fn test_compressor_both() {
	let temp_dir = TempDir::new().unwrap();
	let test_file = temp_dir.path().join("test.css");
	let content = "body { color: red; } ".repeat(100);
	fs::write(&test_file, &content).unwrap();

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(true, true);
	let compressor = WhiteNoiseCompressor::new(config);

	let variants = compressor.compress(test_file.clone()).await.unwrap();

	assert!(variants.gzip.is_some());
	assert!(variants.brotli.is_some());
	assert!(variants.has_variants());
}

#[rstest]
#[tokio::test]
async fn test_compressor_batch() {
	let temp_dir = TempDir::new().unwrap();
	let files: Vec<std::path::PathBuf> = (0..3)
		.map(|i| {
			let path = temp_dir.path().join(format!("test{}.txt", i));
			fs::write(&path, "Hello, world! ".repeat(100)).unwrap();
			path
		})
		.collect();

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(true, true);
	let compressor = WhiteNoiseCompressor::new(config);

	let results = compressor.compress_batch(files.clone()).await.unwrap();

	assert_eq!(results.len(), 3);
	for (original, variants) in results {
		assert!(files.contains(&original));
		assert!(variants.has_variants());
	}
}

#[rstest]
#[tokio::test]
async fn test_compressor_size_reduction() {
	let temp_dir = TempDir::new().unwrap();
	let test_file = temp_dir.path().join("test.txt");
	let content = "Hello, world! ".repeat(1000);
	fs::write(&test_file, &content).unwrap();

	let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
		.with_compression(true, true);
	let compressor = WhiteNoiseCompressor::new(config);

	let variants = compressor.compress(test_file.clone()).await.unwrap();

	let original_size = fs::metadata(&test_file).unwrap().len();

	if let Some(gz_path) = &variants.gzip {
		let gz_size = fs::metadata(gz_path).unwrap().len();
		assert!(gz_size < original_size, "Gzip should reduce size");
	}

	if let Some(br_path) = &variants.brotli {
		let br_size = fs::metadata(br_path).unwrap().len();
		assert!(br_size < original_size, "Brotli should reduce size");
	}
}

// Content negotiation tests

#[rstest]
#[case("br", true, false)]
#[case("gzip", false, true)]
#[case("br, gzip", true, true)]
#[case("gzip, br", true, true)]
#[case("identity", false, false)]
#[case("GZIP", false, true)] // Case insensitive
#[case("BR", true, false)] // Case insensitive
fn test_parse_accept_encoding(
	#[case] header: &str,
	#[case] expect_br: bool,
	#[case] expect_gzip: bool,
) {
	let (supports_br, supports_gzip) = parse_accept_encoding(header);
	assert_eq!(supports_br, expect_br);
	assert_eq!(supports_gzip, expect_gzip);
}

#[rstest]
#[case("")]
#[case("   ")]
fn test_parse_accept_encoding_malformed(#[case] header: &str) {
	let (supports_br, supports_gzip) = parse_accept_encoding(header);
	assert!(!supports_br);
	assert!(!supports_gzip);
}
