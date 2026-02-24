//! Compression support for parsers
//!
//! Provides transparent decompression of request bodies with gzip, brotli, or deflate encoding.

use super::parser::{ParseResult, ParsedData, Parser};
use crate::exception::Error;
use async_trait::async_trait;
use brotli::Decompressor;
use bytes::Bytes;
use flate2::read::{DeflateDecoder, GzDecoder};
use http::HeaderMap;
use std::io::Read;
use std::sync::Arc;

/// Supported compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionEncoding {
	/// Gzip compression (Content-Encoding: gzip)
	Gzip,
	/// Brotli compression (Content-Encoding: br)
	Brotli,
	/// Deflate compression (Content-Encoding: deflate)
	Deflate,
}

impl CompressionEncoding {
	/// Parse compression encoding from Content-Encoding header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::compressed::CompressionEncoding;
	///
	/// assert_eq!(
	///     CompressionEncoding::from_header("gzip"),
	///     Some(CompressionEncoding::Gzip)
	/// );
	/// assert_eq!(
	///     CompressionEncoding::from_header("br"),
	///     Some(CompressionEncoding::Brotli)
	/// );
	/// assert_eq!(
	///     CompressionEncoding::from_header("deflate"),
	///     Some(CompressionEncoding::Deflate)
	/// );
	/// assert_eq!(CompressionEncoding::from_header("unknown"), None);
	/// ```
	pub fn from_header(encoding: &str) -> Option<Self> {
		match encoding.to_lowercase().as_str() {
			"gzip" => Some(Self::Gzip),
			"br" => Some(Self::Brotli),
			"deflate" => Some(Self::Deflate),
			_ => None,
		}
	}

	/// Default maximum decompressed output size (100 MB)
	const DEFAULT_MAX_OUTPUT_SIZE: u64 = 100 * 1024 * 1024;

	/// Decompress data using this encoding with a size limit to prevent
	/// decompression bomb attacks.
	fn decompress(&self, data: &[u8]) -> ParseResult<Vec<u8>> {
		self.decompress_with_limit(data, Self::DEFAULT_MAX_OUTPUT_SIZE)
	}

	/// Decompress data with an explicit output size limit.
	fn decompress_with_limit(&self, data: &[u8], max_output_size: u64) -> ParseResult<Vec<u8>> {
		match self {
			Self::Gzip => {
				let decoder = GzDecoder::new(data);
				let mut limited = decoder.take(max_output_size);
				let mut decompressed = Vec::new();
				limited
					.read_to_end(&mut decompressed)
					.map_err(|e| Error::ParseError(format!("Gzip decompression error: {}", e)))?;
				Ok(decompressed)
			}
			Self::Brotli => {
				let decoder = Decompressor::new(data, 4096);
				let mut limited = decoder.take(max_output_size);
				let mut decompressed = Vec::new();
				limited
					.read_to_end(&mut decompressed)
					.map_err(|e| Error::ParseError(format!("Brotli decompression error: {}", e)))?;
				Ok(decompressed)
			}
			Self::Deflate => {
				let decoder = DeflateDecoder::new(data);
				let mut limited = decoder.take(max_output_size);
				let mut decompressed = Vec::new();
				limited.read_to_end(&mut decompressed).map_err(|e| {
					Error::ParseError(format!("Deflate decompression error: {}", e))
				})?;
				Ok(decompressed)
			}
		}
	}
}

/// Wrapper parser that handles compressed request bodies
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::compressed::CompressedParser;
/// use reinhardt_core::parsers::json::JSONParser;
/// use std::sync::Arc;
///
/// let json_parser = JSONParser::new();
/// let compressed_parser = CompressedParser::new(Arc::new(json_parser));
/// ```
pub struct CompressedParser {
	inner: Arc<dyn Parser>,
}

impl CompressedParser {
	/// Create a new CompressedParser wrapping an existing parser.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::compressed::CompressedParser;
	/// use reinhardt_core::parsers::json::JSONParser;
	/// use std::sync::Arc;
	///
	/// let json_parser = JSONParser::new();
	/// let parser = CompressedParser::new(Arc::new(json_parser));
	/// ```
	pub fn new(inner: Arc<dyn Parser>) -> Self {
		Self { inner }
	}

