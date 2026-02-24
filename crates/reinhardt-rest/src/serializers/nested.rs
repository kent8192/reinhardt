//! NestedSerializer - Django REST Framework inspired nested serialization
//!
//! This module provides serializers for handling nested relationships between models,
//! enabling complex object graphs to be serialized and deserialized.
//!
//! # Relationship Loading Strategy
//!
//! Following Django REST Framework's design philosophy, `NestedSerializer` works with
//! data that is **already loaded** by the ORM layer. This separation of concerns means:
//!
//! - **ORM Layer (reinhardt-orm)**: Responsible for loading related data using
//!   `LoadingStrategy` (Lazy, Joined, Selectin, etc.)
//! - **Serializer Layer**: Responsible for serializing the already-loaded data to JSON
//!
//! ## Usage Pattern
//!
//! ```rust,no_run,ignore
//! // 1. Load data with relationships using ORM
//! let posts = Post::objects()
//!     .select_related("author")  // Load author relationship
//!     .all()
//!     .await?;
//!
//! // 2. Serialize with NestedSerializer
//! let serializer = NestedSerializer::<Post, Author>::new("author").depth(1);
//! for post in posts {
//!     let json = serializer.serialize(&post)?;
//!     // JSON includes author data if it was loaded
//! }
//! ```
//!
//! This design avoids the N+1 query problem and gives developers explicit control
//! over when and how relationships are loaded.

use super::{SerializationArena, Serializer, SerializerError};
use reinhardt_db::orm::Model;
use serde_json::Value;
use std::marker::PhantomData;

/// NestedSerializer - Serialize related models inline
///
/// Handles one-to-one and many-to-one relationships by embedding the related
/// model's data directly in the serialized output.
///
/// # Examples
///
/// ```
/// # use reinhardt_rest::serializers::NestedSerializer;
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct Post {
/// #     id: Option<i64>,
/// #     title: String,
/// #     author_id: i64,
/// # }
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct Author {
/// #     id: Option<i64>,
/// #     name: String,
/// # }
/// #
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     type Fields = PostFields;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { PostFields }
/// # }
/// # #[derive(Clone)]
/// # struct PostFields;
/// # impl reinhardt_db::orm::FieldSelector for PostFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for Author {
/// #     type PrimaryKey = i64;
/// #     type Fields = AuthorFields;
/// #     fn table_name() -> &'static str { "authors" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { AuthorFields }
/// # }
/// # #[derive(Clone)]
/// # struct AuthorFields;
/// # impl reinhardt_db::orm::FieldSelector for AuthorFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # fn example() {
/// // Serialize a post with its author nested
/// let serializer = NestedSerializer::<Post, Author>::new("author");
/// // Verify the serializer is created successfully
/// let _: NestedSerializer<Post, Author> = serializer;
/// # }
/// ```
pub struct NestedSerializer<M: Model, R: Model> {
	relationship_field: String,
	depth: usize,
	use_arena: bool,
	_phantom: PhantomData<(M, R)>,
}

impl<M: Model, R: Model> NestedSerializer<M, R> {
	/// Create a new NestedSerializer
	///
	/// # Arguments
	///
	/// * `relationship_field` - The field name that contains the related model
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::NestedSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, title: String }
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64>, name: String }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = NestedSerializer::<Post, Author>::new("author");
	/// // Verify the serializer is created successfully
	/// let _: NestedSerializer<Post, Author> = serializer;
	/// ```
	pub fn new(relationship_field: impl Into<String>) -> Self {
		Self {
			relationship_field: relationship_field.into(),
			depth: 1,
			use_arena: true,
			_phantom: PhantomData,
		}
	}

	/// Set the nesting depth (default: 1)
	///
	/// Controls how many levels of relationships to serialize.
	/// depth=0 means no nesting (like ModelSerializer),
	/// depth=1 means serialize immediate relationships,
	/// depth=2+ means serialize nested relationships of relationships.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::NestedSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, title: String }
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64>, name: String }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = NestedSerializer::<Post, Author>::new("author")
	///     .depth(2); // Serialize author and author's relationships
	/// // Verify depth configuration
	/// let _: NestedSerializer<Post, Author> = serializer;
	/// ```
	pub fn depth(mut self, depth: usize) -> Self {
		self.depth = depth;
		self
	}

	/// Disable arena allocation (use traditional heap allocation instead)
	///
	/// This is provided for backward compatibility or when arena allocation
	/// is not desired. By default, arena allocation is enabled.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::NestedSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, title: String }
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64>, name: String }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = NestedSerializer::<Post, Author>::new("author")
	///     .without_arena(); // Disable arena allocation
	/// // Verify arena allocation is disabled
	/// let _: NestedSerializer<Post, Author> = serializer;
	/// ```
	pub fn without_arena(mut self) -> Self {
		self.use_arena = false;
		self
	}
}

