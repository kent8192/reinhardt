//! Type-safe ordering field definition
//!
//! Provides compile-time safe field ordering using reinhardt-orm's Field system.
//! Similar to Field/Lookup pattern in field_lookup module.

use reinhardt_db::orm::Model;
use std::marker::PhantomData;

/// Ordering direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
	/// Ascending order (ASC)
	Asc,
	/// Descending order (DESC)
	Desc,
}

/// Type-safe ordering field
///
/// Represents a field with its ordering direction. Can be created from any Field<M, T>
/// using the `.asc()` or `.desc()` methods on Field.
///
/// # Examples
///
/// ```rust
/// # use reinhardt_rest::filters::{OrderingField, OrderDirection, field_extensions::FieldOrderingExt};
/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
/// #
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct Post {
/// #     id: i64,
/// #     title: String,
/// #     created_at: String,
/// # }
/// #
/// # #[derive(Clone)]
/// # struct PostFields;
/// # impl FieldSelector for PostFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     type Fields = PostFields;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn new_fields() -> Self::Fields { PostFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// # }
/// // Create ascending ordering - call .asc() on Field
/// let field = Field::<Post, String>::new(vec!["title"]);
/// let asc_order = field.asc();
/// assert_eq!(asc_order.direction(), OrderDirection::Asc);
///
/// // Create descending ordering - call .desc() on Field
/// let field = Field::<Post, String>::new(vec!["created_at"]);
/// let desc_order = field.desc();
/// assert_eq!(desc_order.direction(), OrderDirection::Desc);
/// ```
pub struct OrderingField<M: Model> {
	pub(crate) field_path: Vec<String>,
	pub(crate) direction: OrderDirection,
	pub(crate) _phantom: PhantomData<M>,
}

impl<M: Model> OrderingField<M> {
	/// Create a new ordering field from path and direction
	pub(crate) fn new(field_path: Vec<String>, direction: OrderDirection) -> Self {
		Self {
			field_path,
			direction,
			_phantom: PhantomData,
		}
	}
	/// Get the field path
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// #
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// let field = Field::<Post, String>::new(vec!["title".to_string()]);
	/// let order = field.asc();
	/// assert_eq!(order.field_path(), &["title".to_string()]);
	/// ```
	pub fn field_path(&self) -> &[String] {
		&self.field_path
	}
	/// Get the ordering direction
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::{OrderDirection, field_extensions::FieldOrderingExt};
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// #
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     created_at: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// let field = Field::<Post, String>::new(vec!["created_at"]);
	/// let order = field.desc();
	/// assert_eq!(order.direction(), OrderDirection::Desc);
	/// ```
	pub fn direction(&self) -> OrderDirection {
		self.direction
	}

	/// Convert to SQL ORDER BY clause fragment
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// #
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// #     created_at: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// let order = Field::<Post, String>::new(vec!["title"]).asc();
	/// assert_eq!(order.to_sql(), "title ASC");
	///
	/// let order = Field::<Post, String>::new(vec!["created_at"]).desc();
	/// assert_eq!(order.to_sql(), "created_at DESC");
	/// ```
	pub fn to_sql(&self) -> String {
		let field_name = self.field_path.join(".");
		let direction_str = match self.direction {
			OrderDirection::Asc => "ASC",
			OrderDirection::Desc => "DESC",
		};
		format!("{} {}", field_name, direction_str)
	}
}

impl<M: Model> Clone for OrderingField<M> {
	fn clone(&self) -> Self {
		Self {
			field_path: self.field_path.clone(),
			direction: self.direction,
			_phantom: PhantomData,
		}
	}
}
