//! Tests for StreamField-style content blocks

use reinhardt_cms::blocks::*;
use reinhardt_cms::error::{CmsError, CmsResult};
use rstest::*;
use serde_json::{Value as JsonValue, json};

// --- Test Block Implementations ---

struct TextBlock {
	text: String,
}

impl Block for TextBlock {
	fn block_type(&self) -> BlockType {
		"text".to_string()
	}

	fn render(&self) -> CmsResult<String> {
		Ok(format!("<p>{}</p>", self.text))
	}

	fn to_json(&self) -> CmsResult<JsonValue> {
		Ok(json!({"text": self.text}))
	}

	fn from_json(value: JsonValue) -> CmsResult<Self> {
		let text = value
			.get("text")
			.and_then(|v| v.as_str())
			.ok_or_else(|| CmsError::Generic("Missing text field".to_string()))?;
		Ok(Self {
			text: text.to_string(),
		})
	}
}

struct HeadingBlock {
	text: String,
	level: u8,
}

impl Block for HeadingBlock {
	fn block_type(&self) -> BlockType {
		"heading".to_string()
	}

	fn render(&self) -> CmsResult<String> {
		Ok(format!(
			"<h{level}>{text}</h{level}>",
			level = self.level,
			text = self.text
		))
	}

	fn to_json(&self) -> CmsResult<JsonValue> {
		Ok(json!({"text": self.text, "level": self.level}))
	}

	fn from_json(value: JsonValue) -> CmsResult<Self> {
		let text = value
			.get("text")
			.and_then(|v| v.as_str())
			.ok_or_else(|| CmsError::Generic("Missing text field".to_string()))?;
		let level = value
			.get("level")
			.and_then(|v| v.as_u64())
			.ok_or_else(|| CmsError::Generic("Missing level field".to_string()))? as u8;
		Ok(Self {
			text: text.to_string(),
			level,
		})
	}
}

struct ImageBlock {
	src: String,
	alt: String,
}

impl Block for ImageBlock {
	fn block_type(&self) -> BlockType {
		"image".to_string()
	}

	fn render(&self) -> CmsResult<String> {
		Ok(format!("<img src=\"{}\" alt=\"{}\">", self.src, self.alt))
	}

	fn to_json(&self) -> CmsResult<JsonValue> {
		Ok(json!({"src": self.src, "alt": self.alt}))
	}

	fn from_json(value: JsonValue) -> CmsResult<Self> {
		let src = value
			.get("src")
			.and_then(|v| v.as_str())
			.ok_or_else(|| CmsError::Generic("Missing src field".to_string()))?;
		let alt = value
			.get("alt")
			.and_then(|v| v.as_str())
			.ok_or_else(|| CmsError::Generic("Missing alt field".to_string()))?;
		Ok(Self {
			src: src.to_string(),
			alt: alt.to_string(),
		})
	}
}

// --- Helper Functions ---

fn register_text_block(library: &mut BlockLibrary) {
	library.register("text".to_string(), |data| {
		let block = TextBlock::from_json(data)?;
		Ok(Box::new(block))
	});
}

fn register_heading_block(library: &mut BlockLibrary) {
	library.register("heading".to_string(), |data| {
		let block = HeadingBlock::from_json(data)?;
		Ok(Box::new(block))
	});
}

fn register_image_block(library: &mut BlockLibrary) {
	library.register("image".to_string(), |data| {
		let block = ImageBlock::from_json(data)?;
		Ok(Box::new(block))
	});
}

// =============================================================================
// Happy Path Tests
// =============================================================================

#[rstest]
fn test_streamfield_new_is_empty() {
	// Arrange
	// (no setup needed)

	// Act
	let field = StreamField::new();

	// Assert
	assert_eq!(field.blocks().len(), 0);
}

#[rstest]
fn test_streamfield_add_and_retrieve_block() {
	// Arrange
	let mut field = StreamField::new();
	let block = StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Hello"}),
		id: Some("block-1".to_string()),
	};

	// Act
	field.add_block(block);

	// Assert
	assert_eq!(field.blocks().len(), 1);
	assert_eq!(field.blocks()[0].block_type, "text");
	assert_eq!(field.blocks()[0].data, json!({"text": "Hello"}));
	assert_eq!(field.blocks()[0].id, Some("block-1".to_string()));
}