	/// Decompress body if Content-Encoding header is present
	///
	/// This method provides explicit decompression control when the Content-Encoding
	/// header value is available from the request context.
	///
	/// # Arguments
	///
	/// * `content_encoding` - Optional Content-Encoding header value (e.g., "gzip", "br", "deflate")
	/// * `body` - Request body bytes (potentially compressed)
	///
	/// # Returns
	///
	/// Returns decompressed body if Content-Encoding is recognized, otherwise returns the original body.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::compressed::CompressedParser;
	/// use reinhardt_core::parsers::json::JSONParser;
	/// use bytes::Bytes;
	/// use std::sync::Arc;
	/// use flate2::write::GzEncoder;
	/// use flate2::Compression;
	/// use std::io::Write;
	///
	/// // Create sample JSON data
	/// let json_data = b"{\"test\":\"data\"}";
	///
	/// // Compress with gzip
	/// let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
	/// encoder.write_all(json_data).unwrap();
	/// let compressed = encoder.finish().unwrap();
	///
	/// // Decompress using CompressedParser
	/// let parser = CompressedParser::new(Arc::new(JSONParser::new()));
	/// let body = Bytes::from(compressed);
	/// let decompressed = parser.decompress_if_needed(Some("gzip"), body).unwrap();
	///
	/// // Verify decompression succeeded
	/// assert_eq!(decompressed.as_ref(), json_data);
	/// ```
	pub fn decompress_if_needed(
		&self,
		content_encoding: Option<&str>,
		body: Bytes,
	) -> ParseResult<Bytes> {
		if let Some(encoding_str) = content_encoding
			&& let Some(encoding) = CompressionEncoding::from_header(encoding_str)
		{
			let decompressed = encoding.decompress(&body)?;
			return Ok(Bytes::from(decompressed));
		}
		Ok(body)
	}
}

#[async_trait]
impl Parser for CompressedParser {
	fn media_types(&self) -> Vec<String> {
		self.inner.media_types()
	}