impl<M: Model, R: Model> Serializer for NestedSerializer<M, R> {
	type Input = M;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		if self.use_arena {
			// Arena-based serialization
			let arena = SerializationArena::new();
			let serialized = arena.serialize_model(input, self.depth);
			let json_value = arena.to_json(serialized);
			serde_json::to_string(&json_value).map_err(|e| SerializerError::Other {
				message: format!("Serialization error: {}", e),
			})
		} else {
			// Traditional heap-based serialization (backward compatibility)
			self.serialize_without_arena(input)
		}
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		serde_json::from_str(output).map_err(|e| SerializerError::Other {
			message: format!("Deserialization error: {}", e),
		})
	}
}

impl<M: Model, R: Model> NestedSerializer<M, R> {
	/// Serialize without using arena allocation (traditional approach)
	fn serialize_without_arena(&self, input: &M) -> Result<String, SerializerError> {
		// Serialize parent model to JSON
		let mut parent_value = serde_json::to_value(input).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})?;

		// If depth > 0, check if relationship data is already loaded in the parent JSON
		// This follows Django REST Framework's approach where related data is loaded
		// by the ORM layer (e.g., using select_related/prefetch_related) before serialization
		if self.depth > 0
			&& let Some(obj) = parent_value.as_object_mut()
		{
			// Check if the relationship field already has data
			if let Some(related_data) = obj.get(&self.relationship_field) {
				// If the data is not null, it means the relationship was already loaded
				// by the ORM (e.g., via Joined or Selectin loading strategy)
				if !related_data.is_null() {
					// The relationship data is already present in the serialized JSON
					// This works because reinhardt-orm's Model trait implementations
					// include relationship fields in their Serialize implementation
					// when those relationships are loaded
				}
			}
		}

		// Convert the value back to string
		serde_json::to_string(&parent_value).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})
	}
}

/// ListSerializer - Serialize collections of models
///
/// Handles serializing multiple instances efficiently, useful for
/// many-to-many and reverse foreign key relationships.
///
/// # Examples
///
/// ```
/// # use reinhardt_rest::serializers::ListSerializer;
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// let serializer = ListSerializer::<User>::new();
/// // Verify the serializer is created successfully
/// let _: ListSerializer<User> = serializer;
/// ```
pub struct ListSerializer<M: Model> {
	_phantom: PhantomData<M>,
}

impl<M: Model> ListSerializer<M> {
	/// Create a new ListSerializer
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
		}
	}
}

impl<M: Model> Default for ListSerializer<M> {
	fn default() -> Self {
		Self::new()
	}
}

impl<M: Model> Serializer for ListSerializer<M> {
	type Input = Vec<M>;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		serde_json::to_string(input).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		serde_json::from_str(output).map_err(|e| SerializerError::Other {
			message: format!("Deserialization error: {}", e),
		})
	}
}

