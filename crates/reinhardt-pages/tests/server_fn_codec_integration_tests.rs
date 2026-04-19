#![cfg(not(target_arch = "wasm32"))]
//! Integration tests for Server Functions Codec System
//!
//! These tests verify the integration of different codecs with server functions:
//! 1. JSON codec (default) - human-readable format
//! 2. URL codec - for GET requests with query parameters
//! 3. MessagePack codec (optional) - binary format for efficiency
//!
//! Test Strategy:
//! - Test codec interoperability with complex types
//! - Verify round-trip encoding/decoding
//! - Test error handling for malformed data
//! - Validate Content-Type headers

use reinhardt_pages::server_fn::codec::{Codec, JsonCodec, UrlCodec};
use serde::{Deserialize, Serialize};

#[cfg(feature = "msgpack")]
use reinhardt_pages::server_fn::codec::MessagePackCodec;

/// Complex nested structure for integration testing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserProfile {
	id: u32,
	name: String,
	email: String,
	settings: UserSettings,
	tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserSettings {
	theme: String,
	language: String,
	notifications: bool,
}

/// Success Criterion 1: JSON codec with complex nested structures
#[test]
fn test_json_codec_complex_structure() {
	let codec = JsonCodec;

	let profile = UserProfile {
		id: 42,
		name: "Alice".to_string(),
		email: "alice@example.com".to_string(),
		settings: UserSettings {
			theme: "dark".to_string(),
			language: "en".to_string(),
			notifications: true,
		},
		tags: vec!["admin".to_string(), "developer".to_string()],
	};

	// Encode
	let encoded = codec.encode(&profile).expect("JSON encoding failed");

	// Verify it's valid JSON
	let json_str = String::from_utf8(encoded.clone()).unwrap();
	assert!(json_str.contains("\"id\":42"));
	assert!(json_str.contains("\"name\":\"Alice\""));
	assert!(json_str.contains("\"theme\":\"dark\""));

	// Decode
	let decoded: UserProfile = codec.decode(&encoded).expect("JSON decoding failed");

	// Verify round-trip
	assert_eq!(decoded, profile);
}

/// Success Criterion 2: URL codec with simple structures (GET request simulation)
#[test]
fn test_url_codec_get_request_params() {
	let codec = UrlCodec;

	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct SearchParams {
		query: String,
		page: u32,
		limit: u32,
		sort: String,
	}

	let params = SearchParams {
		query: "rust wasm".to_string(),
		page: 2,
		limit: 20,
		sort: "relevance".to_string(),
	};

	// Encode
	let encoded = codec.encode(&params).expect("URL encoding failed");
	let url_str = String::from_utf8(encoded.clone()).unwrap();

	// Verify URL encoding format
	assert!(url_str.contains("query=rust+wasm") || url_str.contains("query=rust%20wasm"));
	assert!(url_str.contains("page=2"));
	assert!(url_str.contains("limit=20"));
	assert!(url_str.contains("sort=relevance"));

	// Decode
	let decoded: SearchParams = codec.decode(&encoded).expect("URL decoding failed");

	// Verify round-trip
	assert_eq!(decoded, params);
}

/// Success Criterion 3: MessagePack codec efficiency (binary format)
#[cfg(feature = "msgpack")]
#[test]
fn test_msgpack_codec_efficiency() {
	let json_codec = JsonCodec;
	let msgpack_codec = MessagePackCodec;

	let profile = UserProfile {
		id: 42,
		name: "Alice".to_string(),
		email: "alice@example.com".to_string(),
		settings: UserSettings {
			theme: "dark".to_string(),
			language: "en".to_string(),
			notifications: true,
		},
		tags: vec!["admin".to_string(), "developer".to_string()],
	};

	// Encode with both codecs
	let json_encoded = json_codec.encode(&profile).expect("JSON encoding failed");
	let msgpack_encoded = msgpack_codec
		.encode(&profile)
		.expect("MessagePack encoding failed");

	// MessagePack should be more compact than JSON
	assert!(
		msgpack_encoded.len() < json_encoded.len(),
		"MessagePack size: {}, JSON size: {}",
		msgpack_encoded.len(),
		json_encoded.len()
	);

	// Verify round-trip for MessagePack
	let decoded: UserProfile = msgpack_codec
		.decode(&msgpack_encoded)
		.expect("MessagePack decoding failed");
	assert_eq!(decoded, profile);
}

/// Success Criterion 4: Content-Type headers are correct
#[test]
fn test_codec_content_types() {
	let json_codec = JsonCodec;
	let url_codec = UrlCodec;

	assert_eq!(json_codec.content_type(), "application/json");
	assert_eq!(
		url_codec.content_type(),
		"application/x-www-form-urlencoded"
	);

	#[cfg(feature = "msgpack")]
	{
		let msgpack_codec = MessagePackCodec;
		assert_eq!(msgpack_codec.content_type(), "application/msgpack");
	}
}

/// Success Criterion 5: Error handling for malformed data
#[test]
fn test_codec_error_handling() {
	let json_codec = JsonCodec;

	// Invalid JSON data
	let invalid_json = b"{ invalid json data }";
	let result: Result<UserProfile, _> = json_codec.decode(invalid_json);

	assert!(result.is_err());
	let err_msg = result.unwrap_err();
	assert!(err_msg.contains("JSON decoding failed"));
}

/// Integration test: Codec interoperability (encode with one, decode with same)
#[test]
fn test_codec_round_trip_guarantees() {
	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct TestData {
		field1: String,
		field2: i32,
		field3: Vec<String>,
	}

	let data = TestData {
		field1: "test".to_string(),
		field2: 42,
		field3: vec!["a".to_string(), "b".to_string()],
	};

	// JSON round-trip
	let json_codec = JsonCodec;
	let json_encoded = json_codec.encode(&data).unwrap();
	let json_decoded: TestData = json_codec.decode(&json_encoded).unwrap();
	assert_eq!(json_decoded, data);

	// URL round-trip (simple structure only)
	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct SimpleData {
		a: String,
		b: i32,
	}

	let simple_data = SimpleData {
		a: "test".to_string(),
		b: 42,
	};

	let url_codec = UrlCodec;
	let url_encoded = url_codec.encode(&simple_data).unwrap();
	let url_decoded: SimpleData = url_codec.decode(&url_encoded).unwrap();
	assert_eq!(url_decoded, simple_data);

	// MessagePack round-trip
	#[cfg(feature = "msgpack")]
	{
		let msgpack_codec = MessagePackCodec;
		let msgpack_encoded = msgpack_codec.encode(&data).unwrap();
		let msgpack_decoded: TestData = msgpack_codec.decode(&msgpack_encoded).unwrap();
		assert_eq!(msgpack_decoded, data);
	}
}

/// Integration test: Codec name identification
#[test]
fn test_codec_names() {
	let json_codec = JsonCodec;
	let url_codec = UrlCodec;

	assert_eq!(json_codec.name(), "json");
	assert_eq!(url_codec.name(), "url");

	#[cfg(feature = "msgpack")]
	{
		let msgpack_codec = MessagePackCodec;
		assert_eq!(msgpack_codec.name(), "msgpack");
	}
}
