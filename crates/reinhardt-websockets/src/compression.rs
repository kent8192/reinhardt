//! WebSocket message compression support
//!
//! This module provides compression and decompression for WebSocket messages.
//! It supports three compression algorithms: gzip, deflate, and brotli.
//!
//! ## Usage Example
//!
//! ```
//! use reinhardt_websockets::compression::{CompressionCodec, compress_message, decompress_message};
//! use reinhardt_websockets::Message;
//!
//! # tokio_test::block_on(async {
//! let message = Message::text("Hello, World!".to_string());
//!
//! // Gzip compression
//! let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
//! let decompressed = decompress_message(&compressed, CompressionCodec::Gzip).unwrap();
//!
//! // Decompressed message is always returned as Binary
//! match decompressed {
//!     Message::Binary { data } => {
//!         assert_eq!(String::from_utf8(data).unwrap(), "Hello, World!");
//!     },
//!     _ => panic!("Expected binary message"),
//! }
//! # });
//! ```

use crate::{Message, WebSocketError, WebSocketResult};

/// Default maximum decompressed message size: 10 MB
const DEFAULT_MAX_DECOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

/// Default maximum window bits for deflate-based compression
const DEFAULT_MAX_WINDOW_BITS: u8 = 15;

/// Configuration for WebSocket per-message compression negotiation.
///
/// Controls compression behavior including whether compression is enabled,
/// maximum window bits for deflate-based algorithms, and a hard limit
/// on decompressed message size to prevent decompression bomb attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::compression::CompressionConfig;
///
/// // Use defaults
/// let config = CompressionConfig::default();
/// assert!(config.enabled());
/// assert_eq!(config.max_window_bits(), 15);
/// assert_eq!(config.max_decompressed_size(), 10 * 1024 * 1024);
///
/// // Custom configuration
/// let config = CompressionConfig::new()
///     .with_enabled(false)
///     .with_max_window_bits(12)
///     .with_max_decompressed_size(5 * 1024 * 1024);
/// assert!(!config.enabled());
/// assert_eq!(config.max_window_bits(), 12);
/// assert_eq!(config.max_decompressed_size(), 5 * 1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct CompressionConfig {
	/// Whether compression is enabled for the connection
	enabled: bool,
	/// Maximum window bits for deflate-based compression (8-15)
	max_window_bits: u8,
	/// Maximum allowed size of decompressed data in bytes
	max_decompressed_size: usize,
}

impl Default for CompressionConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			max_window_bits: DEFAULT_MAX_WINDOW_BITS,
			max_decompressed_size: DEFAULT_MAX_DECOMPRESSED_SIZE,
		}
	}
}

impl CompressionConfig {
	/// Creates a new compression configuration with default values.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns whether compression is enabled.
	pub fn enabled(&self) -> bool {
		self.enabled
	}

	/// Returns the maximum window bits setting.
	pub fn max_window_bits(&self) -> u8 {
		self.max_window_bits
	}

	/// Returns the maximum decompressed message size in bytes.
	pub fn max_decompressed_size(&self) -> usize {
		self.max_decompressed_size
	}

	/// Sets whether compression is enabled.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::compression::CompressionConfig;
	///
	/// let config = CompressionConfig::new().with_enabled(false);
	/// assert!(!config.enabled());
	/// ```
	pub fn with_enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Sets the maximum window bits for deflate-based compression.
	///
	/// Valid values are 8-15. Values outside this range are clamped.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::compression::CompressionConfig;
	///
	/// let config = CompressionConfig::new().with_max_window_bits(12);
	/// assert_eq!(config.max_window_bits(), 12);
	/// ```
	pub fn with_max_window_bits(mut self, bits: u8) -> Self {
		self.max_window_bits = bits.clamp(8, 15);
		self
	}

	/// Sets the maximum decompressed message size in bytes.
	///
	/// This limit prevents decompression bomb attacks where a small
	/// compressed payload expands to an extremely large decompressed size.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::compression::CompressionConfig;
	///
	/// let config = CompressionConfig::new()
	///     .with_max_decompressed_size(5 * 1024 * 1024);
	/// assert_eq!(config.max_decompressed_size(), 5 * 1024 * 1024);
	/// ```
	pub fn with_max_decompressed_size(mut self, size: usize) -> Self {
		self.max_decompressed_size = size;
		self
	}
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompressionCodec {
	/// Gzip compression
	Gzip,
	/// Deflate compression
	Deflate,
	/// Brotli compression
	Brotli,
}

impl CompressionCodec {
	/// Converts a compression codec to a string.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::compression::CompressionCodec;
	///
	/// assert_eq!(CompressionCodec::Gzip.as_str(), "gzip");
	/// assert_eq!(CompressionCodec::Deflate.as_str(), "deflate");
	/// assert_eq!(CompressionCodec::Brotli.as_str(), "br");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Gzip => "gzip",
			Self::Deflate => "deflate",
			Self::Brotli => "br",
		}
	}
}

