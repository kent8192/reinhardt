//! Core `Document` trait definition.

use bson::Document as BsonDocument;
use serde::{Serialize, de::DeserializeOwned};

use super::error::OdmResult;

/// Core trait for MongoDB documents.
///
/// This trait is typically implemented automatically via the `#[document(...)]` macro.
///
/// ## Example
///
/// ```ignore
/// #[document(collection = "users", backend = "mongodb")]
/// struct User {
///     #[field(primary_key)]
///     id: ObjectId,
///     name: String,
/// }
/// ```
pub trait Document: Serialize + DeserializeOwned + Send + Sync + 'static {
	/// Primary key type (e.g., `ObjectId`, `Uuid`, `i64`).
	type Id: Serialize + DeserializeOwned + Send + Sync;

	/// Collection name in MongoDB.
	const COLLECTION_NAME: &'static str;

	/// Database name in MongoDB.
	const DATABASE_NAME: &'static str;

	/// Get the document's ID.
	///
	/// Returns `None` if the document hasn't been persisted yet.
	fn id(&self) -> Option<&Self::Id>;

	/// Set the document's ID.
	///
	/// This is typically called after insertion.
	fn set_id(&mut self, id: Self::Id);

	/// Get the backend type for this document.
	///
	/// Generated from `#[document(backend = "...")]` attribute.
	fn backend_type() -> crate::nosql::types::NoSQLBackendType;

	/// Get foreign key references for this document.
	///
	/// Returns a list of (field_name, referenced_collection) pairs.
	/// Generated from `#[field(references = "...")]` attributes.
	fn references() -> Vec<(&'static str, &'static str)> {
		Vec::new()
	}

	/// Get index definitions for this document.
	///
	/// Generated from `#[field(index)]` and `#[field(unique)]` attributes.
	fn indexes() -> Vec<IndexModel> {
		Vec::new()
	}

	/// Get the MongoDB validation schema.
	///
	/// Generated from `#[field(...)]` attributes.
	fn validation_schema() -> Option<BsonDocument> {
		None
	}

	/// Validate this document at the application layer.
	///
	/// Generated from `#[field(validate = "...")]` attributes.
	fn validate(&self) -> OdmResult<()> {
		Ok(())
	}
}

/// Sort order for an index key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexOrder {
	/// Ascending order (1).
	Ascending,
	/// Descending order (-1).
	Descending,
}

/// A single key in an index definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexKey {
	/// Field name to index.
	pub field: String,
	/// Sort order for this key.
	pub order: IndexOrder,
}

/// Options for an index definition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexOptions {
	/// If true, the index enforces uniqueness.
	pub unique: bool,
	/// If true, the index only references documents with the specified field.
	pub sparse: bool,
	/// Custom name for the index.
	pub name: Option<String>,
	/// TTL in seconds for documents in this collection.
	pub expire_after_seconds: Option<u64>,
}

/// Represents an index definition for a MongoDB collection.
///
/// Use [`IndexModel::builder()`] to construct an instance with the builder pattern.
///
/// ## Example
///
/// ```rust
/// use reinhardt_db::nosql::document::{IndexModel, IndexOrder};
///
/// let index = IndexModel::builder()
///     .key("email", IndexOrder::Ascending)
///     .unique(true)
///     .build();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexModel {
	/// Index keys specifying which fields to index and their sort order.
	pub keys: Vec<IndexKey>,
	/// Additional index options.
	pub options: IndexOptions,
}

impl IndexModel {
	/// Create a new index model builder.
	pub fn builder() -> IndexModelBuilder {
		IndexModelBuilder {
			keys: Vec::new(),
			options: IndexOptions::default(),
		}
	}
}

/// Builder for constructing [`IndexModel`] instances.
#[derive(Debug)]
pub struct IndexModelBuilder {
	keys: Vec<IndexKey>,
	options: IndexOptions,
}