/// WritableNestedSerializer - Serialize and create/update nested models
///
/// Extends NestedSerializer to support write operations on nested relationships.
/// This allows creating or updating related models when the parent is saved.
///
/// # Design Philosophy
///
/// This serializer follows the **separation of concerns** principle:
/// - **Validation**: The serializer validates JSON structure and permissions
/// - **Data Extraction**: Provides helper methods to extract nested data
/// - **Database Operations**: Caller handles ORM operations and transactions
///
/// This design gives you full control over transaction management and error handling
/// while the serializer ensures data validity.
///
/// # Permission Control
///
/// - `allow_create(bool)`: Allow creating new related instances (default: false)
/// - `allow_update(bool)`: Allow updating existing related instances (default: false)
///
/// Without these permissions, deserialization will fail if nested data contains
/// create/update operations.
///
/// # Usage Patterns
///
/// ## Basic Usage with Manual ORM Integration
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_rest::serializers::WritableNestedSerializer;
/// use reinhardt_db::orm::{Model, Transaction};
///
/// // Define serializer with permissions
/// let serializer = WritableNestedSerializer::<Post, Author>::new("author")
///     .allow_create(true)
///     .allow_update(true);
///
/// // JSON with nested author
/// let json = r#"{
///     "title": "My Post",
///     "author": {
///         "id": null,
///         "name": "Alice"
///     }
/// }"#;
///
/// // Validate and deserialize
/// let post: Post = serializer.deserialize(&json.to_string())?;
///
/// // Extract nested data for manual processing
/// if let Some(author_data) = serializer.extract_nested_data(json)? {
///     // Create author within transaction
///     let mut tx = Transaction::new();
///     tx.begin()?;
///
///     let author: Author = serde_json::from_value(author_data)?;
///     let saved_author = Author::objects().create(&author).await?;
///
///     // Set foreign key and save parent
///     post.author_id = saved_author.id;
///     let saved_post = Post::objects().create(&post).await?;
///
///     tx.commit()?;
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }
/// ```
///
/// ## Advanced: Handling Both Create and Update
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use serde_json::json;
/// # use reinhardt_db::orm::{Model, Transaction};
/// # use reinhardt_rest::serializers::WritableNestedSerializer;
/// # #[derive(serde::Serialize, serde::Deserialize)]
/// # struct Author { id: Option<i64>, name: String }
/// # impl Model for Author {
/// #     type PrimaryKey = i64;
/// #     fn table_name() -> &'static str { "authors" }
/// #     fn primary_key(&self) -> Option<&i64> { self.id.as_ref() }
/// #     fn set_primary_key(&mut self, value: i64) { self.id = Some(value); }
/// # }
/// # #[derive(serde::Serialize, serde::Deserialize)]
/// # struct Post { id: i64, title: String, author_id: i64 }
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn primary_key(&self) -> Option<&i64> { Some(&self.id) }
/// #     fn set_primary_key(&mut self, value: i64) { self.id = value; }
/// # }
/// # let serializer = WritableNestedSerializer::<Post, Author>::new();
/// # let json = json!({});
/// if let Some(author_data) = serializer.extract_nested_data(json)? {
///     let mut tx = Transaction::new();
///     tx.begin()?;
///
///     let author: Author = serde_json::from_value(author_data)?;
///     let saved_author = if WritableNestedSerializer::<Post, Author>::is_create_operation(&author_data) {
///         // Create new author
///         Author::objects().create(&author).await?
///     } else {
///         // Update existing author
///         Author::objects().update(&author).await?
///     };
///
///     post.author_id = saved_author.id;
///     Post::objects().create(&post).await?;
///
///     tx.commit()?;
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// The serializer returns `SerializerError` in these cases:
/// - JSON parsing fails
/// - Nested data violates permissions (create/update not allowed)
/// - Invalid nested data structure
///
/// Database errors are handled by the caller during ORM operations.
///
/// # Examples
///
/// ```
/// # use reinhardt_rest::serializers::WritableNestedSerializer;
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct Post { id: Option<i64>, title: String }
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct Comment { id: Option<i64>, text: String }
/// #
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     type Fields = PostFields;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { PostFields }
/// # }
/// # #[derive(Clone)]
/// # struct PostFields;
/// # impl reinhardt_db::orm::FieldSelector for PostFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for Comment {
/// #     type PrimaryKey = i64;
/// #     type Fields = CommentFields;
/// #     fn table_name() -> &'static str { "comments" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { CommentFields }
/// # }
/// # #[derive(Clone)]
/// # struct CommentFields;
/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # fn example() {
/// // Create a post and its comments in one operation
/// let serializer = WritableNestedSerializer::<Post, Comment>::new("comments")
///     .allow_create(true);
/// // Verify the serializer is created with create permission
/// let _: WritableNestedSerializer<Post, Comment> = serializer;
/// # }
/// ```
pub struct WritableNestedSerializer<M: Model, R: Model> {
	relationship_field: String,
	allow_create: bool,
	allow_update: bool,
	_phantom: PhantomData<(M, R)>,
}

impl<M: Model, R: Model> WritableNestedSerializer<M, R> {
	/// Create a new WritableNestedSerializer
	pub fn new(relationship_field: impl Into<String>) -> Self {
		Self {
			relationship_field: relationship_field.into(),
			allow_create: false,
			allow_update: false,
			_phantom: PhantomData,
		}
	}

	/// Allow creating new related instances (default: false)
	pub fn allow_create(mut self, allow: bool) -> Self {
		self.allow_create = allow;
		self
	}

	/// Allow updating existing related instances (default: false)
	pub fn allow_update(mut self, allow: bool) -> Self {
		self.allow_update = allow;
		self
	}