impl std::str::FromStr for CompressionCodec {
	type Err = WebSocketError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"gzip" => Ok(Self::Gzip),
			"deflate" => Ok(Self::Deflate),
			"br" | "brotli" => Ok(Self::Brotli),
			_ => Err(WebSocketError::Protocol(
				"unsupported compression codec".to_string(),
			)),
		}
	}
}

/// Compresses a message.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::compression::{CompressionCodec, compress_message};
/// use reinhardt_websockets::Message;
///
/// let message = Message::text("Hello, World!".to_string());
/// let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
///
/// match compressed {
///     Message::Binary { data } => {
///         assert!(data.len() > 0);
///         // Compressed data may be smaller than the original text
///     },
///     _ => panic!("Expected binary message"),
/// }
/// ```
#[cfg(feature = "compression")]
pub fn compress_message(message: &Message, codec: CompressionCodec) -> WebSocketResult<Message> {
	let data = match message {
		Message::Text { data } => data.as_bytes(),
		Message::Binary { data } => data.as_slice(),
		_ => {
			return Err(WebSocketError::Protocol(
				"Cannot compress non-data messages".to_string(),
			));
		}
	};

	let compressed = match codec {
		CompressionCodec::Gzip => compress_gzip(data)?,
		CompressionCodec::Deflate => compress_deflate(data)?,
		CompressionCodec::Brotli => compress_brotli(data)?,
	};

	Ok(Message::Binary { data: compressed })
}

/// Decompresses a compressed message.
///
/// Always returns a binary message, as the original message type
/// information is not preserved during compression.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::compression::{CompressionCodec, compress_message, decompress_message};
/// use reinhardt_websockets::Message;
///
/// let original = Message::text("Hello, World!".to_string());
/// let compressed = compress_message(&original, CompressionCodec::Gzip).unwrap();
/// let decompressed = decompress_message(&compressed, CompressionCodec::Gzip).unwrap();
///
/// // Decompressed message is always returned as Binary
/// match decompressed {
///     Message::Binary { data } => {
///         assert_eq!(String::from_utf8(data).unwrap(), "Hello, World!");
///     },
///     _ => panic!("Expected binary message"),
/// }
/// ```
#[cfg(feature = "compression")]
pub fn decompress_message(message: &Message, codec: CompressionCodec) -> WebSocketResult<Message> {
	let data = match message {
		Message::Binary { data } => data.as_slice(),
		_ => {
			return Err(WebSocketError::Protocol(
				"Can only decompress binary messages".to_string(),
			));
		}
	};

	let decompressed = match codec {
		CompressionCodec::Gzip => decompress_gzip(data)?,
		CompressionCodec::Deflate => decompress_deflate(data)?,
		CompressionCodec::Brotli => decompress_brotli(data)?,
	};

	// Return as binary message (original type information is not preserved)
	Ok(Message::Binary { data: decompressed })
}

#[cfg(feature = "compression")]
fn compress_gzip(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use flate2::Compression;
	use flate2::write::GzEncoder;
	use std::io::Write;

	let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
	encoder
		.write_all(data)
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))?;
	encoder
		.finish()
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))
}

#[cfg(feature = "compression")]
fn decompress_gzip(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use flate2::read::GzDecoder;
	use std::io::Read;

	let mut decoder = GzDecoder::new(data);
	let mut decompressed = Vec::new();
	decoder
		.read_to_end(&mut decompressed)
		.map_err(|_| WebSocketError::Protocol("decompression failed".to_string()))?;
	Ok(decompressed)
}

#[cfg(feature = "compression")]
fn compress_deflate(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use flate2::Compression;
	use flate2::write::DeflateEncoder;
	use std::io::Write;

	let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
	encoder
		.write_all(data)
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))?;
	encoder
		.finish()
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))
}

#[cfg(feature = "compression")]
fn decompress_deflate(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use flate2::read::DeflateDecoder;
	use std::io::Read;

	let mut decoder = DeflateDecoder::new(data);
	let mut decompressed = Vec::new();
	decoder
		.read_to_end(&mut decompressed)
		.map_err(|_| WebSocketError::Protocol("decompression failed".to_string()))?;
	Ok(decompressed)
}

