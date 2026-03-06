//! Tag model fixtures for reinhardt-taggit tests
//!
//! Provides reusable fixtures for generating Tag test data,
//! including database helper functions using SeaQuery.

use chrono::Utc;
use reinhardt_taggit::Tag;
use sea_query::{Alias, PostgresQueryBuilder, Query};
use sqlx::Row;

/// Default tag: name="rust", slug="rust"
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::default_tag;
///
/// #[rstest]
/// fn test_tag_fixture(default_tag: Tag) {
///     assert_eq!(default_tag.name, "rust");
///     assert_eq!(default_tag.slug, "rust");
/// }
/// ```
pub fn default_tag() -> Tag {
	Tag::new("rust", "rust")
}

/// Custom tag with specified name
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::custom_tag;
///
/// #[rstest]
/// fn test_custom_tag(#[with("python")] custom_tag: Tag) {
///     assert_eq!(custom_tag.name, "python");
/// }
/// ```
pub fn custom_tag(name: &str) -> Tag {
	let slug = name.to_lowercase().replace(" ", "-");
	Tag::new(name, &slug)
}

/// List of predefined tags
///
/// Returns multiple tags for bulk operations testing.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::tag_list;
///
/// #[rstest]
/// fn test_tag_list(tag_list: Vec<Tag>) {
///     assert_eq!(tag_list.len(), 3);
/// }
/// ```
pub fn tag_list() -> Vec<Tag> {
	vec![
		Tag::new("rust", "rust"),
		Tag::new("python", "python"),
		Tag::new("javascript", "javascript"),
	]
}

/// Insert a tag into the database and return its generated id
///
/// Uses SeaQuery to construct the INSERT statement.
pub async fn insert_tag_to_db(pool: &sqlx::PgPool, name: &str, slug: &str) -> i64 {
	let now = Utc::now().to_rfc3339();
	let sql = Query::insert()
		.into_table(Alias::new("tags"))
		.columns([
			Alias::new("name"),
			Alias::new("slug"),
			Alias::new("created_at"),
		])
		.values_panic([name.into(), slug.into(), now.into()])
		.returning_col(Alias::new("id"))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&sql)
		.fetch_one(pool)
		.await
		.expect("Failed to insert tag");

	row.get("id")
}

/// Builder for creating custom Tag instances
///
/// Provides a fluent API for building Tag instances with custom values.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit_tests::TagBuilder;
///
/// let tag = TagBuilder::new("Web Development")
///     .slug("web-dev")
///     .build();
///
/// assert_eq!(tag.name, "Web Development");
/// assert_eq!(tag.slug, "web-dev");
/// ```
pub struct TagBuilder {
	name: String,
	slug: Option<String>,
}

impl TagBuilder {
	/// Create a new TagBuilder with the specified name
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			slug: None,
		}
	}

	/// Set a custom slug (defaults to lowercased name with hyphens)
	pub fn slug(mut self, slug: impl Into<String>) -> Self {
		self.slug = Some(slug.into());
		self
	}

	/// Build the Tag instance
	pub fn build(self) -> Tag {
		let slug = self
			.slug
			.unwrap_or_else(|| self.name.to_lowercase().replace(" ", "-"));
		Tag::new(&self.name, &slug)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_tag() {
		let tag = default_tag();
		assert_eq!(tag.name, "rust");
		assert_eq!(tag.slug, "rust");
		assert!(tag.id.is_none());
	}

	#[test]
	fn test_custom_tag() {
		let tag = custom_tag("python");
		assert_eq!(tag.name, "python");
		assert_eq!(tag.slug, "python");
	}

	#[test]
	fn test_custom_tag_with_spaces() {
		let tag = custom_tag("Web Development");
		assert_eq!(tag.name, "Web Development");
		assert_eq!(tag.slug, "web-development");
	}

	#[test]
	fn test_tag_list() {
		let tags = tag_list();
		assert_eq!(tags.len(), 3);
		assert_eq!(tags[0].name, "rust");
		assert_eq!(tags[1].name, "python");
		assert_eq!(tags[2].name, "javascript");
	}

	#[test]
	fn test_tag_builder_default_slug() {
		let tag = TagBuilder::new("Web Development").build();
		assert_eq!(tag.name, "Web Development");
		assert_eq!(tag.slug, "web-development");
	}

	#[test]
	fn test_tag_builder_custom_slug() {
		let tag = TagBuilder::new("Web Development").slug("web-dev").build();
		assert_eq!(tag.name, "Web Development");
		assert_eq!(tag.slug, "web-dev");
	}
}
