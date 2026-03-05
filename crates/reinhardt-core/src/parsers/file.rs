use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;

use super::parser::{ParseError, ParseResult, ParsedData, Parser, UploadedFile};

/// Raw file upload parser
#[derive(Debug, Clone)]
pub struct FileUploadParser {
	/// Maximum file size in bytes (None = unlimited)
	pub max_file_size: Option<usize>,
	/// Field name for the file
	pub field_name: String,
}

impl FileUploadParser {
	/// Parse filename from Content-Disposition header.
	/// Supports both standard filename and RFC2231 encoded filename* parameters.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::file::FileUploadParser;
	///
	/// let parser = FileUploadParser::new("file");
	/// let disposition = "inline; filename=document.txt";
	/// let filename = parser.get_filename(Some(disposition)).unwrap();
	/// assert_eq!(filename, "document.txt");
	///
	/// // RFC2231 encoded filename
	/// let disposition_encoded = "inline; filename*=utf-8''%C3%A0.txt";
	/// let filename_encoded = parser.get_filename(Some(disposition_encoded)).unwrap();
	/// assert_eq!(filename_encoded, "à.txt");
	/// ```
	pub fn get_filename(&self, content_disposition: Option<&str>) -> Result<String, ParseError> {
		let disposition = content_disposition.ok_or_else(|| {
            ParseError::ParseError(
                "Missing filename. Request should include a Content-Disposition header with a filename parameter.".to_string()
            )
        })?;

		if disposition.trim().is_empty() {
			return Err(ParseError::ParseError(
                "Missing filename. Request should include a Content-Disposition header with a filename parameter.".to_string()
            ));
		}

		// RFC2231 encoded filename* takes precedence
		if let Some(encoded_filename) = Self::extract_encoded_filename(disposition) {
			return Ok(encoded_filename);
		}

		// Standard filename parameter
		if let Some(filename) = Self::extract_standard_filename(disposition) {
			return Ok(filename);
		}

		Err(ParseError::ParseError(
            "Missing filename. Request should include a Content-Disposition header with a filename parameter.".to_string()
        ))
	}

	fn extract_encoded_filename(disposition: &str) -> Option<String> {
		// RFC2231: filename*=utf-8''encoded_name or filename*=utf-8'lang'encoded_name
		for part in disposition.split(';') {
			let part = part.trim();
			if part.starts_with("filename*=") {
				let value = part.trim_start_matches("filename*=");

				// Parse RFC2231 format: charset'language'value
				// Find the first single quote to get charset
				if let Some(first_quote) = value.find('\'') {
					// Find the second single quote to get the encoded value
					let rest = &value[first_quote + 1..];
					if let Some(second_quote) = rest.find('\'') {
						let encoded = &rest[second_quote + 1..];
						// URL decode the value
						if let Ok(decoded) = urlencoding::decode(encoded) {
							return Some(decoded.to_string());
						}
					}
				}
			}
		}
		None
	}

	fn extract_standard_filename(disposition: &str) -> Option<String> {
		for part in disposition.split(';') {
			let part = part.trim();
			if part.starts_with("filename=") && !part.starts_with("filename*=") {
				let value = part.trim_start_matches("filename=");
				// Remove quotes if present
				let value = value.trim_matches('"').trim_matches('\'');
				return Some(value.to_string());
			}
		}
		None
	}
}

impl FileUploadParser {
	/// Create a new FileUploadParser with the specified field name.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::file::FileUploadParser;
	///
	/// let parser = FileUploadParser::new("document");
	/// assert_eq!(parser.field_name, "document");
	/// assert!(parser.max_file_size.is_none());
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			max_file_size: None,
			field_name: field_name.into(),
		}
	}
	/// Set the maximum file size in bytes.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::file::FileUploadParser;
	///
	/// let parser = FileUploadParser::new("file").max_file_size(5 * 1024 * 1024); // 5MB
	/// assert_eq!(parser.max_file_size, Some(5 * 1024 * 1024));
	/// ```
	pub fn max_file_size(mut self, size: usize) -> Self {
		self.max_file_size = Some(size);
		self
	}
}

impl Default for FileUploadParser {
	fn default() -> Self {
		Self {
			max_file_size: None,
			field_name: "file".to_string(),
		}
	}
}

#[async_trait]
impl Parser for FileUploadParser {
	fn media_types(&self) -> Vec<String> {
		vec!["application/octet-stream".to_string(), "*/*".to_string()]
	}

