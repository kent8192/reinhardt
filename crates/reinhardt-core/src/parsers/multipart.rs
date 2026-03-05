use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use multer::Multipart as MulterMultipart;
use std::collections::HashMap;

use super::parser::{MediaType, ParseError, ParseResult, ParsedData, Parser, UploadedFile};

/// Default maximum file size: 10 MB
const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Default maximum total upload size: 50 MB
const DEFAULT_MAX_TOTAL_SIZE: usize = 50 * 1024 * 1024;

/// MultiPart parser for multipart/form-data content type (file uploads)
///
/// By default, enforces sensible size limits to prevent denial-of-service
/// via large uploads:
/// - Per-file limit: 10 MB
/// - Total upload limit: 50 MB
///
/// Use the builder methods to override these limits, or
/// [`MultiPartParser::unlimited()`] to remove all limits.
#[derive(Debug, Clone)]
pub struct MultiPartParser {
	/// Maximum file size in bytes (`None` = unlimited)
	pub max_file_size: Option<usize>,
	/// Maximum total size in bytes (`None` = unlimited)
	pub max_total_size: Option<usize>,
}

impl Default for MultiPartParser {
	/// Creates a parser with default size limits (10 MB per file, 50 MB total).
	fn default() -> Self {
		Self {
			max_file_size: Some(DEFAULT_MAX_FILE_SIZE),
			max_total_size: Some(DEFAULT_MAX_TOTAL_SIZE),
		}
	}
}

impl MultiPartParser {
	/// Create a new MultiPartParser with default size limits.
	///
	/// Default limits:
	/// - Per-file: 10 MB
	/// - Total: 50 MB
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::multipart::MultiPartParser;
	///
	/// let parser = MultiPartParser::new();
	/// assert_eq!(parser.max_file_size, Some(10 * 1024 * 1024));
	/// assert_eq!(parser.max_total_size, Some(50 * 1024 * 1024));
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a new MultiPartParser with no size limits.
	///
	/// # Safety Note
	///
	/// Using unlimited uploads without external size controls (e.g., reverse
	/// proxy limits) may expose the server to denial-of-service attacks.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::multipart::MultiPartParser;
	///
	/// let parser = MultiPartParser::unlimited();
	/// assert!(parser.max_file_size.is_none());
	/// assert!(parser.max_total_size.is_none());
	/// ```
	pub fn unlimited() -> Self {
		Self {
			max_file_size: None,
			max_total_size: None,
		}
	}

	/// Set the maximum file size in bytes for individual files.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::multipart::MultiPartParser;
	///
	/// let parser = MultiPartParser::new().max_file_size(1024 * 1024); // 1MB
	/// assert_eq!(parser.max_file_size, Some(1024 * 1024));
	/// ```
	pub fn max_file_size(mut self, size: usize) -> Self {
		self.max_file_size = Some(size);
		self
	}
	/// Set the maximum total size in bytes for all uploaded files combined.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::multipart::MultiPartParser;
	///
	/// let parser = MultiPartParser::new().max_total_size(10 * 1024 * 1024); // 10MB
	/// assert_eq!(parser.max_total_size, Some(10 * 1024 * 1024));
	/// ```
	pub fn max_total_size(mut self, size: usize) -> Self {
		self.max_total_size = Some(size);
		self
	}

