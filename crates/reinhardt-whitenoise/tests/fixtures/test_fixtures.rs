//! Specialized test fixtures for reinhardt-whitenoise
//!
//! This module provides fixtures that wrap generic reinhardt-test fixtures
//! with whitenoise-specific test data.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir as TempDirType;

/// Wrapper for tempfile TempDir to match reinhardt-test fixture pattern
pub struct TempDir {
	inner: TempDirType,
}

impl TempDir {
	fn new(temp_dir: TempDirType) -> Self {
		Self { inner: temp_dir }
	}

	pub fn path(&self) -> &std::path::Path {
		self.inner.path()
	}
}

/// Creates a temporary static file directory with sample files
pub fn static_dir() -> TempDir {
	let temp_dir = TempDirType::new().unwrap();
	let static_path = temp_dir.path().join("static");
	fs::create_dir(&static_path).unwrap();

	// Create CSS file
	let css_path = static_path.join("app.css");
	let mut css_file = File::create(&css_path).unwrap();
	writeln!(css_file, "{}", "body { color: red; }".repeat(100)).unwrap();

	// Create JS file
	let js_path = static_path.join("app.js");
	let mut js_file = File::create(&js_path).unwrap();
	writeln!(js_file, "{}", "console.log('test');".repeat(100)).unwrap();

	// Create hashed (immutable) file
	let hashed_css = static_path.join("app.abc123def456.css");
	fs::copy(&css_path, &hashed_css).unwrap();

	// Create small file (below compression threshold)
	let small_path = static_path.join("small.txt");
	fs::write(&small_path, "small content").unwrap();

	TempDir::new(temp_dir)
}

/// Creates a large file for compression testing
pub fn large_file(size: usize) -> (TempDir, PathBuf) {
	let temp_dir = TempDirType::new().unwrap();
	let file_path = temp_dir.path().join("large.txt");
	let content = "x".repeat(size);
	fs::write(&file_path, content).unwrap();
	(TempDir::new(temp_dir), file_path)
}

/// Creates a directory with manifest.json
pub fn manifest_dir() -> TempDir {
	let temp_dir = static_dir();
	let manifest_path = temp_dir.path().join("static/manifest.json");
	let manifest = r#"{
		"app.js": "app.abc123def456.js",
		"app.css": "app.1234567890ab.css"
	}"#;
	fs::write(&manifest_path, manifest).unwrap();
	temp_dir
}

/// Creates a nested directory structure
pub fn nested_dir() -> TempDir {
	let temp_dir = static_dir();
	let css_dir = temp_dir.path().join("static/css");
	fs::create_dir(&css_dir).unwrap();

	let style_path = css_dir.join("style.css");
	let mut style_file = File::create(&style_path).unwrap();
	writeln!(style_file, "{}", ".style { color: blue; }".repeat(100)).unwrap();

	temp_dir
}

/// Creates files with various extensions
pub fn mixed_extensions_dir() -> TempDir {
	let temp_dir = static_dir();

	// Add image files (should be excluded from compression)
	let png_path = temp_dir.path().join("static/image.png");
	fs::write(&png_path, b"\x89PNG\r\n\x1a\n").unwrap();

	let jpg_path = temp_dir.path().join("static/photo.jpg");
	fs::write(&jpg_path, b"\xff\xd8\xff\xe0").unwrap();

	// Add already compressed file
	let gz_path = temp_dir.path().join("static/data.json.gz");
	fs::write(&gz_path, b"compressed data").unwrap();

	temp_dir
}
