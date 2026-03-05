//! Image optimization for static assets
//!
//! Provides optimization for PNG, JPEG, and WebP images to reduce file size
//! while maintaining visual quality.

use super::{ProcessingResult, Processor};
use async_trait::async_trait;
use std::io;
use std::path::Path;

/// Image optimizer
///
/// Optimizes images to reduce file size while maintaining quality.
pub struct ImageOptimizer {
	/// Quality level (1-100)
	// Reserved for future image optimization implementation
	#[allow(dead_code)]
	quality: u8,
	/// Enable lossy compression
	// Reserved for future image optimization implementation
	#[allow(dead_code)]
	lossy: bool,
}

impl ImageOptimizer {
	/// Create a new image optimizer with default settings
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::images::ImageOptimizer;
	///
	/// let optimizer = ImageOptimizer::new(85);
	/// ```
	pub fn new(quality: u8) -> Self {
		Self {
			quality: quality.clamp(1, 100),
			lossy: true,
		}
	}

	/// Create an optimizer with custom settings
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::images::ImageOptimizer;
	///
	/// let optimizer = ImageOptimizer::with_settings(90, false);
	/// ```
	pub fn with_settings(quality: u8, lossy: bool) -> Self {
		Self {
			quality: quality.clamp(1, 100),
			lossy,
		}
	}

	/// Optimize PNG image
	fn optimize_png(&self, input: &[u8]) -> ProcessingResult<Vec<u8>> {
		#[cfg(feature = "image-optimization")]
		{
			use oxipng::{Options, optimize_from_memory};

			// Create optimization options based on quality setting
			// Quality 1-100 maps to oxipng compression level 0-6
			let level = ((self.quality as f32 / 100.0) * 6.0) as u8;
			let level = level.clamp(0, 6);

			let options = Options::from_preset(level);

			optimize_from_memory(input, &options)
				.map_err(|e| io::Error::other(format!("PNG optimization failed: {}", e)))
		}

		#[cfg(not(feature = "image-optimization"))]
		{
			// Without image-optimization feature, return input unchanged
			Ok(input.to_vec())
		}
	}

	/// Optimize JPEG image
	fn optimize_jpeg(&self, input: &[u8]) -> ProcessingResult<Vec<u8>> {
		#[cfg(feature = "image-optimization")]
		{
			use image::ImageReader;
			use image::codecs::jpeg::JpegEncoder;
			use std::io::Cursor;

			// Decode the JPEG image
			let img = ImageReader::new(Cursor::new(input))
				.with_guessed_format()
				.map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Failed to read JPEG: {}", e),
					)
				})?
				.decode()
				.map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Failed to decode JPEG: {}", e),
					)
				})?;

			// Re-encode with specified quality
			let mut output = Vec::new();
			let mut encoder = JpegEncoder::new_with_quality(&mut output, self.quality);
			encoder
				.encode(
					img.as_bytes(),
					img.width(),
					img.height(),
					img.color().into(),
				)
				.map_err(|e| io::Error::other(format!("JPEG encoding failed: {}", e)))?;

			Ok(output)
		}

		#[cfg(not(feature = "image-optimization"))]
		{
			// Without image-optimization feature, return input unchanged
			Ok(input.to_vec())
		}
	}

	/// Optimize WebP image
	fn optimize_webp(&self, input: &[u8]) -> ProcessingResult<Vec<u8>> {
		#[cfg(feature = "image-optimization")]
		{
			use image::ImageReader;
			use std::io::Cursor;

			// Decode the image
			let img = ImageReader::new(Cursor::new(input))
				.with_guessed_format()
				.map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Failed to read WebP: {}", e),
					)
				})?
				.decode()
				.map_err(|e| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Failed to decode WebP: {}", e),
					)
				})?;

			// Encode to WebP
			let encoder = webp::Encoder::from_image(&img)
				.map_err(|e| io::Error::other(format!("Failed to create WebP encoder: {}", e)))?;

			let webp_data = if self.lossy {
				// Lossy compression with quality setting
				encoder.encode(self.quality as f32)
			} else {
				// Lossless compression
				encoder.encode_lossless()
			};

			Ok(webp_data.to_vec())
		}

		#[cfg(not(feature = "image-optimization"))]
		{
			// Without image-optimization feature, return input unchanged
			Ok(input.to_vec())
		}
	}

	/// Detect image format from file extension
	fn detect_format(&self, path: &Path) -> Option<ImageFormat> {
		path.extension()
			.and_then(|ext| ext.to_str())
			.and_then(|ext| match ext.to_lowercase().as_str() {
				"png" => Some(ImageFormat::Png),
				"jpg" | "jpeg" => Some(ImageFormat::Jpeg),
				"webp" => Some(ImageFormat::WebP),
				_ => None,
			})
	}
}

impl Default for ImageOptimizer {
	fn default() -> Self {
		Self::new(85)
	}
}