impl IndexModelBuilder {
	/// Add an index key with the specified field name and sort order.
	pub fn key(mut self, field: impl Into<String>, order: IndexOrder) -> Self {
		self.keys.push(IndexKey {
			field: field.into(),
			order,
		});
		self
	}

	/// Set whether the index enforces uniqueness.
	pub fn unique(mut self, unique: bool) -> Self {
		self.options.unique = unique;
		self
	}

	/// Set whether the index is sparse.
	pub fn sparse(mut self, sparse: bool) -> Self {
		self.options.sparse = sparse;
		self
	}

	/// Set a custom name for the index.
	pub fn name(mut self, name: impl Into<String>) -> Self {
		self.options.name = Some(name.into());
		self
	}

	/// Set the TTL in seconds for documents in this collection.
	pub fn expire_after_seconds(mut self, seconds: u64) -> Self {
		self.options.expire_after_seconds = Some(seconds);
		self
	}

	/// Build the [`IndexModel`].
	pub fn build(self) -> IndexModel {
		IndexModel {
			keys: self.keys,
			options: self.options,
		}
	}
}

#[cfg(feature = "mongodb")]
impl From<IndexModel> for mongodb::IndexModel {
	fn from(model: IndexModel) -> Self {
		use std::time::Duration;

		let mut keys = bson::Document::new();
		for key in &model.keys {
			let value = match key.order {
				IndexOrder::Ascending => bson::Bson::Int32(1),
				IndexOrder::Descending => bson::Bson::Int32(-1),
			};
			keys.insert(key.field.clone(), value);
		}

		let mut opts = mongodb::options::IndexOptions::default();
		if model.options.unique {
			opts.unique = Some(true);
		}
		if model.options.sparse {
			opts.sparse = Some(true);
		}
		if model.options.name.is_some() {
			opts.name.clone_from(&model.options.name);
		}
		if let Some(seconds) = model.options.expire_after_seconds {
			opts.expire_after = Some(Duration::from_secs(seconds));
		}

		mongodb::IndexModel::builder()
			.keys(keys)
			.options(opts)
			.build()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn build_index_model_with_single_key() {
		// Arrange
		let builder = IndexModel::builder();

		// Act
		let index = builder
			.key("email", IndexOrder::Ascending)
			.unique(true)
			.build();

		// Assert
		assert_eq!(index.keys.len(), 1);
		assert_eq!(index.keys[0].field, "email");
		assert_eq!(index.keys[0].order, IndexOrder::Ascending);
		assert!(index.options.unique);
	}

	#[rstest]
	fn build_index_model_with_multiple_keys() {
		// Arrange & Act
		let index = IndexModel::builder()
			.key("user_id", IndexOrder::Ascending)
			.key("created_at", IndexOrder::Descending)
			.build();

		// Assert
		assert_eq!(index.keys.len(), 2);
		assert_eq!(index.keys[0].field, "user_id");
		assert_eq!(index.keys[0].order, IndexOrder::Ascending);
		assert_eq!(index.keys[1].field, "created_at");
		assert_eq!(index.keys[1].order, IndexOrder::Descending);
	}

	#[rstest]
	fn build_index_model_with_default_options() {
		// Arrange & Act
		let index = IndexModel::builder()
			.key("name", IndexOrder::Ascending)
			.build();

		// Assert
		assert!(!index.options.unique);
		assert!(!index.options.sparse);
		assert_eq!(index.options.name, None);
		assert_eq!(index.options.expire_after_seconds, None);
	}

	#[rstest]
	fn build_index_model_with_all_options() {
		// Arrange & Act
		let index = IndexModel::builder()
			.key("session_token", IndexOrder::Ascending)
			.unique(true)
			.sparse(true)
			.name("idx_session_token")
			.expire_after_seconds(3600)
			.build();

		// Assert
		assert!(index.options.unique);
		assert!(index.options.sparse);
		assert_eq!(index.options.name.as_deref(), Some("idx_session_token"));
		assert_eq!(index.options.expire_after_seconds, Some(3600));
	}
}
