//! Generic relations for polymorphic model relationships
//!
//! This module provides `GenericRelationSet` for reverse lookups in models
//! that use `GenericForeignKey` to reference the parent model.
//!
//! # Overview
//!
//! In Django-style contenttypes, a `GenericForeignKey` allows a model to reference
//! any other model. The `GenericRelationSet` provides the reverse side of this
//! relationship, enabling queries like "get all comments for this post".
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_db::orm::relations::GenericRelationSet;
//!
//! // Post model with generic relations to comments
//! struct Post {
//!     id: i64,
//!     title: String,
//!     // Reverse relation to Comment model
//!     comments: GenericRelationSet<Comment>,
//! }
//!
//! // Get all comments for a post
//! let comments = post.comments.all().await?;
//! ```

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::orm::Model;

/// A set of objects that have a GenericForeignKey pointing to the owner model
///
/// This struct represents a reverse relation for GenericForeignKey relationships.
/// It provides QuerySet-like operations for querying related objects.
///
/// # Type Parameters
///
/// - `T`: The model type that has a GenericForeignKey pointing back to the owner
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::relations::GenericRelationSet;
///
/// // Create a relation set configuration
/// let relation: GenericRelationSet<()> = GenericRelationSet::new(
///     1,                        // content_type_id of the owner model
///     42,                       // object_id of the owner instance
///     "content_type_id",        // field name for content type in related model
///     "object_id",              // field name for object id in related model
/// );
///
/// assert_eq!(relation.content_type_id(), 1);
/// assert_eq!(relation.object_id(), 42);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericRelationSet<T> {
	/// Content type ID of the owner model
	content_type_id: i64,
	/// Object ID of the owner instance
	object_id: i64,
	/// Field name for content type in the related model
	ct_field: String,
	/// Field name for object id in the related model
	fk_field: String,
	/// Phantom data for the related model type
	#[serde(skip)]
	_phantom: PhantomData<T>,
}

impl<T> GenericRelationSet<T> {
	/// Create a new GenericRelationSet
	///
	/// # Arguments
	///
	/// - `content_type_id`: Content type ID of the owner model
	/// - `object_id`: Object ID of the owner instance
	/// - `ct_field`: Field name for content type in the related model (default: "content_type_id")
	/// - `fk_field`: Field name for object id in the related model (default: "object_id")
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::relations::GenericRelationSet;
	///
	/// let relation: GenericRelationSet<()> = GenericRelationSet::new(
	///     5, 100, "content_type_id", "object_id"
	/// );
	/// ```
	pub fn new(
		content_type_id: i64,
		object_id: i64,
		ct_field: impl Into<String>,
		fk_field: impl Into<String>,
	) -> Self {
		Self {
			content_type_id,
			object_id,
			ct_field: ct_field.into(),
			fk_field: fk_field.into(),
			_phantom: PhantomData,
		}
	}

	/// Create a new GenericRelationSet with default field names
	///
	/// Uses "content_type_id" and "object_id" as the default field names.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::relations::GenericRelationSet;
	///
	/// let relation: GenericRelationSet<()> = GenericRelationSet::with_defaults(5, 100);
	/// assert_eq!(relation.ct_field(), "content_type_id");
	/// assert_eq!(relation.fk_field(), "object_id");
	/// ```
	pub fn with_defaults(content_type_id: i64, object_id: i64) -> Self {
		Self::new(content_type_id, object_id, "content_type_id", "object_id")
	}

	/// Get the content type ID of the owner model
	pub fn content_type_id(&self) -> i64 {
		self.content_type_id
	}

	/// Get the object ID of the owner instance
	pub fn object_id(&self) -> i64 {
		self.object_id
	}

	/// Get the content type field name
	pub fn ct_field(&self) -> &str {
		&self.ct_field
	}

	/// Get the object id field name
	pub fn fk_field(&self) -> &str {
		&self.fk_field
	}

