//! Property-based tests for StreamField content blocks

use proptest::prelude::*;
use reinhardt_cms::blocks::*;
use reinhardt_cms::error::CmsResult;
use serde_json::{Value as JsonValue, json};

// Minimal block implementation for property tests
struct SimpleTestBlock;

impl Block for SimpleTestBlock {
	fn block_type(&self) -> BlockType {
		"simple".to_string()
	}

	fn render(&self) -> CmsResult<String> {
		Ok("<div>simple</div>".to_string())
	}

	fn to_json(&self) -> CmsResult<JsonValue> {
		Ok(json!(null))
	}

	fn from_json(_value: JsonValue) -> CmsResult<Self> {
		Ok(Self)
	}
}

proptest! {
	#[test]
	fn prop_streamfield_block_count_matches_additions(count in 0usize..50) {
		// Arrange
		let mut field = StreamField::new();

		// Act
		for i in 0..count {
			field.add_block(StreamBlock {
				block_type: format!("type_{}", i),
				data: json!(null),
				id: None,
			});
		}

		// Assert
		prop_assert_eq!(field.blocks().len(), count);
	}

	#[test]
	fn fuzz_streamfield_add_random_blocks(
		block_type in "[a-z_]{1,50}",
		id in proptest::option::of("[a-z0-9-]{1,36}"),
	) {
		// Arrange
		let mut field = StreamField::new();

		// Act
		field.add_block(StreamBlock {
			block_type: block_type.clone(),
			data: json!({"text": "test"}),
			id: id.clone(),
		});

		// Assert
		prop_assert_eq!(field.blocks().len(), 1);
		prop_assert_eq!(&field.blocks()[0].block_type, &block_type);
		prop_assert_eq!(&field.blocks()[0].id, &id);
	}

	#[test]
	fn fuzz_block_library_random_type_names(
		type_name in "[a-zA-Z_][a-zA-Z0-9_.-]{0,100}",
		query_name in "[a-zA-Z_][a-zA-Z0-9_.-]{0,100}",
	) {
		// Arrange
		let mut library = BlockLibrary::new();
		library.register(type_name.clone(), |_data| {
			Ok(Box::new(SimpleTestBlock))
		});

		// Act
		let result = library.create_block(&query_name, json!(null));

		// Assert
		if type_name == query_name {
			prop_assert!(result.is_ok(), "Expected Ok for matching type '{}' but got Err", type_name);
		} else {
			prop_assert!(result.is_err(), "Expected Err for mismatched types '{}' != '{}'", type_name, query_name);
		}
	}
}
