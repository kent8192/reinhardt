//! # Reinhardt Parsers
//!
//! Request body parsers for the Reinhardt framework, inspired by Django REST Framework.
//!
//! ## Parsers
//!
//! - **JSONParser**: Parse JSON request bodies
//! - **XMLParser**: Parse XML request bodies (application/xml, text/xml)
//! - **YamlParser**: Parse YAML request bodies (application/yaml, application/x-yaml)
//! - **FormParser**: Parse HTML form data (application/x-www-form-urlencoded)
//! - **MultiPartParser**: Handle file uploads (multipart/form-data)
//! - **FileUploadParser**: Raw file upload handling
//! - **CompressedParser**: Wrapper for transparent decompression (gzip, brotli, deflate)
//! - **MessagePackParser**: Parse MessagePack binary format (application/msgpack)
//! - **ProtobufParser**: Parse Protocol Buffers with dynamic schema support (application/protobuf)
//! - **StreamingParser**: Memory-efficient parsing for large uploads
//!
//! ## Validation
//!
//! - **ParserValidator**: Trait for custom validation hooks (before_parse, after_parse)
//! - **SizeLimitValidator**: Enforce maximum body size limits
//! - **ContentTypeValidator**: Validate required content types
//! - **CompositeValidator**: Combine multiple validators
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_core::parsers::{JSONParser, Parser};
//! use bytes::Bytes;
//! use http::HeaderMap;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = JSONParser::new();
//! let body = Bytes::from(r#"{"key":"value"}"#);
//! let headers = HeaderMap::new();
//! let data = parser.parse(Some("application/json"), body, &headers).await?;
//! # Ok(())
//! # }
//! ```

/// Compressed request body parser (gzip, deflate, brotli).
pub mod compressed;
/// File upload parser.
pub mod file;
/// URL-encoded form data parser.
pub mod form;
/// JSON request body parser.
pub mod json;
/// MessagePack binary format parser.
pub mod msgpack;
/// Multipart form data parser.
pub mod multipart;
/// Base parser trait definition.
pub mod parser;
/// Protocol Buffers parser.
pub mod protobuf;
/// Streaming request body parser.
pub mod streaming;
/// Parser input validation.
pub mod validator;
/// XML request body parser.
#[cfg(feature = "xml")]
pub mod xml;
/// YAML request body parser.
#[cfg(feature = "yaml")]
pub mod yaml;

pub use compressed::{CompressedParser, CompressionEncoding};
pub use file::FileUploadParser;
pub use form::FormParser;
pub use json::JSONParser;
pub use msgpack::MessagePackParser;
pub use multipart::MultiPartParser;
pub use parser::{MediaType, ParseError, ParseResult, Parser};
pub use protobuf::{ProtobufMessage, ProtobufParser};
pub use streaming::{StreamChunk, StreamingParser};
pub use validator::{
	CompositeValidator, ContentTypeValidator, ParserValidator, SizeLimitValidator,
};
#[cfg(feature = "xml")]
pub use xml::{XMLParser, XmlParserConfig, XmlParserConfigBuilder};
#[cfg(feature = "yaml")]
pub use yaml::YamlParser;
