//! File scanner for pre-compression at startup

use crate::config::WhiteNoiseConfig;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// File scanner for WhiteNoise
pub struct FileScanner {
	/// Configuration for scanning behavior
	config: WhiteNoiseConfig,
}

impl FileScanner {
	/// Creates a new file scanner
	///
	/// # Arguments
	///
	/// * `config` - WhiteNoise configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::{WhiteNoiseConfig, compression::FileScanner};
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(
	///     PathBuf::from("static"),
	///     "/static/".to_string(),
	/// );
	/// let scanner = FileScanner::new(config);
	/// ```
	pub fn new(config: WhiteNoiseConfig) -> Self {
		Self { config }
	}

	/// Scans the static root directory for compressible files
	///
	/// Returns a list of files that should be compressed based on:
	/// - File size (must be >= min_compress_size)
	/// - File extension (must be in compress_extensions)
	/// - Not in exclude_extensions
	///
	/// # Returns
	///
	/// Vector of absolute paths to files that should be compressed
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_whitenoise::{WhiteNoiseConfig, compression::FileScanner};
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(
	///     PathBuf::from("static"),
	///     "/static/".to_string(),
	/// );
	/// let scanner = FileScanner::new(config);
	/// let files = scanner.scan()?;
	/// ```
	pub fn scan(&self) -> crate::Result<Vec<PathBuf>> {
		let mut files = Vec::new();

		let walker = WalkDir::new(&self.config.root).follow_links(self.config.follow_symlinks);

		for entry in walker {
			let entry = entry?;

			// Skip directories
			if !entry.file_type().is_file() {
				continue;
			}

			let path = entry.path();

			// Check if file should be compressed
			if self.should_compress(path)? {
				files.push(path.to_path_buf());
			}
		}

		Ok(files)
	}

	/// Determines if a file should be compressed
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	///
	/// # Returns
	///
	/// `true` if the file should be compressed
	fn should_compress(&self, path: &Path) -> crate::Result<bool> {
		// Get file extension
		let extension = match path.extension() {
			Some(ext) => ext.to_string_lossy().to_lowercase(),
			None => return Ok(false), // No extension, skip
		};

		// Check if extension is excluded
		if self.config.exclude_extensions.contains(&extension) {
			return Ok(false);
		}

		// Check if extension is in compress list
		if !self.config.compress_extensions.contains(&extension) {
			return Ok(false);
		}

		// Check file size
		let metadata = std::fs::metadata(path)?;
		if metadata.len() < self.config.min_compress_size as u64 {
			return Ok(false);
		}

		Ok(true)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs::{self, File};
	use std::io::Write;
	use tempfile::TempDir;

	#[rstest]
	fn test_scanner_finds_compressible_files() {
		let temp_dir = TempDir::new().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Create test files
		let css_path = static_root.join("app.css");
		let mut css_file = File::create(&css_path).unwrap();
		// Write enough data to exceed min_compress_size (1024 bytes)
		writeln!(css_file, "{}", "body {{ color: red; }}".repeat(100)).unwrap();

		let js_path = static_root.join("app.js");
		let mut js_file = File::create(&js_path).unwrap();
		writeln!(js_file, "{}", "console.log('test');".repeat(100)).unwrap();

		// Create a file that should be excluded (image)
		let img_path = static_root.join("image.png");
		File::create(&img_path).unwrap();

		let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string());
		let scanner = FileScanner::new(config);

		let files = scanner.scan().unwrap();

		// Should find CSS and JS but not PNG
		assert_eq!(files.len(), 2);
		assert!(files.iter().any(|p| p.ends_with("app.css")));
		assert!(files.iter().any(|p| p.ends_with("app.js")));
		assert!(!files.iter().any(|p| p.ends_with("image.png")));
	}

	#[rstest]
	fn test_scanner_respects_min_size() {
		let temp_dir = TempDir::new().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Create a small file (below min_compress_size)
		let small_path = static_root.join("small.css");
		let mut small_file = File::create(&small_path).unwrap();
		writeln!(small_file, "small").unwrap();

		// Create a large file (above min_compress_size)
		let large_path = static_root.join("large.css");
		let mut large_file = File::create(&large_path).unwrap();
		writeln!(large_file, "{}", "body {{ color: red; }}".repeat(100)).unwrap();

		let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string());
		let scanner = FileScanner::new(config);

		let files = scanner.scan().unwrap();

		// Should only find the large file
		assert_eq!(files.len(), 1);
		assert!(files.iter().any(|p| p.ends_with("large.css")));
		assert!(!files.iter().any(|p| p.ends_with("small.css")));
	}

	#[rstest]
	fn test_scanner_recursive() {
		let temp_dir = TempDir::new().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Create subdirectory
		let subdir = static_root.join("css");
		fs::create_dir(&subdir).unwrap();

		// Create file in subdirectory
		let css_path = subdir.join("style.css");
		let mut css_file = File::create(&css_path).unwrap();
		writeln!(css_file, "{}", "body {{ color: red; }}".repeat(100)).unwrap();

		let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string());
		let scanner = FileScanner::new(config);

		let files = scanner.scan().unwrap();

		// Should find file in subdirectory
		assert_eq!(files.len(), 1);
		assert!(files.iter().any(|p| p.ends_with("css/style.css")));
	}

	#[rstest]
	fn test_scanner_exclude_extensions() {
		let temp_dir = TempDir::new().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Create already compressed file
		let gz_path = static_root.join("app.js.gz");
		let mut gz_file = File::create(&gz_path).unwrap();
		writeln!(gz_file, "{}", "compressed".repeat(100)).unwrap();

		let config = WhiteNoiseConfig::new(static_root.clone(), "/static/".to_string());
		let scanner = FileScanner::new(config);

		let files = scanner.scan().unwrap();

		// Should not find .gz files (in exclude_extensions by default)
		assert_eq!(files.len(), 0);
	}
}
