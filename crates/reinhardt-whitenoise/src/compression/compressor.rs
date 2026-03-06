//! File compression utilities

#[cfg(feature = "compression")]
use brotli::enc::BrotliEncoderParams;
#[cfg(feature = "compression")]
use flate2::Compression;
#[cfg(feature = "compression")]
use flate2::write::GzEncoder;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::cache::CompressedVariants;
use crate::config::WhiteNoiseConfig;

/// WhiteNoise file compressor
pub struct WhiteNoiseCompressor {
	/// Configuration for compression behavior
	config: WhiteNoiseConfig,
}

impl WhiteNoiseCompressor {
	/// Creates a new compressor
	///
	/// # Arguments
	///
	/// * `config` - WhiteNoise configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::{WhiteNoiseConfig, compression::WhiteNoiseCompressor};
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(
	///     PathBuf::from("static"),
	///     "/static/".to_string(),
	/// );
	/// let compressor = WhiteNoiseCompressor::new(config);
	/// ```
	pub fn new(config: WhiteNoiseConfig) -> Self {
		Self { config }
	}

	/// Compresses a file with both gzip and brotli (if enabled)
	///
	/// # Arguments
	///
	/// * `path` - Path to the file to compress
	///
	/// # Returns
	///
	/// [`CompressedVariants`] containing paths to compressed files
	///
	/// # Errors
	///
	/// Returns error if compression fails
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_whitenoise::{WhiteNoiseConfig, compression::WhiteNoiseCompressor};
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(
	///     PathBuf::from("static"),
	///     "/static/".to_string(),
	/// );
	/// let compressor = WhiteNoiseCompressor::new(config);
	///
	/// let variants = compressor.compress(PathBuf::from("app.js")).await?;
	/// ```
	pub async fn compress(&self, path: PathBuf) -> crate::Result<CompressedVariants> {
		let mut variants = CompressedVariants::new();

		// Read file content
		let content = fs::read(&path).await?;

		// Compress with gzip if enabled
		#[cfg(feature = "compression")]
		if self.config.enable_gzip {
			let gzip_path = self.compress_gzip(&path, &content).await?;
			variants = variants.with_gzip(gzip_path);
		}

		// Compress with brotli if enabled
		#[cfg(feature = "compression")]
		if self.config.enable_brotli {
			let brotli_path = self.compress_brotli(&path, &content).await?;
			variants = variants.with_brotli(brotli_path);
		}

		Ok(variants)
	}

	/// Compresses a file using gzip
	///
	/// # Arguments
	///
	/// * `path` - Original file path
	/// * `content` - File content to compress
	///
	/// # Returns
	///
	/// Path to the compressed .gz file
	#[cfg(feature = "compression")]
	async fn compress_gzip(&self, path: &Path, content: &[u8]) -> crate::Result<PathBuf> {
		let gz_path = path.with_extension(format!(
			"{}.gz",
			path.extension()
				.map(|e| e.to_string_lossy())
				.unwrap_or_default()
		));

		// Compress with gzip
		let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.config.gzip_level));
		encoder.write_all(content)?;
		let compressed = encoder.finish()?;

		// Write compressed file
		let mut file = fs::File::create(&gz_path).await?;
		file.write_all(&compressed).await?;
		file.flush().await?;

		Ok(gz_path)
	}

	/// Compresses a file using brotli
	///
	/// # Arguments
	///
	/// * `path` - Original file path
	/// * `content` - File content to compress
	///
	/// # Returns
	///
	/// Path to the compressed .br file
	#[cfg(feature = "compression")]
	async fn compress_brotli(&self, path: &Path, content: &[u8]) -> crate::Result<PathBuf> {
		let br_path = path.with_extension(format!(
			"{}.br",
			path.extension()
				.map(|e| e.to_string_lossy())
				.unwrap_or_default()
		));

		// Compress with brotli
		let mut compressed = Vec::new();
		let params = BrotliEncoderParams {
			quality: self.config.brotli_quality as i32,
			..Default::default()
		};

		brotli::BrotliCompress(&mut std::io::Cursor::new(content), &mut compressed, &params)?;

		// Write compressed file
		let mut file = fs::File::create(&br_path).await?;
		file.write_all(&compressed).await?;
		file.flush().await?;

		Ok(br_path)
	}