	/// Generate the WHERE clause for filtering related objects
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::relations::GenericRelationSet;
	///
	/// let relation: GenericRelationSet<()> = GenericRelationSet::new(
	///     1, 42, "content_type_id", "object_id"
	/// );
	/// assert_eq!(
	///     relation.where_clause(),
	///     "content_type_id = 1 AND object_id = 42"
	/// );
	/// ```
	pub fn where_clause(&self) -> String {
		format!(
			"{} = {} AND {} = {}",
			self.ct_field, self.content_type_id, self.fk_field, self.object_id
		)
	}

	/// Generate SQL condition for use in a WHERE clause
	///
	/// Returns a tuple of (condition_string, values) for parameterized queries.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::relations::GenericRelationSet;
	///
	/// let relation: GenericRelationSet<()> = GenericRelationSet::new(
	///     1, 42, "content_type_id", "object_id"
	/// );
	/// let (sql, values) = relation.sql_condition();
	/// assert_eq!(sql, "content_type_id = $1 AND object_id = $2");
	/// assert_eq!(values, vec![1_i64, 42_i64]);
	/// ```
	pub fn sql_condition(&self) -> (String, Vec<i64>) {
		let sql = format!("{} = $1 AND {} = $2", self.ct_field, self.fk_field);
		(sql, vec![self.content_type_id, self.object_id])
	}
}

impl<T: Model> GenericRelationSet<T> {
	/// Create a QuerySet for related objects
	///
	/// Returns a QuerySet configured to filter by the content type and object ID.
	/// This allows chaining additional filters before executing the query.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let active_comments = post.comments
	///     .query()
	///     .filter(Filter::new("is_active", FilterOperator::Eq, FilterValue::Bool(true)))
	///     .all()
	///     .await?;
	/// ```
	pub fn query(&self) -> super::query::QuerySet<T> {
		use crate::orm::query::{Filter, FilterOperator, FilterValue};

		// Create filters for content_type_id and object_id
		let ct_filter = Filter::new(
			self.ct_field.clone(),
			FilterOperator::Eq,
			FilterValue::Integer(self.content_type_id),
		);
		let fk_filter = Filter::new(
			self.fk_field.clone(),
			FilterOperator::Eq,
			FilterValue::Integer(self.object_id),
		);

		T::objects().all().filter(ct_filter).filter(fk_filter)
	}

	/// Get all related objects
	///
	/// Returns all instances of the related model that have a GenericForeignKey
	/// pointing to the owner instance.
	///
	/// # Returns
	///
	/// A Result containing a Vec of related model instances.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let comments = post.comments.all().await?;
	/// for comment in comments {
	///     println!("Comment: {}", comment.content);
	/// }
	/// ```
	pub async fn all(&self) -> reinhardt_core::exception::Result<Vec<T>> {
		self.query().all().await
	}

	/// Count related objects
	///
	/// Returns the count of related model instances.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let comment_count = post.comments.count().await?;
	/// println!("Post has {} comments", comment_count);
	/// ```
	pub async fn count(&self) -> reinhardt_core::exception::Result<usize> {
		self.query().count().await
	}

	/// Check if any related objects exist
	///
	/// # Example
	///
	/// ```rust,ignore
	/// if post.comments.exists().await? {
	///     println!("Post has comments");
	/// }
	/// ```
	pub async fn exists(&self) -> reinhardt_core::exception::Result<bool> {
		Ok(self.count().await? > 0)
	}

	/// Get first related object
	///
	/// # Example
	///
	/// ```rust,ignore
	/// if let Some(first_comment) = post.comments.first().await? {
	///     println!("First comment: {}", first_comment.content);
	/// }
	/// ```
	pub async fn first(&self) -> reinhardt_core::exception::Result<Option<T>> {
		self.query().first().await
	}
}

/// Configuration for a GenericRelation field in model definition
///
/// This struct holds the configuration needed to set up a GenericRelation
/// on a model, typically used during macro expansion.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::relations::GenericRelationConfig;
///
/// let config = GenericRelationConfig::new("Comment")
///     .ct_field("content_type_id")
///     .fk_field("object_id");
///
/// assert_eq!(config.related_model(), "Comment");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericRelationConfig {
	/// Name of the related model
	related_model: String,
	/// Content type field name in the related model
	ct_field: String,
	/// Object id field name in the related model
	fk_field: String,
	/// Optional related name for the relation
	related_name: Option<String>,
}