	async fn parse(
		&self,
		content_type: Option<&str>,
		body: Bytes,
		headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		// Extract Content-Encoding from headers
		let encoding = headers
			.get("content-encoding")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("identity");

		// Decompress based on Content-Encoding header
		let decompressed = match encoding {
			"gzip" => CompressionEncoding::Gzip
				.decompress(&body)
				.map(Bytes::from)?,
			"deflate" => CompressionEncoding::Deflate
				.decompress(&body)
				.map(Bytes::from)?,
			"br" => CompressionEncoding::Brotli
				.decompress(&body)
				.map(Bytes::from)?,
			// "identity" or any other unknown encoding - return body as-is
			_ => body,
		};

		// Parse decompressed data
		self.inner.parse(content_type, decompressed, headers).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parsers::json::JSONParser;
	use flate2::Compression;
	use flate2::write::{DeflateEncoder, GzEncoder};
	use std::io::Write;

	#[test]
	fn test_compression_encoding_from_header() {
		assert_eq!(
			CompressionEncoding::from_header("gzip"),
			Some(CompressionEncoding::Gzip)
		);
		assert_eq!(
			CompressionEncoding::from_header("GZIP"),
			Some(CompressionEncoding::Gzip)
		);
		assert_eq!(
			CompressionEncoding::from_header("br"),
			Some(CompressionEncoding::Brotli)
		);
		assert_eq!(
			CompressionEncoding::from_header("deflate"),
			Some(CompressionEncoding::Deflate)
		);
		assert_eq!(CompressionEncoding::from_header("unknown"), None);
	}

	#[tokio::test]
	async fn test_compressed_parser_gzip() {
		use http::HeaderMap;

		let json_data = r#"{"name":"John","age":30}"#;

		// Compress with gzip
		let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(json_data.as_bytes()).unwrap();
		let compressed = encoder.finish().unwrap();

		// Create headers with Content-Encoding
		let mut headers = HeaderMap::new();
		headers.insert("content-encoding", "gzip".parse().unwrap());

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.parse(Some("application/json"), Bytes::from(compressed), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::Json(value) => {
				assert_eq!(value["name"], "John");
				assert_eq!(value["age"], 30);
			}
			_ => panic!("Expected JSON data"),
		}
	}

	#[tokio::test]
	async fn test_compressed_parser_deflate() {
		use http::HeaderMap;

		let json_data = r#"{"name":"Alice","city":"NYC"}"#;

		// Compress with deflate
		let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(json_data.as_bytes()).unwrap();
		let compressed = encoder.finish().unwrap();

		// Create headers with Content-Encoding
		let mut headers = HeaderMap::new();
		headers.insert("content-encoding", "deflate".parse().unwrap());

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.parse(Some("application/json"), Bytes::from(compressed), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::Json(value) => {
				assert_eq!(value["name"], "Alice");
				assert_eq!(value["city"], "NYC");
			}
			_ => panic!("Expected JSON data"),
		}
	}

	#[tokio::test]
	async fn test_compressed_parser_brotli() {
		use http::HeaderMap;

		let json_data = r#"{"product":"widget","price":19.99}"#;

		// Compress with brotli
		let mut compressed = Vec::new();
		{
			let mut encoder = brotli::CompressorWriter::new(
				&mut compressed,
				4096,
				11, // quality
				22, // lg_window_size
			);
			encoder.write_all(json_data.as_bytes()).unwrap();
		}

		// Create headers with Content-Encoding
		let mut headers = HeaderMap::new();
		headers.insert("content-encoding", "br".parse().unwrap());

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.parse(Some("application/json"), Bytes::from(compressed), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::Json(value) => {
				assert_eq!(value["product"], "widget");
				assert_eq!(value["price"], 19.99);
			}
			_ => panic!("Expected JSON data"),
		}
	}

	#[tokio::test]
	async fn test_compressed_parser_uncompressed() {
		use http::HeaderMap;

		let json_data = r#"{"uncompressed":true}"#;

		// No Content-Encoding header (identity)
		let headers = HeaderMap::new();

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.parse(Some("application/json"), Bytes::from(json_data), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::Json(value) => {
				assert!(value["uncompressed"].as_bool().unwrap());
			}
			_ => panic!("Expected JSON data"),
		}
	}

	#[test]
	fn test_compressed_parser_media_types() {
		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let media_types = parser.media_types();

		assert!(media_types.contains(&"application/json".to_string()));
	}

	#[tokio::test]
	async fn test_gzip_decompression() {
		let original = b"Hello, World!";

		let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(original).unwrap();
		let compressed = encoder.finish().unwrap();

		let decompressed = CompressionEncoding::Gzip.decompress(&compressed).unwrap();

		assert_eq!(decompressed, original);
	}

	#[tokio::test]
	async fn test_deflate_decompression() {
		let original = b"Test data for deflate";

		let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(original).unwrap();
		let compressed = encoder.finish().unwrap();

		let decompressed = CompressionEncoding::Deflate
			.decompress(&compressed)
			.unwrap();

		assert_eq!(decompressed, original);
	}

	#[tokio::test]
	async fn test_brotli_decompression() {
		let original = b"Brotli compressed content";

		let mut compressed = Vec::new();
		{
			let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
			encoder.write_all(original).unwrap();
		}

		let decompressed = CompressionEncoding::Brotli.decompress(&compressed).unwrap();

		assert_eq!(decompressed, original);
	}

	#[tokio::test]
	async fn test_invalid_gzip_data() {
		let invalid_data = b"not gzip data";
		let result = CompressionEncoding::Gzip.decompress(invalid_data);
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_invalid_brotli_data() {
		let invalid_data = b"not brotli data";
		let result = CompressionEncoding::Brotli.decompress(invalid_data);
		assert!(result.is_err());
	}

	#[test]
	fn test_decompress_if_needed_gzip() {
		let original = b"Hello, World!";

		// Compress with gzip
		let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(original).unwrap();
		let compressed = encoder.finish().unwrap();

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.decompress_if_needed(Some("gzip"), Bytes::from(compressed))
			.unwrap();

		assert_eq!(result.as_ref(), original);
	}

	#[test]
	fn test_decompress_if_needed_deflate() {
		let original = b"Test data for deflate";

		// Compress with deflate
		let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(original).unwrap();
		let compressed = encoder.finish().unwrap();

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.decompress_if_needed(Some("deflate"), Bytes::from(compressed))
			.unwrap();

		assert_eq!(result.as_ref(), original);
	}

	#[test]
	fn test_decompress_if_needed_brotli() {
		let original = b"Brotli compressed content";

		// Compress with brotli
		let mut compressed = Vec::new();
		{
			let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
			encoder.write_all(original).unwrap();
		}

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.decompress_if_needed(Some("br"), Bytes::from(compressed))
			.unwrap();

		assert_eq!(result.as_ref(), original);
	}

	#[test]
	fn test_decompress_if_needed_no_encoding() {
		let original = b"Uncompressed data";

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.decompress_if_needed(None, Bytes::from(original.as_slice()))
			.unwrap();

		assert_eq!(result.as_ref(), original);
	}

	#[test]
	fn test_decompress_if_needed_unknown_encoding() {
		let original = b"Unknown encoding";

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));
		let result = parser
			.decompress_if_needed(Some("unknown"), Bytes::from(original.as_slice()))
			.unwrap();

		// Should return original data unchanged
		assert_eq!(result.as_ref(), original);
	}

	#[test]
	fn test_decompress_if_needed_case_insensitive() {
		let original = b"Case test";

		// Compress with gzip
		let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(original).unwrap();
		let compressed = encoder.finish().unwrap();

		let parser = CompressedParser::new(Arc::new(JSONParser::new()));

		// Test with uppercase
		let result = parser
			.decompress_if_needed(Some("GZIP"), Bytes::from(compressed.clone()))
			.unwrap();
		assert_eq!(result.as_ref(), original);

		// Test with mixed case
		let result = parser
			.decompress_if_needed(Some("GzIp"), Bytes::from(compressed))
			.unwrap();
		assert_eq!(result.as_ref(), original);
	}
}
