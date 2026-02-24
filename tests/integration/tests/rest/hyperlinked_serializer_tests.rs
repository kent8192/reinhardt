//! Hyperlinked serializer tests
//!
//! Tests for `HyperlinkedModelSerializer` and `UrlReverser` from reinhardt-rest.

use reinhardt_rest::serializers::{HyperlinkedModelSerializer, Serializer, UrlReverser};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestModel {
	id: Option<i64>,
	name: String,
}

reinhardt_test::impl_test_model!(TestModel, i64, "test_models");

#[test]
fn test_hyperlinked_serializer_creation() {
	// Verify the serializer works and produces correct default URL field
	let serializer = HyperlinkedModelSerializer::<TestModel>::new("detail", None);
	let model = TestModel {
		id: Some(1),
		name: String::from("test"),
	};
	let result = serializer.serialize(&model).unwrap();
	let value: Value = serde_json::from_str(&result).unwrap();
	// Default url_field_name is "url"
	assert!(value.get("url").is_some());
	// View name "detail" is used in fallback URL
	assert!(value["url"].as_str().unwrap().contains("detail"));
}

#[test]
fn test_custom_url_field_name() {
	// Verify custom url_field_name via serialization output
	let serializer =
		HyperlinkedModelSerializer::<TestModel>::new("detail", None).url_field_name("self_link");
	let model = TestModel {
		id: Some(1),
		name: String::from("test"),
	};
	let result = serializer.serialize(&model).unwrap();
	let value: Value = serde_json::from_str(&result).unwrap();
	// Should use "self_link" instead of "url"
	assert!(value.get("self_link").is_some());
	assert!(value.get("url").is_none());
}

#[test]
fn test_serialize_with_url_fallback() {
	// Test fallback path-based URL generation (no reverser)
	let serializer = HyperlinkedModelSerializer::<TestModel>::new("detail", None);
	let model = TestModel {
		id: Some(42),
		name: String::from("test"),
	};

	let result = serializer.serialize(&model).unwrap();
	let value: Value = serde_json::from_str(&result).unwrap();

	assert_eq!(value["id"], 42);
	assert_eq!(value["name"], "test");
	assert_eq!(value["url"], "/test_models/detail/42");
}

#[test]
fn test_serialize_with_url_reverser() {
	// Test proper URL reversal using custom UrlReverser implementation
	struct TestUrlReverser;

	impl UrlReverser for TestUrlReverser {
		fn reverse(&self, _name: &str, params: &HashMap<String, String>) -> Result<String, String> {
			let id = params
				.get("id")
				.ok_or_else(|| "Missing id parameter".to_string())?;
			Ok(format!("/models/{}/", id))
		}
	}

	let reverser: Arc<dyn UrlReverser> = Arc::new(TestUrlReverser);
	let serializer = HyperlinkedModelSerializer::<TestModel>::new("detail", Some(reverser));
	let model = TestModel {
		id: Some(42),
		name: String::from("test"),
	};

	let result = serializer.serialize(&model).unwrap();
	let value: Value = serde_json::from_str(&result).unwrap();

	assert_eq!(value["id"], 42);
	assert_eq!(value["name"], "test");
	assert_eq!(value["url"], "/models/42/");
}

#[test]
fn test_serialize_without_pk_fails() {
	let serializer = HyperlinkedModelSerializer::<TestModel>::new("detail", None);
	let model = TestModel {
		id: None,
		name: String::from("test"),
	};

	let result = serializer.serialize(&model);
	assert!(result.is_err());
	assert!(result.unwrap_err().message().contains("no primary key"));
}

#[test]
fn test_deserialize() {
	let serializer = HyperlinkedModelSerializer::<TestModel>::new("detail", None);
	let json = r#"{"id":42,"name":"test","url":"/test_models/detail/42"}"#;

	let result = serializer.deserialize(&json.to_string()).unwrap();
	assert_eq!(result.id, Some(42));
	assert_eq!(result.name, "test");
}