#[rstest]
fn test_streamfield_add_multiple_blocks_preserves_order() {
	// Arrange
	let mut field = StreamField::new();

	// Act
	field.add_block(StreamBlock {
		block_type: "heading".to_string(),
		data: json!({"text": "Title", "level": 1}),
		id: None,
	});
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Body"}),
		id: None,
	});
	field.add_block(StreamBlock {
		block_type: "image".to_string(),
		data: json!({"src": "img.png", "alt": "Photo"}),
		id: None,
	});

	// Assert
	assert_eq!(field.blocks().len(), 3);
	assert_eq!(field.blocks()[0].block_type, "heading");
	assert_eq!(field.blocks()[1].block_type, "text");
	assert_eq!(field.blocks()[2].block_type, "image");
}

#[rstest]
fn test_block_library_register_and_create() {
	// Arrange
	let mut library = BlockLibrary::new();
	register_text_block(&mut library);

	// Act
	let block = library
		.create_block("text", json!({"text": "Hello world"}))
		.unwrap();
	let html = block.render().unwrap();

	// Assert
	assert_eq!(html, "<p>Hello world</p>");
	assert_eq!(block.block_type(), "text");
}

#[rstest]
fn test_streamfield_render_concatenates_html() {
	// Arrange
	let mut library = BlockLibrary::new();
	register_text_block(&mut library);

	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "First"}),
		id: None,
	});
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Second"}),
		id: None,
	});

	// Act
	let html = field.render(&library).unwrap();

	// Assert
	assert_eq!(html, "<p>First</p><p>Second</p>");
}

// =============================================================================
// Error Path Tests
// =============================================================================

#[rstest]
fn test_create_block_unregistered_type() {
	// Arrange
	let library = BlockLibrary::new();

	// Act
	let result = library.create_block("nonexistent", json!({"text": "hello"}));

	// Assert
	let err = result.err().expect("Expected UnknownBlockType error");
	assert!(matches!(err, CmsError::UnknownBlockType(_)));
}

#[rstest]
fn test_streamfield_render_with_unregistered_block() {
	// Arrange
	let library = BlockLibrary::new();
	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "unknown_type".to_string(),
		data: json!({"text": "hello"}),
		id: None,
	});

	// Act
	let result = field.render(&library);

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::UnknownBlockType(_)));
}

#[rstest]
fn test_block_factory_returns_error_propagates() {
	// Arrange
	let mut library = BlockLibrary::new();
	library.register("failing".to_string(), |_data| {
		Err(CmsError::Generic("Factory error".to_string()))
	});

	// Act
	let result = library.create_block("failing", json!(null));

	// Assert
	let err = result.err().expect("Expected Generic error from factory");
	assert!(matches!(err, CmsError::Generic(_)));
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[rstest]
fn test_streamfield_render_empty_field() {
	// Arrange
	let library = BlockLibrary::new();
	let field = StreamField::new();

	// Act
	let html = field.render(&library).unwrap();

	// Assert
	assert_eq!(html, "");
}

#[rstest]
fn test_block_library_register_overwrites_existing() {
	// Arrange
	let mut library = BlockLibrary::new();
	// First registration: standard TextBlock
	register_text_block(&mut library);
	// Second registration: always renders with "overwritten" text
	library.register("text".to_string(), |_data| {
		Ok(Box::new(TextBlock {
			text: "overwritten".to_string(),
		}))
	});

	// Act
	let block = library
		.create_block("text", json!({"text": "original"}))
		.unwrap();
	let html = block.render().unwrap();

	// Assert
	assert_eq!(html, "<p>overwritten</p>");
}

#[rstest]
fn test_streamfield_default_trait() {
	// Arrange
	// (no setup needed)

	// Act
	let field = StreamField::default();

	// Assert
	assert_eq!(field.blocks().len(), 0);
}

// =============================================================================
// Sanity Tests
// =============================================================================

#[rstest]
fn test_block_library_default_trait() {
	// Arrange
	let library = BlockLibrary::default();

	// Act
	let result = library.create_block("any_type", json!(null));

	// Assert
	let err = result.err().expect("Expected UnknownBlockType error");
	assert!(matches!(err, CmsError::UnknownBlockType(_)));
}

#[rstest]
fn test_stream_block_serialization_roundtrip() {
	// Arrange
	let block = StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Hello world", "nested": {"key": "value"}}),
		id: Some("unique-id-123".to_string()),
	};

	// Act
	let json_str = serde_json::to_string(&block).unwrap();
	let deserialized: StreamBlock = serde_json::from_str(&json_str).unwrap();

	// Assert
	assert_eq!(deserialized.block_type, "text");
	assert_eq!(
		deserialized.data,
		json!({"text": "Hello world", "nested": {"key": "value"}})
	);
	assert_eq!(deserialized.id, Some("unique-id-123".to_string()));
}