#[cfg(feature = "compression")]
fn compress_brotli(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use std::io::Write;

	let mut compressed = Vec::new();
	let mut compressor = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
	compressor
		.write_all(data)
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))?;
	compressor
		.flush()
		.map_err(|_| WebSocketError::Protocol("compression failed".to_string()))?;
	drop(compressor);
	Ok(compressed)
}

#[cfg(feature = "compression")]
fn decompress_brotli(data: &[u8]) -> WebSocketResult<Vec<u8>> {
	use std::io::Read;

	let mut decompressor = brotli::Decompressor::new(data, 4096);
	let mut decompressed = Vec::new();
	decompressor
		.read_to_end(&mut decompressed)
		.map_err(|_| WebSocketError::Protocol("decompression failed".to_string()))?;
	Ok(decompressed)
}

/// Decompresses a message with size limits from a [`CompressionConfig`].
///
/// This function enforces the `max_decompressed_size` limit to prevent
/// decompression bomb attacks. If the decompressed data exceeds the
/// configured limit, an error is returned.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::compression::{
///     CompressionCodec, CompressionConfig, compress_message, decompress_message_with_config,
/// };
/// use reinhardt_websockets::Message;
///
/// let original = Message::text("Hello, World!".to_string());
/// let compressed = compress_message(&original, CompressionCodec::Gzip).unwrap();
///
/// let config = CompressionConfig::new()
///     .with_max_decompressed_size(1024 * 1024); // 1MB limit
///
/// let decompressed = decompress_message_with_config(
///     &compressed, CompressionCodec::Gzip, &config,
/// ).unwrap();
///
/// match decompressed {
///     Message::Binary { data } => {
///         assert_eq!(String::from_utf8(data).unwrap(), "Hello, World!");
///     },
///     _ => panic!("Expected binary message"),
/// }
/// ```
#[cfg(feature = "compression")]
pub fn decompress_message_with_config(
	message: &Message,
	codec: CompressionCodec,
	config: &CompressionConfig,
) -> WebSocketResult<Message> {
	if !config.enabled() {
		return Err(WebSocketError::Protocol(
			"Compression is disabled by configuration".to_string(),
		));
	}

	let data = match message {
		Message::Binary { data } => data.as_slice(),
		_ => {
			return Err(WebSocketError::Protocol(
				"Can only decompress binary messages".to_string(),
			));
		}
	};

	let decompressed = match codec {
		CompressionCodec::Gzip => decompress_gzip_limited(data, config.max_decompressed_size())?,
		CompressionCodec::Deflate => {
			decompress_deflate_limited(data, config.max_decompressed_size())?
		}
		CompressionCodec::Brotli => {
			decompress_brotli_limited(data, config.max_decompressed_size())?
		}
	};

	Ok(Message::Binary { data: decompressed })
}

/// Feature-gated no-op for config-aware decompression when compression is disabled.
#[cfg(not(feature = "compression"))]
pub fn decompress_message_with_config(
	_message: &Message,
	_codec: CompressionCodec,
	_config: &CompressionConfig,
) -> WebSocketResult<Message> {
	Err(WebSocketError::Protocol(
		"Compression feature not enabled. Add 'compression' feature to Cargo.toml.".to_string(),
	))
}

#[cfg(feature = "compression")]
fn decompress_gzip_limited(data: &[u8], max_size: usize) -> WebSocketResult<Vec<u8>> {
	use flate2::read::GzDecoder;
	use std::io::Read;

	let mut decoder = GzDecoder::new(data);
	let mut decompressed = Vec::new();
	let mut buf = [0u8; 8192];

	loop {
		let n = decoder
			.read(&mut buf)
			.map_err(|e| WebSocketError::Protocol(format!("Gzip decompression failed: {}", e)))?;
		if n == 0 {
			break;
		}
		if decompressed.len() + n > max_size {
			return Err(WebSocketError::Protocol(format!(
				"Decompressed data exceeds maximum size limit ({} bytes)",
				max_size
			)));
		}
		decompressed.extend_from_slice(&buf[..n]);
	}

	Ok(decompressed)
}

