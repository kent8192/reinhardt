use crate::exception::{Error, Result};
use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use serde_json::Value;
use std::collections::HashMap;

/// Type alias for parser errors, using the framework's `Error` type.
pub type ParseError = Error;
/// Type alias for parser results, using the framework's `Result` type.
pub type ParseResult<T> = Result<T>;

/// Media type representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaType {
	/// Primary type (e.g., "application", "text").
	pub main_type: String,
	/// Subtype (e.g., "json", "html").
	pub sub_type: String,
	/// Additional parameters (e.g., charset=utf-8).
	pub parameters: HashMap<String, String>,
}

impl MediaType {
	/// Create a new MediaType with the specified main and sub types.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::MediaType;
	///
	/// let media_type = MediaType::new("application", "json");
	/// assert_eq!(media_type.main_type, "application");
	/// assert_eq!(media_type.sub_type, "json");
	/// assert!(media_type.parameters.is_empty());
	/// ```
	pub fn new(main_type: impl Into<String>, sub_type: impl Into<String>) -> Self {
		Self {
			main_type: main_type.into(),
			sub_type: sub_type.into(),
			parameters: HashMap::new(),
		}
	}
	/// Add a parameter to the media type (e.g., charset=utf-8).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::MediaType;
	///
	/// let media_type = MediaType::new("text", "html")
	///     .with_param("charset", "utf-8");
	/// assert_eq!(media_type.parameters.get("charset"), Some(&"utf-8".to_string()));
	/// ```
	pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.parameters.insert(key.into(), value.into());
		self
	}
	/// Parse a content-type string into a MediaType.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::MediaType;
	///
	/// let media_type = MediaType::parse("application/json; charset=utf-8").unwrap();
	/// assert_eq!(media_type.main_type, "application");
	/// assert_eq!(media_type.sub_type, "json");
	/// assert_eq!(media_type.parameters.get("charset"), Some(&"utf-8".to_string()));
	/// ```
	pub fn parse(content_type: &str) -> ParseResult<Self> {
		let parts: Vec<&str> = content_type.split(';').collect();
		if parts.is_empty() {
			return Err(Error::Validation(content_type.to_string()));
		}

		let type_parts: Vec<&str> = parts[0].trim().split('/').collect();
		if type_parts.len() != 2 {
			return Err(Error::Validation(content_type.to_string()));
		}

		let mut media_type = MediaType::new(type_parts[0], type_parts[1]);

		// Parse parameters
		for part in parts.iter().skip(1) {
			let param_parts: Vec<&str> = part.trim().splitn(2, '=').collect();
			if param_parts.len() == 2 {
				media_type.parameters.insert(
					param_parts[0].trim().to_string(),
					param_parts[1].trim().to_string(),
				);
			}
		}

		Ok(media_type)
	}
	/// Check if this media type matches a pattern (supports wildcards).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::MediaType;
	///
	/// let media_type = MediaType::new("application", "json");
	/// assert!(media_type.matches("application/json"));
	/// assert!(media_type.matches("application/*"));
	/// assert!(media_type.matches("*/json"));
	/// assert!(media_type.matches("*/*"));
	/// assert!(!media_type.matches("text/html"));
	/// ```
	pub fn matches(&self, pattern: &str) -> bool {
		let parts: Vec<&str> = pattern.split('/').collect();
		if parts.len() != 2 {
			return false;
		}

		(parts[0] == "*" || parts[0] == self.main_type)
			&& (parts[1] == "*" || parts[1] == self.sub_type)
	}
}

impl std::fmt::Display for MediaType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}/{}", self.main_type, self.sub_type)?;
		for (key, value) in &self.parameters {
			write!(f, "; {}={}", key, value)?;
		}
		Ok(())
	}
}

/// Parsed data representation
#[derive(Debug, Clone)]
pub enum ParsedData {
	/// JSON-parsed data.
	Json(Value),
	/// XML-parsed data.
	Xml(Value),
	/// YAML-parsed data.
	Yaml(Value),
	/// URL-encoded form data.
	Form(HashMap<String, String>),
	/// Multipart form data with text fields and file uploads.
	MultiPart {
		/// Text field name-value pairs.
		fields: HashMap<String, String>,
		/// Uploaded file attachments.
		files: Vec<UploadedFile>,
	},
	/// A single uploaded file.
	File(UploadedFile),
	/// MessagePack-parsed data.
	MessagePack(Value),
	/// Protobuf-parsed data.
	Protobuf(Value),
}

/// Uploaded file representation
#[derive(Debug, Clone)]
pub struct UploadedFile {
	/// Form field name for this upload.
	pub name: String,
	/// Original filename from the client, if provided.
	pub filename: Option<String>,
	/// MIME content type of the uploaded file.
	pub content_type: Option<String>,
	/// Size of the file data in bytes.
	pub size: usize,
	/// Raw file data.
	pub data: Bytes,
}