// =============================================================================
// Equivalence Partitioning Tests
// =============================================================================

#[rstest]
#[case::snake_case("snake_case")]
#[case::pascal_case("PascalCase")]
#[case::kebab_case("kebab-case")]
#[case::dotted_name("dotted.name")]
fn test_streamfield_block_type_names(#[case] type_name: &str) {
	// Arrange
	let mut field = StreamField::new();

	// Act
	field.add_block(StreamBlock {
		block_type: type_name.to_string(),
		data: json!(null),
		id: None,
	});

	// Assert
	assert_eq!(field.blocks()[0].block_type, type_name);
}

#[rstest]
#[case::null_data(json!(null))]
#[case::string_data(json!("hello"))]
#[case::number_data(json!(42))]
#[case::object_data(json!({"key": "value"}))]
#[case::array_data(json!([1, 2, 3]))]
fn test_streamfield_add_block_with_various_json_data(#[case] data: serde_json::Value) {
	// Arrange
	let mut field = StreamField::new();
	let expected_data = data.clone();

	// Act
	field.add_block(StreamBlock {
		block_type: "generic".to_string(),
		data,
		id: None,
	});

	// Assert
	assert_eq!(field.blocks()[0].data, expected_data);
}

// =============================================================================
// Boundary Value Tests
// =============================================================================

#[rstest]
#[case::zero_blocks(0)]
#[case::one_block(1)]
#[case::ten_blocks(10)]
#[case::hundred_blocks(100)]
fn test_streamfield_block_count_boundaries(#[case] count: usize) {
	// Arrange
	let mut field = StreamField::new();

	// Act
	for i in 0..count {
		field.add_block(StreamBlock {
			block_type: "text".to_string(),
			data: json!({"text": format!("Block {}", i)}),
			id: None,
		});
	}

	// Assert
	assert_eq!(field.blocks().len(), count);
}

#[rstest]
#[case::empty_name("".to_string())]
#[case::single_char("a".to_string())]
#[case::medium_length("a".repeat(255))]
#[case::long_name("a".repeat(1000))]
fn test_block_type_name_length_boundaries(#[case] type_name: String) {
	// Arrange
	let mut library = BlockLibrary::new();
	let expected_name = type_name.clone();
	library.register(type_name, |_data| {
		Ok(Box::new(TextBlock {
			text: "test".to_string(),
		}))
	});

	// Act
	let block = library.create_block(&expected_name, json!(null)).unwrap();

	// Assert
	assert_eq!(block.render().unwrap(), "<p>test</p>");
}

// =============================================================================
// Combination Tests
// =============================================================================

#[rstest]
fn test_streamfield_with_block_library_end_to_end() {
	// Arrange
	let mut library = BlockLibrary::new();
	register_heading_block(&mut library);
	register_text_block(&mut library);

	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "heading".to_string(),
		data: json!({"text": "Welcome", "level": 1}),
		id: None,
	});
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Hello world"}),
		id: None,
	});

	// Act
	let html = field.render(&library).unwrap();

	// Assert
	assert_eq!(html, "<h1>Welcome</h1><p>Hello world</p>");
}