	/// Compresses multiple files in parallel
	///
	/// # Arguments
	///
	/// * `paths` - Paths to files to compress
	///
	/// # Returns
	///
	/// Vector of tuples (original_path, compressed_variants)
	///
	/// # Errors
	///
	/// Returns error if any compression fails
	pub async fn compress_batch(
		&self,
		paths: Vec<PathBuf>,
	) -> crate::Result<Vec<(PathBuf, CompressedVariants)>> {
		let mut results = Vec::new();

		for path in paths {
			let variants = self.compress(path.clone()).await?;
			results.push((path, variants));
		}

		Ok(results)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use tempfile::TempDir;

	#[rstest]
	#[tokio::test]
	async fn test_compressor_gzip() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");

		// Create test file with compressible content
		let content = "Hello, world! ".repeat(100);
		std::fs::write(&test_file, &content).unwrap();

		let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
			.with_compression(true, false);
		let compressor = WhiteNoiseCompressor::new(config);

		let variants = compressor.compress(test_file.clone()).await.unwrap();

		// Check that gzip variant exists
		assert!(variants.gzip.is_some());
		let gz_path = variants.gzip.unwrap();
		assert!(gz_path.exists());
		assert!(gz_path.to_string_lossy().ends_with(".txt.gz"));

		// Verify compressed file is smaller than original
		let original_size = std::fs::metadata(&test_file).unwrap().len();
		let compressed_size = std::fs::metadata(&gz_path).unwrap().len();
		assert!(compressed_size < original_size);
	}

	#[rstest]
	#[tokio::test]
	async fn test_compressor_brotli() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");

		// Create test file with compressible content
		let content = "Hello, world! ".repeat(100);
		std::fs::write(&test_file, &content).unwrap();

		let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
			.with_compression(false, true);
		let compressor = WhiteNoiseCompressor::new(config);

		let variants = compressor.compress(test_file.clone()).await.unwrap();

		// Check that brotli variant exists
		assert!(variants.brotli.is_some());
		let br_path = variants.brotli.unwrap();
		assert!(br_path.exists());
		assert!(br_path.to_string_lossy().ends_with(".txt.br"));

		// Verify compressed file is smaller than original
		let original_size = std::fs::metadata(&test_file).unwrap().len();
		let compressed_size = std::fs::metadata(&br_path).unwrap().len();
		assert!(compressed_size < original_size);
	}

	#[rstest]
	#[tokio::test]
	async fn test_compressor_both() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.css");

		// Create test file with compressible content
		let content = "body { color: red; } ".repeat(100);
		std::fs::write(&test_file, &content).unwrap();

		let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
			.with_compression(true, true);
		let compressor = WhiteNoiseCompressor::new(config);

		let variants = compressor.compress(test_file.clone()).await.unwrap();

		// Check that both variants exist
		assert!(variants.gzip.is_some());
		assert!(variants.brotli.is_some());
		assert!(variants.has_variants());

		// Verify both compressed files exist
		let gz_path = variants.gzip.unwrap();
		let br_path = variants.brotli.unwrap();
		assert!(gz_path.exists());
		assert!(br_path.exists());
	}

	#[rstest]
	#[tokio::test]
	async fn test_compressor_batch() {
		let temp_dir = TempDir::new().unwrap();

		// Create multiple test files
		let files: Vec<PathBuf> = (0..3)
			.map(|i| {
				let path = temp_dir.path().join(format!("test{}.txt", i));
				std::fs::write(&path, "Hello, world! ".repeat(100)).unwrap();
				path
			})
			.collect();

		let config = WhiteNoiseConfig::new(temp_dir.path().to_path_buf(), "/static/".to_string())
			.with_compression(true, true);
		let compressor = WhiteNoiseCompressor::new(config);

		let results = compressor.compress_batch(files.clone()).await.unwrap();

		// Check all files were compressed
		assert_eq!(results.len(), 3);
		for (original, variants) in results {
			assert!(files.contains(&original));
			assert!(variants.has_variants());
		}
	}
}