#[async_trait]
impl Processor for ImageOptimizer {
	async fn process(&self, input: &[u8], path: &Path) -> ProcessingResult<Vec<u8>> {
		match self.detect_format(path) {
			Some(ImageFormat::Png) => self.optimize_png(input),
			Some(ImageFormat::Jpeg) => self.optimize_jpeg(input),
			Some(ImageFormat::WebP) => self.optimize_webp(input),
			None => Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"Unsupported image format",
			)),
		}
	}

	fn can_process(&self, path: &Path) -> bool {
		self.detect_format(path).is_some()
	}

	fn name(&self) -> &str {
		"ImageOptimizer"
	}
}

/// Image format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageFormat {
	Png,
	Jpeg,
	WebP,
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn test_optimizer_creation() {
		let optimizer = ImageOptimizer::new(85);
		assert_eq!(optimizer.quality, 85);
		assert!(optimizer.lossy);
	}

	#[test]
	fn test_optimizer_with_settings() {
		let optimizer = ImageOptimizer::with_settings(90, false);
		assert_eq!(optimizer.quality, 90);
		assert!(!optimizer.lossy);
	}

	#[test]
	fn test_quality_clamping() {
		let optimizer1 = ImageOptimizer::new(150);
		assert_eq!(optimizer1.quality, 100);

		let optimizer2 = ImageOptimizer::new(0);
		assert_eq!(optimizer2.quality, 1);
	}

	#[test]
	fn test_can_process_png() {
		let optimizer = ImageOptimizer::new(85);
		assert!(optimizer.can_process(&PathBuf::from("image.png")));
		assert!(optimizer.can_process(&PathBuf::from("image.PNG")));
	}

	#[test]
	fn test_can_process_jpeg() {
		let optimizer = ImageOptimizer::new(85);
		assert!(optimizer.can_process(&PathBuf::from("photo.jpg")));
		assert!(optimizer.can_process(&PathBuf::from("photo.jpeg")));
		assert!(optimizer.can_process(&PathBuf::from("photo.JPEG")));
	}

	#[test]
	fn test_can_process_webp() {
		let optimizer = ImageOptimizer::new(85);
		assert!(optimizer.can_process(&PathBuf::from("image.webp")));
		assert!(optimizer.can_process(&PathBuf::from("image.WEBP")));
	}

	#[test]
	fn test_cannot_process_other_formats() {
		let optimizer = ImageOptimizer::new(85);
		assert!(!optimizer.can_process(&PathBuf::from("style.css")));
		assert!(!optimizer.can_process(&PathBuf::from("script.js")));
		assert!(!optimizer.can_process(&PathBuf::from("image.gif")));
	}

	#[tokio::test]
	#[cfg(not(feature = "image-optimization"))]
	async fn test_optimize_png_basic_without_feature() {
		// Without image-optimization feature, input should be returned unchanged
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake png data";
		let result = optimizer
			.process(input, &PathBuf::from("test.png"))
			.await
			.unwrap();
		assert_eq!(result, input);
	}

	#[tokio::test]
	#[cfg(feature = "image-optimization")]
	async fn test_optimize_png_basic_with_feature() {
		// With image-optimization feature, invalid PNG data should return error
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake png data";
		let result = optimizer.process(input, &PathBuf::from("test.png")).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	#[cfg(not(feature = "image-optimization"))]
	async fn test_optimize_jpeg_basic_without_feature() {
		// Without image-optimization feature, input should be returned unchanged
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake jpeg data";
		let result = optimizer
			.process(input, &PathBuf::from("test.jpg"))
			.await
			.unwrap();
		assert_eq!(result, input);
	}

	#[tokio::test]
	#[cfg(feature = "image-optimization")]
	async fn test_optimize_jpeg_basic_with_feature() {
		// With image-optimization feature, invalid JPEG data should return error
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake jpeg data";
		let result = optimizer.process(input, &PathBuf::from("test.jpg")).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	#[cfg(not(feature = "image-optimization"))]
	async fn test_optimize_webp_basic_without_feature() {
		// Without image-optimization feature, input should be returned unchanged
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake webp data";
		let result = optimizer
			.process(input, &PathBuf::from("test.webp"))
			.await
			.unwrap();
		assert_eq!(result, input);
	}

	#[tokio::test]
	#[cfg(feature = "image-optimization")]
	async fn test_optimize_webp_basic_with_feature() {
		// With image-optimization feature, invalid WebP data should return error
		let optimizer = ImageOptimizer::new(85);
		let input = b"fake webp data";
		let result = optimizer.process(input, &PathBuf::from("test.webp")).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_unsupported_format() {
		let optimizer = ImageOptimizer::new(85);
		let input = b"data";
		let result = optimizer.process(input, &PathBuf::from("test.gif")).await;
		assert!(result.is_err());
	}

	#[test]
	fn test_default() {
		let optimizer = ImageOptimizer::default();
		assert_eq!(optimizer.quality, 85);
		assert!(optimizer.lossy);
	}
}