	async fn parse(
		&self,
		content_type: Option<&str>,
		body: Bytes,
		_headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		let size = body.len();

		// Check file size limit
		if let Some(max_size) = self.max_file_size
			&& size > max_size
		{
			return Err(ParseError::ParseError(format!(
				"File exceeds maximum size of {} bytes",
				max_size
			)));
		}

		let mut file = UploadedFile::new(self.field_name.clone(), body);

		if let Some(ct) = content_type {
			file = file.with_content_type(ct.to_string());
		}

		Ok(ParsedData::File(file))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_file_upload_parser_valid() {
		let parser = FileUploadParser::new("upload");
		let body = Bytes::from("binary file content here");
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/octet-stream"), body.clone(), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::File(file) => {
				assert_eq!(file.name, "upload");
				assert_eq!(file.data, body);
				assert_eq!(file.size, body.len());
				assert_eq!(
					file.content_type,
					Some("application/octet-stream".to_string())
				);
			}
			_ => panic!("Expected file data"),
		}
	}

	#[tokio::test]
	async fn test_file_upload_parser_max_size() {
		let parser = FileUploadParser::new("upload").max_file_size(10);
		let body = Bytes::from("this is a very long file content that exceeds the limit");
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/octet-stream"), body, &headers)
			.await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_file_upload_parser_no_content_type() {
		let parser = FileUploadParser::new("upload");
		let body = Bytes::from("file content");
		let headers = HeaderMap::new();

		let result = parser.parse(None, body.clone(), &headers).await.unwrap();

		match result {
			ParsedData::File(file) => {
				assert_eq!(file.name, "upload");
				assert_eq!(file.data, body);
				assert_eq!(file.content_type, None);
			}
			_ => panic!("Expected file data"),
		}
	}

	#[test]
	fn test_file_upload_parser_media_types() {
		let parser = FileUploadParser::new("upload");
		let media_types = parser.media_types();

		assert!(media_types.contains(&"application/octet-stream".to_string()));
		assert!(media_types.contains(&"*/*".to_string()));
	}

	// Tests from Django REST Framework

	#[tokio::test]
	async fn test_file_parse_drf() {
		// DRF test: Parse raw file upload
		let parser = FileUploadParser::new("file");
		let body = Bytes::from("Test text file");
		let headers = HeaderMap::new();

		let content_disposition = "Content-Disposition: inline; filename=file.txt";
		let filename = parser.get_filename(Some(content_disposition)).unwrap();

		let result = parser
			.parse(Some("application/octet-stream"), body.clone(), &headers)
			.await
			.unwrap();

		match result {
			ParsedData::File(file) => {
				assert_eq!(file.size, 14);
				assert_eq!(filename, "file.txt");
			}
			_ => panic!("Expected file data"),
		}
	}

	#[tokio::test]
	async fn test_parse_missing_filename() {
		// DRF test: Parse raw file upload when filename is missing
		let parser = FileUploadParser::new("file");

		let result = parser.get_filename(Some(""));
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err().to_string(),
			"Parse error: Missing filename. Request should include a Content-Disposition header with a filename parameter."
		);
	}

	#[tokio::test]
	async fn test_parse_missing_filename_none() {
		// DRF test: Parse when Content-Disposition header is None
		let parser = FileUploadParser::new("file");

		let result = parser.get_filename(None);
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err().to_string(),
			"Parse error: Missing filename. Request should include a Content-Disposition header with a filename parameter."
		);
	}

	#[test]
	fn test_get_filename() {
		// DRF test: Get filename from Content-Disposition header
		let parser = FileUploadParser::new("file");
		let content_disposition = "Content-Disposition: inline; filename=file.txt";

		let filename = parser.get_filename(Some(content_disposition)).unwrap();
		assert_eq!(filename, "file.txt");
	}

	#[test]
	fn test_get_encoded_filename() {
		// DRF test: Get RFC2231 encoded filename
		let parser = FileUploadParser::new("file");

		// Test 1: filename* only
		let disposition = "inline; filename*=utf-8''%C3%80%C4%A5%C6%A6.txt";
		let filename = parser.get_filename(Some(disposition)).unwrap();
		assert_eq!(filename, "ÀĥƦ.txt");

		// Test 2: Both filename and filename* (filename* takes precedence)
		let disposition = "inline; filename=fallback.txt; filename*=utf-8''%C3%80%C4%A5%C6%A6.txt";
		let filename = parser.get_filename(Some(disposition)).unwrap();
		assert_eq!(filename, "ÀĥƦ.txt");

		// Test 3: With language tag
		let disposition =
			"inline; filename=fallback.txt; filename*=utf-8'en-us'%C3%80%C4%A5%C6%A6.txt";
		let filename = parser.get_filename(Some(disposition)).unwrap();
		assert_eq!(filename, "ÀĥƦ.txt");
	}
}
