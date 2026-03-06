//! TaggedItem model fixtures for reinhardt-taggit tests
//!
//! Provides reusable fixtures for generating TaggedItem test data,
//! including database helper functions using SeaQuery.

use chrono::Utc;
use reinhardt_taggit::TaggedItem;
use sea_query::{Alias, PostgresQueryBuilder, Query};
use sqlx::Row;

/// Default TaggedItem: tag_id=1, content_type="Food", object_id=42
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::default_tagged_item;
///
/// #[rstest]
/// fn test_tagged_item_fixture(default_tagged_item: TaggedItem) {
///     assert_eq!(default_tagged_item.tag_id, 1);
///     assert_eq!(default_tagged_item.content_type, "Food");
///     assert_eq!(default_tagged_item.object_id, 42);
/// }
/// ```
pub fn default_tagged_item() -> TaggedItem {
	TaggedItem::new(1, "Food", 42)
}

/// Custom TaggedItem with specified parameters
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::custom_tagged_item;
///
/// #[rstest]
/// fn test_custom_tagged_item(
///     #[with(2)] tag_id: i64,
///     #[with("Recipe")] content_type: &str,
///     #[with(100)] object_id: i64,
///     #[default(1)] custom_tagged_item: TaggedItem
/// ) {
///     assert_eq!(custom_tagged_item.tag_id, 2);
///     assert_eq!(custom_tagged_item.content_type, "Recipe");
///     assert_eq!(custom_tagged_item.object_id, 100);
/// }
/// ```
pub fn custom_tagged_item(tag_id: i64, content_type: &str, object_id: i64) -> TaggedItem {
	TaggedItem::new(tag_id, content_type, object_id)
}

/// Multiple TaggedItems for the same object
///
/// Returns three TaggedItems with different tag_ids but the same
/// content_type and object_id. Used for testing multiple tags on
/// a single object.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::tagged_item_list;
///
/// #[rstest]
/// fn test_tagged_item_list(tagged_item_list: Vec<TaggedItem>) {
///     assert_eq!(tagged_item_list.len(), 3);
///     // All have same content_type and object_id
///     for item in &tagged_item_list {
///         assert_eq!(item.content_type, "Food");
///         assert_eq!(item.object_id, 42);
///     }
/// }
/// ```
pub fn tagged_item_list() -> Vec<TaggedItem> {
	vec![
		TaggedItem::new(1, "Food", 42),
		TaggedItem::new(2, "Food", 42),
		TaggedItem::new(3, "Food", 42),
	]
}

/// Tag and TaggedItem linked together
///
/// Returns a tuple of (tag_id, TaggedItem) where the TaggedItem
/// references a Tag with the given ID.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::tag_and_tagged_item;
///
/// #[rstest]
/// fn test_tag_and_tagged_item(tag_and_tagged_item: (i64, TaggedItem)) {
///     let (tag_id, item) = tag_and_tagged_item;
///     assert_eq!(item.tag_id, tag_id);
/// }
/// ```
pub fn tag_and_tagged_item() -> (i64, TaggedItem) {
	let tag_id = 1;
	let item = TaggedItem::new(tag_id, "Food", 42);
	(tag_id, item)
}

/// Insert a tagged item into the database and return its generated id
///
/// Uses SeaQuery to construct the INSERT statement.
/// Requires an existing tag with the given `tag_id` (FK constraint).
pub async fn insert_tagged_item_to_db(
	pool: &sqlx::PgPool,
	tag_id: i64,
	content_type: &str,
	object_id: i64,
) -> i64 {
	let now = Utc::now().to_rfc3339();
	let sql = Query::insert()
		.into_table(Alias::new("tagged_items"))
		.columns([
			Alias::new("tag_id"),
			Alias::new("content_type"),
			Alias::new("object_id"),
			Alias::new("created_at"),
		])
		.values_panic([
			tag_id.into(),
			content_type.into(),
			object_id.into(),
			now.into(),
		])
		.returning_col(Alias::new("id"))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&sql)
		.fetch_one(pool)
		.await
		.expect("Failed to insert tagged item");

	row.get("id")
}

/// Builder for creating custom TaggedItem instances
///
/// Provides a fluent API for building TaggedItem instances with custom values.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::TaggedItemBuilder;
///
/// let item = TaggedItemBuilder::new()
///     .tag_id(5)
///     .content_type("Recipe")
///     .object_id(100)
///     .build();
///
/// assert_eq!(item.tag_id, 5);
/// assert_eq!(item.content_type, "Recipe");
/// assert_eq!(item.object_id, 100);
/// ```
pub struct TaggedItemBuilder {
	tag_id: i64,
	content_type: Option<String>,
	object_id: i64,
}

impl TaggedItemBuilder {
	/// Create a new TaggedItemBuilder
	pub fn new() -> Self {
		Self {
			tag_id: 1,
			content_type: None,
			object_id: 42,
		}
	}

	/// Set the tag_id
	pub fn tag_id(mut self, tag_id: i64) -> Self {
		self.tag_id = tag_id;
		self
	}

	/// Set the content_type
	pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
		self.content_type = Some(content_type.into());
		self
	}

	/// Set the object_id
	pub fn object_id(mut self, object_id: i64) -> Self {
		self.object_id = object_id;
		self
	}

	/// Build the TaggedItem instance
	pub fn build(self) -> TaggedItem {
		let content_type = self.content_type.unwrap_or_else(|| "Food".to_string());
		TaggedItem::new(self.tag_id, &content_type, self.object_id)
	}
}

impl Default for TaggedItemBuilder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_tagged_item() {
		let item = default_tagged_item();
		assert_eq!(item.tag_id, 1);
		assert_eq!(item.content_type, "Food");
		assert_eq!(item.object_id, 42);
		assert!(item.id.is_none());
	}

	#[test]
	fn test_custom_tagged_item() {
		let item = custom_tagged_item(2, "Recipe", 100);
		assert_eq!(item.tag_id, 2);
		assert_eq!(item.content_type, "Recipe");
		assert_eq!(item.object_id, 100);
	}

	#[test]
	fn test_tagged_item_list() {
		let items = tagged_item_list();
		assert_eq!(items.len(), 3);
		assert_eq!(items[0].tag_id, 1);
		assert_eq!(items[1].tag_id, 2);
		assert_eq!(items[2].tag_id, 3);
		// All have same content_type and object_id
		for item in &items {
			assert_eq!(item.content_type, "Food");
			assert_eq!(item.object_id, 42);
		}
	}

	#[test]
	fn test_tag_and_tagged_item() {
		let (tag_id, item) = tag_and_tagged_item();
		assert_eq!(tag_id, 1);
		assert_eq!(item.tag_id, 1);
		assert_eq!(item.content_type, "Food");
		assert_eq!(item.object_id, 42);
	}

	#[test]
	fn test_tagged_item_builder_default() {
		let item = TaggedItemBuilder::new().build();
		assert_eq!(item.tag_id, 1);
		assert_eq!(item.content_type, "Food");
		assert_eq!(item.object_id, 42);
	}

	#[test]
	fn test_tagged_item_builder_custom() {
		let item = TaggedItemBuilder::new()
			.tag_id(5)
			.content_type("Recipe")
			.object_id(100)
			.build();
		assert_eq!(item.tag_id, 5);
		assert_eq!(item.content_type, "Recipe");
		assert_eq!(item.object_id, 100);
	}

	#[test]
	fn test_tagged_item_builder_default_trait() {
		let item = TaggedItemBuilder::default().build();
		assert_eq!(item.tag_id, 1);
		assert_eq!(item.content_type, "Food");
		assert_eq!(item.object_id, 42);
	}
}
