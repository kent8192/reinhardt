//! StreamField-style content blocks
//!
//! Polymorphic content blocks inspired by Wagtail's StreamField.
//! Blocks can be nested and combined to create flexible page layouts.

use crate::error::{CmsError, CmsResult};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Block type identifier
pub type BlockType = String;

/// Factory function that creates a block from JSON data
type BlockFactory = Box<dyn Fn(JsonValue) -> CmsResult<Box<dyn Block>>>;

/// Block trait for all content blocks
pub trait Block: Send + Sync {
	/// Get the block type identifier
	fn block_type(&self) -> BlockType;

	/// Render this block to HTML
	fn render(&self) -> CmsResult<String>;

	/// Serialize block data to JSON
	fn to_json(&self) -> CmsResult<JsonValue>;

	/// Deserialize block data from JSON
	fn from_json(value: JsonValue) -> CmsResult<Self>
	where
		Self: Sized;
}

/// StreamField containing a sequence of blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamField {
	/// Ordered list of blocks
	blocks: Vec<StreamBlock>,
}

/// A block instance in a StreamField
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamBlock {
	/// Block type
	pub block_type: BlockType,

	/// Block data (JSON)
	pub data: JsonValue,

	/// Optional block ID for editing
	pub id: Option<String>,
}

impl StreamField {
	/// Create a new empty StreamField
	pub fn new() -> Self {
		Self { blocks: Vec::new() }
	}

	/// Add a block to the field
	pub fn add_block(&mut self, block: StreamBlock) -> &mut Self {
		self.blocks.push(block);
		self
	}

	/// Get all blocks
	pub fn blocks(&self) -> &[StreamBlock] {
		&self.blocks
	}

	/// Render all blocks to HTML
	pub fn render(&self, registry: &BlockLibrary) -> CmsResult<String> {
		let mut html = String::new();
		for block in &self.blocks {
			let block_instance = registry.create_block(&block.block_type, block.data.clone())?;
			html.push_str(&block_instance.render()?);
		}
		Ok(html)
	}
}

impl Default for StreamField {
	fn default() -> Self {
		Self::new()
	}
}

/// Registry of available block types
pub struct BlockLibrary {
	blocks: HashMap<BlockType, BlockFactory>,
}

impl BlockLibrary {
	/// Create a new block library
	pub fn new() -> Self {
		Self {
			blocks: HashMap::new(),
		}
	}

	/// Register a block type
	pub fn register<F>(&mut self, block_type: BlockType, factory: F)
	where
		F: Fn(JsonValue) -> CmsResult<Box<dyn Block>> + 'static,
	{
		self.blocks.insert(block_type, Box::new(factory));
	}

	/// Create a block instance from JSON
	pub fn create_block(&self, block_type: &str, data: JsonValue) -> CmsResult<Box<dyn Block>> {
		let factory = self
			.blocks
			.get(block_type)
			.ok_or_else(|| CmsError::UnknownBlockType(block_type.to_string()))?;

		factory(data)
	}
}

impl Default for BlockLibrary {
	fn default() -> Self {
		Self::new()
	}
}