#[cfg(feature = "compression")]
fn decompress_deflate_limited(data: &[u8], max_size: usize) -> WebSocketResult<Vec<u8>> {
	use flate2::read::DeflateDecoder;
	use std::io::Read;

	let mut decoder = DeflateDecoder::new(data);
	let mut decompressed = Vec::new();
	let mut buf = [0u8; 8192];

	loop {
		let n = decoder.read(&mut buf).map_err(|e| {
			WebSocketError::Protocol(format!("Deflate decompression failed: {}", e))
		})?;
		if n == 0 {
			break;
		}
		if decompressed.len() + n > max_size {
			return Err(WebSocketError::Protocol(format!(
				"Decompressed data exceeds maximum size limit ({} bytes)",
				max_size
			)));
		}
		decompressed.extend_from_slice(&buf[..n]);
	}

	Ok(decompressed)
}

#[cfg(feature = "compression")]
fn decompress_brotli_limited(data: &[u8], max_size: usize) -> WebSocketResult<Vec<u8>> {
	use std::io::Read;

	let mut decompressor = brotli::Decompressor::new(data, 4096);
	let mut decompressed = Vec::new();
	let mut buf = [0u8; 8192];

	loop {
		let n = decompressor
			.read(&mut buf)
			.map_err(|e| WebSocketError::Protocol(format!("Brotli decompression failed: {}", e)))?;
		if n == 0 {
			break;
		}
		if decompressed.len() + n > max_size {
			return Err(WebSocketError::Protocol(format!(
				"Decompressed data exceeds maximum size limit ({} bytes)",
				max_size
			)));
		}
		decompressed.extend_from_slice(&buf[..n]);
	}

	Ok(decompressed)
}

/// Feature-gated no-op: Returns error when compression is disabled.
///
/// To enable compression functionality, add the `compression` feature:
/// ```toml
/// reinhardt-websockets = { version = "...", features = ["compression"] }
/// ```
#[cfg(not(feature = "compression"))]
pub fn compress_message(_message: &Message, _codec: CompressionCodec) -> WebSocketResult<Message> {
	Err(WebSocketError::Protocol(
		"compression not available".to_string(),
	))
}

/// Feature-gated no-op: Returns error when compression is disabled.
///
/// To enable compression functionality, add the `compression` feature:
/// ```toml
/// reinhardt-websockets = { version = "...", features = ["compression"] }
/// ```
#[cfg(not(feature = "compression"))]
pub fn decompress_message(
	_message: &Message,
	_codec: CompressionCodec,
) -> WebSocketResult<Message> {
	Err(WebSocketError::Protocol(
		"compression not available".to_string(),
	))
}

#[cfg(test)]
mod config_tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_compression_config_default() {
		// Arrange & Act
		let config = CompressionConfig::default();

		// Assert
		assert!(config.enabled());
		assert_eq!(config.max_window_bits(), 15);
		assert_eq!(config.max_decompressed_size(), 10 * 1024 * 1024);
	}

	#[rstest]
	fn test_compression_config_disabled() {
		// Arrange & Act
		let config = CompressionConfig::new().with_enabled(false);

		// Assert
		assert!(!config.enabled());
	}

	#[rstest]
	fn test_compression_config_custom_window_bits() {
		// Arrange & Act
		let config = CompressionConfig::new().with_max_window_bits(12);

		// Assert
		assert_eq!(config.max_window_bits(), 12);
	}

	#[rstest]
	fn test_compression_config_window_bits_clamped_low() {
		// Arrange & Act
		let config = CompressionConfig::new().with_max_window_bits(3);

		// Assert
		assert_eq!(config.max_window_bits(), 8);
	}

	#[rstest]
	fn test_compression_config_window_bits_clamped_high() {
		// Arrange & Act
		let config = CompressionConfig::new().with_max_window_bits(20);

		// Assert
		assert_eq!(config.max_window_bits(), 15);
	}

	#[rstest]
	fn test_compression_config_custom_max_size() {
		// Arrange & Act
		let config = CompressionConfig::new().with_max_decompressed_size(5 * 1024 * 1024);

		// Assert
		assert_eq!(config.max_decompressed_size(), 5 * 1024 * 1024);
	}

	#[rstest]
	fn test_compression_config_builder_chain() {
		// Arrange & Act
		let config = CompressionConfig::new()
			.with_enabled(true)
			.with_max_window_bits(10)
			.with_max_decompressed_size(1024);

		// Assert
		assert!(config.enabled());
		assert_eq!(config.max_window_bits(), 10);
		assert_eq!(config.max_decompressed_size(), 1024);
	}
}

#[cfg(all(test, feature = "compression"))]
mod tests {
	use super::*;
	use std::str::FromStr;