impl UploadedFile {
	/// Create a new UploadedFile with the given name and data.
	///
	/// # Examples
	///
	/// ```
	/// use bytes::Bytes;
	/// use reinhardt_core::parsers::parser::UploadedFile;
	///
	/// let data = Bytes::from("file content");
	/// let file = UploadedFile::new("upload".to_string(), data.clone());
	/// assert_eq!(file.name, "upload");
	/// assert_eq!(file.size, data.len());
	/// assert!(file.filename.is_none());
	/// assert!(file.content_type.is_none());
	/// ```
	pub fn new(name: String, data: Bytes) -> Self {
		let size = data.len();
		Self {
			name,
			filename: None,
			content_type: None,
			size,
			data,
		}
	}
	/// Set the original filename for this upload.
	///
	/// # Examples
	///
	/// ```
	/// use bytes::Bytes;
	/// use reinhardt_core::parsers::parser::UploadedFile;
	///
	/// let file = UploadedFile::new("upload".to_string(), Bytes::from("content"))
	///     .with_filename("document.pdf".to_string());
	/// assert_eq!(file.filename, Some("document.pdf".to_string()));
	/// ```
	pub fn with_filename(mut self, filename: String) -> Self {
		self.filename = Some(filename);
		self
	}
	/// Set the content type for this upload.
	///
	/// # Examples
	///
	/// ```
	/// use bytes::Bytes;
	/// use reinhardt_core::parsers::parser::UploadedFile;
	///
	/// let file = UploadedFile::new("upload".to_string(), Bytes::from("content"))
	///     .with_content_type("application/pdf".to_string());
	/// assert_eq!(file.content_type, Some("application/pdf".to_string()));
	/// ```
	pub fn with_content_type(mut self, content_type: String) -> Self {
		self.content_type = Some(content_type);
		self
	}
}

/// Trait for request body parsers
#[async_trait]
pub trait Parser: Send + Sync {
	/// Get the media types this parser can handle
	fn media_types(&self) -> Vec<String>;

	/// Parse the request body
	async fn parse(
		&self,
		content_type: Option<&str>,
		body: Bytes,
		headers: &HeaderMap,
	) -> ParseResult<ParsedData>;

	/// Check if this parser can handle the given content type
	fn can_parse(&self, content_type: Option<&str>) -> bool {
		if let Some(ct) = content_type
			&& let Ok(media_type) = MediaType::parse(ct)
		{
			return self.media_types().iter().any(|mt| media_type.matches(mt));
		}
		false
	}
}

/// Parser registry for selecting appropriate parser
#[derive(Default)]
pub struct ParserRegistry {
	parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistry {
	/// Create a new empty parser registry.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::ParserRegistry;
	///
	/// let registry = ParserRegistry::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}
	/// Register a parser to handle specific content types.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::parser::ParserRegistry;
	/// use reinhardt_core::parsers::json::JSONParser;
	/// use reinhardt_core::parsers::form::FormParser;
	///
	/// let registry = ParserRegistry::new()
	///     .register(JSONParser::new())
	///     .register(FormParser::new());
	/// ```
	pub fn register<P: Parser + 'static>(mut self, parser: P) -> Self {
		self.parsers.push(Box::new(parser));
		self
	}
	/// Parse the request body using the first matching parser.
	///
	/// # Examples
	///
	/// ```
	/// use bytes::Bytes;
	/// use http::HeaderMap;
	/// use reinhardt_core::parsers::parser::{ParserRegistry, ParsedData};
	/// use reinhardt_core::parsers::json::JSONParser;
	///
	/// # tokio_test::block_on(async {
	/// let registry = ParserRegistry::new().register(JSONParser::new());
	/// let body = Bytes::from(r#"{"key":"value"}"#);
	/// let headers = HeaderMap::new();
	/// let result = registry.parse(Some("application/json"), body, &headers).await.unwrap();
	/// match result {
	///     ParsedData::Json(_) => {},
	///     _ => panic!("Expected JSON"),
	/// }
	/// # });
	/// ```
	pub async fn parse(
		&self,
		content_type: Option<&str>,
		body: Bytes,
		headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		for parser in &self.parsers {
			if parser.can_parse(content_type) {
				return parser.parse(content_type, body, headers).await;
			}
		}

		Err(Error::Validation(
			content_type.unwrap_or("none").to_string(),
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_media_type_parse() {
		let mt = MediaType::parse("application/json").unwrap();
		assert_eq!(mt.main_type, "application");
		assert_eq!(mt.sub_type, "json");

		let mt = MediaType::parse("text/html; charset=utf-8").unwrap();
		assert_eq!(mt.main_type, "text");
		assert_eq!(mt.sub_type, "html");
		assert_eq!(mt.parameters.get("charset"), Some(&"utf-8".to_string()));
	}

	#[test]
	fn test_media_type_matches() {
		let mt = MediaType::new("application", "json");
		assert!(mt.matches("application/json"));
		assert!(mt.matches("application/*"));
		assert!(mt.matches("*/json"));
		assert!(mt.matches("*/*"));
		assert!(!mt.matches("text/html"));
	}
}