impl GenericRelationConfig {
	/// Create a new GenericRelationConfig
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::relations::GenericRelationConfig;
	///
	/// let config = GenericRelationConfig::new("Comment");
	/// assert_eq!(config.related_model(), "Comment");
	/// ```
	pub fn new(related_model: impl Into<String>) -> Self {
		Self {
			related_model: related_model.into(),
			ct_field: "content_type_id".to_string(),
			fk_field: "object_id".to_string(),
			related_name: None,
		}
	}

	/// Set the content type field name
	pub fn ct_field(mut self, field: impl Into<String>) -> Self {
		self.ct_field = field.into();
		self
	}

	/// Set the object id field name
	pub fn fk_field(mut self, field: impl Into<String>) -> Self {
		self.fk_field = field.into();
		self
	}

	/// Set the related name
	pub fn related_name(mut self, name: impl Into<String>) -> Self {
		self.related_name = Some(name.into());
		self
	}

	/// Get the related model name
	pub fn related_model(&self) -> &str {
		&self.related_model
	}

	/// Get the content type field name
	pub fn get_ct_field(&self) -> &str {
		&self.ct_field
	}

	/// Get the object id field name
	pub fn get_fk_field(&self) -> &str {
		&self.fk_field
	}

	/// Get the related name if set
	pub fn get_related_name(&self) -> Option<&str> {
		self.related_name.as_deref()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_generic_relation_set_new() {
		let relation: GenericRelationSet<()> =
			GenericRelationSet::new(1, 42, "content_type_id", "object_id");

		assert_eq!(relation.content_type_id(), 1);
		assert_eq!(relation.object_id(), 42);
		assert_eq!(relation.ct_field(), "content_type_id");
		assert_eq!(relation.fk_field(), "object_id");
	}

	#[test]
	fn test_generic_relation_set_with_defaults() {
		let relation: GenericRelationSet<()> = GenericRelationSet::with_defaults(5, 100);

		assert_eq!(relation.content_type_id(), 5);
		assert_eq!(relation.object_id(), 100);
		assert_eq!(relation.ct_field(), "content_type_id");
		assert_eq!(relation.fk_field(), "object_id");
	}

	#[test]
	fn test_generic_relation_set_where_clause() {
		let relation: GenericRelationSet<()> = GenericRelationSet::new(1, 42, "ct_id", "obj_id");

		assert_eq!(relation.where_clause(), "ct_id = 1 AND obj_id = 42");
	}

	#[test]
	fn test_generic_relation_set_sql_condition() {
		let relation: GenericRelationSet<()> =
			GenericRelationSet::new(1, 42, "content_type_id", "object_id");

		let (sql, values) = relation.sql_condition();
		assert_eq!(sql, "content_type_id = $1 AND object_id = $2");
		assert_eq!(values, vec![1_i64, 42_i64]);
	}

	#[test]
	fn test_generic_relation_config_new() {
		let config = GenericRelationConfig::new("Comment");

		assert_eq!(config.related_model(), "Comment");
		assert_eq!(config.get_ct_field(), "content_type_id");
		assert_eq!(config.get_fk_field(), "object_id");
		assert!(config.get_related_name().is_none());
	}

	#[test]
	fn test_generic_relation_config_builder() {
		let config = GenericRelationConfig::new("Comment")
			.ct_field("ct_id")
			.fk_field("obj_id")
			.related_name("comments");

		assert_eq!(config.related_model(), "Comment");
		assert_eq!(config.get_ct_field(), "ct_id");
		assert_eq!(config.get_fk_field(), "obj_id");
		assert_eq!(config.get_related_name(), Some("comments"));
	}

	#[test]
	fn test_generic_relation_set_serialization() {
		let relation: GenericRelationSet<()> = GenericRelationSet::new(1, 42, "ct", "obj");

		let serialized = serde_json::to_string(&relation).unwrap();
		assert!(serialized.contains("1"));
		assert!(serialized.contains("42"));

		let deserialized: GenericRelationSet<()> = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized.content_type_id(), 1);
		assert_eq!(deserialized.object_id(), 42);
	}
}