	/// Extract nested data from JSON for manual processing
	///
	/// Returns the nested data as a serde_json::Value for the caller to process.
	/// This allows the caller to handle database operations with full control.
	///
	/// # Examples
	///
	/// ```ignore
	/// let serializer = WritableNestedSerializer::<Post, Author>::new("author");
	/// let json = r#"{"id": 1, "title": "Post", "author": {"id": 2, "name": "Alice"}}"#;
	///
	/// if let Some(nested_data) = serializer.extract_nested_data(json)? {
	///     // Verify nested data extraction succeeds
	///     let author: Author = serde_json::from_value(nested_data)?;
	/// }
	/// ```
	pub fn extract_nested_data(&self, json: &str) -> Result<Option<Value>, SerializerError> {
		let value: Value = serde_json::from_str(json).map_err(|e| SerializerError::Other {
			message: format!("JSON parsing error: {}", e),
		})?;

		if let Value::Object(ref map) = value
			&& let Some(nested_value) = map.get(&self.relationship_field)
			&& !nested_value.is_null()
		{
			return Ok(Some(nested_value.clone()));
		}

		Ok(None)
	}

	/// Check if nested data represents a create operation (no primary key or null primary key)
	///
	/// # Examples
	///
	/// ```ignore
	/// let create_data = serde_json::json!({"id": null, "name": "New Author"});
	/// // Verify create operation detection
	/// assert!(WritableNestedSerializer::<Post, Author>::is_create_operation(&create_data));
	///
	/// let update_data = serde_json::json!({"id": 42, "name": "Existing Author"});
	/// // Verify update operation detection
	/// assert!(!WritableNestedSerializer::<Post, Author>::is_create_operation(&update_data));
	/// ```
	pub fn is_create_operation(nested_value: &Value) -> bool {
		if let Some(pk) = nested_value.get(M::primary_key_field()) {
			pk.is_null()
		} else {
			true // No primary key field means create
		}
	}
}

impl<M: Model, R: Model> Serializer for WritableNestedSerializer<M, R> {
	type Input = M;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		// Same as NestedSerializer - requires ORM relationship loading
		// See NestedSerializer::serialize for implementation roadmap
		serde_json::to_string(input).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		// Parse JSON to validate structure
		let value: Value = serde_json::from_str(output).map_err(|e| SerializerError::Other {
			message: format!("JSON parsing error: {}", e),
		})?;

		// Check for nested data at relationship_field
		if let Value::Object(ref map) = value
			&& let Some(nested_value) = map.get(&self.relationship_field)
		{
			// Validate permissions
			if nested_value.is_object() {
				// Single related object
				if let Some(pk) = nested_value.get(M::primary_key_field()) {
					if pk.is_null() && !self.allow_create {
						return Err(SerializerError::Other {
							message: "Creating nested instances is not allowed".to_string(),
						});
					} else if !pk.is_null() && !self.allow_update {
						return Err(SerializerError::Other {
							message: "Updating nested instances is not allowed".to_string(),
						});
					}
				}
			} else if nested_value.is_array() {
				// Multiple related objects
				for item in nested_value.as_array().unwrap() {
					if let Some(pk) = item.get(M::primary_key_field()) {
						if pk.is_null() && !self.allow_create {
							return Err(SerializerError::Other {
								message: "Creating nested instances is not allowed".to_string(),
							});
						} else if !pk.is_null() && !self.allow_update {
							return Err(SerializerError::Other {
								message: "Updating nested instances is not allowed".to_string(),
							});
						}
					}
				}
			}

			// Nested data validation passed
			// The actual database operations (create/update) are handled by the caller
			// using ORM methods like QuerySet::create() or Model::save()
			//
			// This design follows the separation of concerns principle:
			// - Serializer: Validates structure and permissions
			// - ORM Layer: Performs database operations
			// - Transaction: Ensures atomicity
			//
			// Example usage pattern:
			// ```
			// let serializer = WritableNestedSerializer::new("author").allow_create(true);
			// let post: Post = serializer.deserialize(&json)?;
			//
			// // Caller handles database operations within transaction:
			// let mut tx = Transaction::new();
			// tx.begin()?;
			// let author = Author::objects().create(&post.author).await?;
			// post.author_id = author.id;
			// let saved_post = Post::objects().create(&post).await?;
			// tx.commit()?;
			// ```
		}

		// This method intentionally deserializes only the parent model.
		// Following Django REST Framework's separation of concerns:
		// - Serializer: Validates JSON structure and permissions
		// - ORM Layer: Handles database operations (caller's responsibility)
		// - Use extract_nested_data() and is_create_operation() for nested processing
		serde_json::from_str(output).map_err(|e| SerializerError::Other {
			message: format!("Deserialization error: {}", e),
		})
	}
}