	async fn parse_multipart(&self, boundary: &str, body: Bytes) -> ParseResult<ParsedData> {
		let mut multipart = MulterMultipart::new(
			futures_util::stream::once(async move { Result::<_, std::io::Error>::Ok(body) }),
			boundary,
		);

		let mut fields = HashMap::new();
		let mut files = Vec::new();
		let mut total_size = 0usize;

		while let Some(field) = multipart
			.next_field()
			.await
			.map_err(|e| ParseError::ParseError(format!("Multipart parse error: {}", e)))?
		{
			let name = field.name().unwrap_or("").to_string();
			let filename = field.file_name().map(|s| s.to_string());
			let content_type = field.content_type().map(|m| m.to_string());

			let data = field
				.bytes()
				.await
				.map_err(|e| ParseError::ParseError(format!("Failed to read field data: {}", e)))?;

			let size = data.len();

			// Check file size limit
			if let Some(max_size) = self.max_file_size
				&& size > max_size
			{
				return Err(ParseError::ParseError(format!(
					"File '{}' exceeds maximum size of {} bytes",
					name, max_size
				)));
			}

			// Check total size limit
			total_size += size;
			if let Some(max_total) = self.max_total_size
				&& total_size > max_total
			{
				return Err(ParseError::ParseError(format!(
					"Total upload size exceeds maximum of {} bytes",
					max_total
				)));
			}

			if filename.is_some() {
				// This is a file field
				let mut file = UploadedFile::new(name.clone(), data);
				if let Some(fname) = filename {
					file = file.with_filename(fname);
				}
				if let Some(ct) = content_type {
					file = file.with_content_type(ct);
				}
				files.push(file);
			} else {
				// This is a regular field
				let value = String::from_utf8_lossy(&data).to_string();
				fields.insert(name, value);
			}
		}

		Ok(ParsedData::MultiPart { fields, files })
	}
}

#[async_trait]
impl Parser for MultiPartParser {
	fn media_types(&self) -> Vec<String> {
		vec!["multipart/form-data".to_string()]
	}

	async fn parse(
		&self,
		content_type: Option<&str>,
		body: Bytes,
		_headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		let content_type = content_type.ok_or(ParseError::MissingContentType)?;

		let media_type = MediaType::parse(content_type)?;

		let boundary = media_type
			.parameters
			.get("boundary")
			.ok_or_else(|| ParseError::ParseError("Missing boundary parameter".to_string()))?;

		self.parse_multipart(boundary, body).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn create_multipart_body(boundary: &str) -> Bytes {
		let body = format!(
			"--{}\r\n\
             Content-Disposition: form-data; name=\"field1\"\r\n\
             \r\n\
             value1\r\n\
             --{}\r\n\
             Content-Disposition: form-data; name=\"field2\"\r\n\
             \r\n\
             value2\r\n\
             --{}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             Hello, World!\r\n\
             --{}--\r\n",
			boundary, boundary, boundary, boundary
		);
		Bytes::from(body)
	}

	#[tokio::test]
	async fn test_multipart_parser_valid() {
		let parser = MultiPartParser::new();
		let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
		let body = create_multipart_body(boundary);
		let content_type = format!("multipart/form-data; boundary={}", boundary);
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some(&content_type), body, &headers)
			.await
			.unwrap();

		match result {
			ParsedData::MultiPart { fields, files } => {
				assert_eq!(fields.get("field1"), Some(&"value1".to_string()));
				assert_eq!(fields.get("field2"), Some(&"value2".to_string()));
				assert_eq!(files.len(), 1);
				assert_eq!(files[0].name, "file");
				assert_eq!(files[0].filename, Some("test.txt".to_string()));
				assert_eq!(files[0].content_type, Some("text/plain".to_string()));
			}
			_ => panic!("Expected multipart data"),
		}
	}

	#[tokio::test]
	async fn test_multipart_parser_max_file_size() {
		let parser = MultiPartParser::new().max_file_size(10); // Very small limit
		let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
		let body = create_multipart_body(boundary);
		let content_type = format!("multipart/form-data; boundary={}", boundary);
		let headers = HeaderMap::new();

		let result = parser.parse(Some(&content_type), body, &headers).await;
		assert!(result.is_err());
	}

	#[test]
	fn test_multipart_parser_media_types() {
		let parser = MultiPartParser::new();
		let media_types = parser.media_types();

		assert!(media_types.contains(&"multipart/form-data".to_string()));
	}
}
