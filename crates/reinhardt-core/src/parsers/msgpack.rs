//! MessagePack parser for binary message format.
//!
//! This parser handles `application/msgpack` and `application/x-msgpack` content types,
//! deserializing MessagePack binary data into structured JSON values.

use crate::exception::Error;
use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use serde_json::Value;

use super::parser::{ParseResult, ParsedData, Parser};

/// Parser for MessagePack binary format.
///
/// MessagePack is an efficient binary serialization format that is more compact
/// than JSON while maintaining similar data structures.
///
/// # Supported Content Types
///
/// - `application/msgpack`
/// - `application/x-msgpack`
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::msgpack::MessagePackParser;
/// use reinhardt_core::parsers::parser::Parser;
/// use bytes::Bytes;
/// use http::HeaderMap;
///
/// # tokio_test::block_on(async {
/// let parser = MessagePackParser::new();
///
/// // Example MessagePack data (serialized from {"key": "value"})
/// let msgpack_data = vec![0x81, 0xa3, 0x6b, 0x65, 0x79, 0xa5, 0x76, 0x61, 0x6c, 0x75, 0x65];
/// let body = Bytes::from(msgpack_data);
/// let headers = HeaderMap::new();
///
/// let result = parser.parse(Some("application/msgpack"), body, &headers).await;
/// assert!(result.is_ok());
/// # });
/// ```
#[derive(Debug, Clone, Default)]
pub struct MessagePackParser;

impl MessagePackParser {
	/// Create a new MessagePackParser instance.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::msgpack::MessagePackParser;
	///
	/// let parser = MessagePackParser::new();
	/// ```
	pub fn new() -> Self {
		Self
	}
}

#[async_trait]
impl Parser for MessagePackParser {
	fn media_types(&self) -> Vec<String> {
		vec![
			"application/msgpack".to_string(),
			"application/x-msgpack".to_string(),
		]
	}

	async fn parse(
		&self,
		_content_type: Option<&str>,
		body: Bytes,
		_headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		let value: Value = rmp_serde::from_slice(&body)
			.map_err(|e| Error::Validation(format!("Failed to parse MessagePack data: {}", e)))?;

		Ok(ParsedData::MessagePack(value))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[tokio::test]
	async fn test_msgpack_parser_media_types() {
		let parser = MessagePackParser::new();
		let media_types = parser.media_types();

		assert_eq!(media_types.len(), 2);
		assert!(media_types.contains(&"application/msgpack".to_string()));
		assert!(media_types.contains(&"application/x-msgpack".to_string()));
	}

	#[tokio::test]
	async fn test_msgpack_parser_can_parse() {
		let parser = MessagePackParser::new();

		assert!(parser.can_parse(Some("application/msgpack")));
		assert!(parser.can_parse(Some("application/x-msgpack")));
		assert!(!parser.can_parse(Some("application/json")));
		assert!(!parser.can_parse(None));
	}

	#[tokio::test]
	async fn test_msgpack_parser_simple_object() {
		let parser = MessagePackParser::new();

		// Serialize {"key": "value"} to MessagePack
		let data = json!({"key": "value"});
		let msgpack_bytes = rmp_serde::to_vec(&data).unwrap();
		let body = Bytes::from(msgpack_bytes);
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/msgpack"), body, &headers)
			.await;
		assert!(result.is_ok());

		match result.unwrap() {
			ParsedData::MessagePack(value) => {
				assert_eq!(value["key"], "value");
			}
			_ => panic!("Expected MessagePack variant"),
		}
	}

	#[tokio::test]
	async fn test_msgpack_parser_nested_object() {
		#[derive(serde::Serialize)]
		struct User {
			name: &'static str,
			age: u8,
			active: bool,
		}

		#[derive(serde::Serialize)]
		struct Payload {
			user: User,
		}

		let parser = MessagePackParser::new();

		// Serialize nested object to MessagePack
		let msgpack_bytes = rmp_serde::to_vec_named(&Payload {
			user: User {
				name: "John Doe",
				age: 30,
				active: true,
			},
		})
		.unwrap();
		let body = Bytes::from(msgpack_bytes);
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/msgpack"), body, &headers)
			.await;
		assert!(result.is_ok());

		match result.unwrap() {
			ParsedData::MessagePack(value) => {
				assert_eq!(value["user"]["name"], "John Doe");
				assert_eq!(value["user"]["age"], 30);
				assert!(value["user"]["active"].as_bool().unwrap());
			}
			_ => panic!("Expected MessagePack variant"),
		}
	}

	#[tokio::test]
	async fn test_msgpack_parser_array() {
		let parser = MessagePackParser::new();

		// Serialize array to MessagePack
		let msgpack_bytes = rmp_serde::to_vec(&[1, 2, 3, 4, 5]).unwrap();
		let body = Bytes::from(msgpack_bytes);
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/msgpack"), body, &headers)
			.await;
		assert!(result.is_ok());

		match result.unwrap() {
			ParsedData::MessagePack(value) => {
				assert!(value.is_array());
				assert_eq!(value.as_array().unwrap().len(), 5);
				assert_eq!(value[0], 1);
				assert_eq!(value[4], 5);
			}
			_ => panic!("Expected MessagePack variant"),
		}
	}

	#[tokio::test]
	async fn test_msgpack_parser_invalid_data() {
		let parser = MessagePackParser::new();

		// Invalid MessagePack data - incomplete string marker without data
		let body = Bytes::from(vec![0xDA, 0xFF, 0xFF]); // fixstr marker followed by incomplete length
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/msgpack"), body, &headers)
			.await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_msgpack_parser_empty_body() {
		let parser = MessagePackParser::new();

		let body = Bytes::new();
		let headers = HeaderMap::new();
		let result = parser
			.parse(Some("application/msgpack"), body, &headers)
			.await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_msgpack_parser_with_x_msgpack_content_type() {
		let parser = MessagePackParser::new();

		let data = json!({"test": "data"});
		let msgpack_bytes = rmp_serde::to_vec(&data).unwrap();
		let body = Bytes::from(msgpack_bytes);
		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/x-msgpack"), body, &headers)
			.await;
		assert!(result.is_ok());

		match result.unwrap() {
			ParsedData::MessagePack(value) => {
				assert_eq!(value["test"], "data");
			}
			_ => panic!("Expected MessagePack variant"),
		}
	}
}