#[rstest]
fn test_streamfield_chaining_api() {
	// Arrange
	let mut field = StreamField::new();

	// Act
	let result = field
		.add_block(StreamBlock {
			block_type: "text".to_string(),
			data: json!({"text": "First"}),
			id: None,
		})
		.add_block(StreamBlock {
			block_type: "text".to_string(),
			data: json!({"text": "Second"}),
			id: None,
		});

	// Assert
	assert_eq!(result.blocks().len(), 2);
	assert_eq!(result.blocks()[0].data, json!({"text": "First"}));
	assert_eq!(result.blocks()[1].data, json!({"text": "Second"}));
}

// =============================================================================
// Use Case Tests
// =============================================================================

#[rstest]
fn test_blog_content_streamfield() {
	// Arrange
	let mut library = BlockLibrary::new();
	register_heading_block(&mut library);
	register_text_block(&mut library);
	register_image_block(&mut library);

	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "heading".to_string(),
		data: json!({"text": "My Blog Post", "level": 1}),
		id: Some("heading-1".to_string()),
	});
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "This is the introduction."}),
		id: Some("text-1".to_string()),
	});
	field.add_block(StreamBlock {
		block_type: "image".to_string(),
		data: json!({"src": "/images/photo.jpg", "alt": "A beautiful photo"}),
		id: Some("image-1".to_string()),
	});

	// Act
	let html = field.render(&library).unwrap();

	// Assert
	assert_eq!(
		html,
		"<h1>My Blog Post</h1>\
		 <p>This is the introduction.</p>\
		 <img src=\"/images/photo.jpg\" alt=\"A beautiful photo\">"
	);
}

#[rstest]
fn test_streamfield_serialization_roundtrip() {
	// Arrange
	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Hello"}),
		id: Some("block-1".to_string()),
	});
	field.add_block(StreamBlock {
		block_type: "heading".to_string(),
		data: json!({"text": "Title", "level": 2}),
		id: None,
	});

	// Act
	let json_str = serde_json::to_string(&field).unwrap();
	let deserialized: StreamField = serde_json::from_str(&json_str).unwrap();

	// Assert
	assert_eq!(deserialized.blocks().len(), 2);
	assert_eq!(deserialized.blocks()[0].block_type, "text");
	assert_eq!(deserialized.blocks()[0].data, json!({"text": "Hello"}));
	assert_eq!(deserialized.blocks()[0].id, Some("block-1".to_string()));
	assert_eq!(deserialized.blocks()[1].block_type, "heading");
	assert_eq!(
		deserialized.blocks()[1].data,
		json!({"text": "Title", "level": 2})
	);
	assert_eq!(deserialized.blocks()[1].id, None);
}

// =============================================================================
// Decision Table Tests
// =============================================================================

#[rstest]
#[case::registered_valid_data("text", json!({"text": "hello"}), true)]
#[case::registered_invalid_data("text", json!(42), false)]
#[case::unregistered_valid_data("nonexistent", json!({"text": "hello"}), false)]
#[case::unregistered_invalid_data("nonexistent", json!(42), false)]
fn test_create_block_registration_status_decision_table(
	#[case] block_type: &str,
	#[case] data: serde_json::Value,
	#[case] expect_success: bool,
) {
	// Arrange
	let mut library = BlockLibrary::new();
	register_text_block(&mut library);

	// Act
	let result = library.create_block(block_type, data);

	// Assert
	assert_eq!(result.is_ok(), expect_success);
}

#[rstest]
#[case::empty_field(vec![], true)]
#[case::single_registered_block(vec!["text"], true)]
#[case::multiple_registered_blocks(vec!["text", "heading"], true)]
#[case::single_unregistered_block(vec!["unknown"], false)]
#[case::mixed_with_unregistered(vec!["text", "unknown"], false)]
fn test_streamfield_render_block_validity_decision_table(
	#[case] block_types: Vec<&str>,
	#[case] expect_success: bool,
) {
	// Arrange
	let mut library = BlockLibrary::new();
	register_text_block(&mut library);
	register_heading_block(&mut library);

	let mut field = StreamField::new();
	for block_type in &block_types {
		let data = match *block_type {
			"heading" => json!({"text": "Title", "level": 1}),
			_ => json!({"text": "Content"}),
		};
		field.add_block(StreamBlock {
			block_type: block_type.to_string(),
			data,
			id: None,
		});
	}

	// Act
	let result = field.render(&library);

	// Assert
	assert_eq!(result.is_ok(), expect_success);
}