	#[test]
	fn test_compression_codec_from_str() {
		assert_eq!(
			CompressionCodec::from_str("gzip").unwrap(),
			CompressionCodec::Gzip
		);
		assert_eq!(
			CompressionCodec::from_str("deflate").unwrap(),
			CompressionCodec::Deflate
		);
		assert_eq!(
			CompressionCodec::from_str("br").unwrap(),
			CompressionCodec::Brotli
		);
		assert_eq!(
			CompressionCodec::from_str("brotli").unwrap(),
			CompressionCodec::Brotli
		);
		assert!(CompressionCodec::from_str("unknown").is_err());
	}

	#[test]
	fn test_compression_codec_as_str() {
		assert_eq!(CompressionCodec::Gzip.as_str(), "gzip");
		assert_eq!(CompressionCodec::Deflate.as_str(), "deflate");
		assert_eq!(CompressionCodec::Brotli.as_str(), "br");
	}

	#[test]
	fn test_gzip_compression_text_message() {
		let text = "Hello, World!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
		let decompressed = decompress_message(&compressed, CompressionCodec::Gzip).unwrap();

		// After decompression, it becomes a binary message
		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_deflate_compression_text_message() {
		let text = "Hello, World!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Deflate).unwrap();
		let decompressed = decompress_message(&compressed, CompressionCodec::Deflate).unwrap();

		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_brotli_compression_text_message() {
		let text = "Hello, World!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Brotli).unwrap();
		let decompressed = decompress_message(&compressed, CompressionCodec::Brotli).unwrap();

		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_gzip_compression_binary_message() {
		let data = vec![1, 2, 3, 4, 5];
		let message = Message::binary(data.clone());
		let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
		let decompressed = decompress_message(&compressed, CompressionCodec::Gzip).unwrap();

		match decompressed {
			Message::Binary {
				data: decompressed_data,
			} => {
				assert_eq!(decompressed_data, data);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_compression_reduces_size() {
		// Long repeated text has high compression efficiency
		let long_text = "Hello, World! ".repeat(100);
		let message = Message::text(long_text.clone());
		let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();

		match compressed {
			Message::Binary { data } => {
				// Compressed data should be smaller than the original text
				assert!(data.len() < long_text.len());
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_compress_non_data_message_fails() {
		let message = Message::Ping;
		let result = compress_message(&message, CompressionCodec::Gzip);
		assert!(result.is_err());
	}

	#[test]
	fn test_decompress_text_message_fails() {
		let message = Message::text("Not compressed".to_string());
		let result = decompress_message(&message, CompressionCodec::Gzip);
		assert!(result.is_err());
	}

	#[test]
	fn test_decompress_with_config_gzip() {
		// Arrange
		let text = "Hello, World!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
		let config = CompressionConfig::default();

		// Act
		let decompressed =
			decompress_message_with_config(&compressed, CompressionCodec::Gzip, &config).unwrap();

		// Assert
		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_decompress_with_config_deflate() {
		// Arrange
		let text = "Hello, Deflate!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Deflate).unwrap();
		let config = CompressionConfig::default();

		// Act
		let decompressed =
			decompress_message_with_config(&compressed, CompressionCodec::Deflate, &config)
				.unwrap();

		// Assert
		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_decompress_with_config_brotli() {
		// Arrange
		let text = "Hello, Brotli!".to_string();
		let message = Message::text(text.clone());
		let compressed = compress_message(&message, CompressionCodec::Brotli).unwrap();
		let config = CompressionConfig::default();

		// Act
		let decompressed =
			decompress_message_with_config(&compressed, CompressionCodec::Brotli, &config).unwrap();

		// Assert
		match decompressed {
			Message::Binary { data } => {
				assert_eq!(String::from_utf8(data).unwrap(), text);
			}
			_ => panic!("Expected binary message"),
		}
	}

	#[test]
	fn test_decompress_with_config_rejects_oversized() {
		// Arrange - compress a large message and set a small limit
		let large_text = "A".repeat(1024);
		let message = Message::text(large_text);
		let compressed = compress_message(&message, CompressionCodec::Gzip).unwrap();
		let config = CompressionConfig::new().with_max_decompressed_size(100); // 100 byte limit

		// Act
		let result = decompress_message_with_config(&compressed, CompressionCodec::Gzip, &config);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.internal_detail().contains("exceeds maximum size limit"));
	}

	#[test]
	fn test_decompress_with_config_disabled() {
		// Arrange
		let message = Message::binary(vec![1, 2, 3]);
		let config = CompressionConfig::new().with_enabled(false);

		// Act
		let result = decompress_message_with_config(&message, CompressionCodec::Gzip, &config);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.internal_detail().contains("disabled"));
	}
}
