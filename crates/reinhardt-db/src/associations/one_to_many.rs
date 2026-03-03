//! One-to-Many relationship definition
//!
//! Provides One-to-Many relationship types for defining reverse side of
//! ForeignKey relationships.

use std::marker::PhantomData;

use super::reverse::ReverseRelationship;

/// One-to-Many relationship field
///
/// Represents the "many" side of a one-to-many relationship.
/// This is the reverse accessor for a ForeignKey relationship.
///
/// # Type Parameters
///
/// * `T` - The type of the related model (the "many" side)
/// * `K` - The type of the primary key field
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::OneToMany;
///
/// #[derive(Clone)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// #[derive(Clone)]
/// struct Post {
///     id: i64,
///     author_id: i64,
///     title: String,
/// }
///
/// // Define one-to-many relationship on User model
/// // This creates a reverse accessor for the foreign key on Post
/// let rel: OneToMany<Post, i64> = OneToMany::new("posts")
///     .foreign_key("author_id");
/// ```
#[derive(Debug, Clone)]
pub struct OneToMany<T, K> {
	/// The name of the accessor on the source model
	pub accessor_name: String,
	/// The name of the foreign key field on the related model
	pub foreign_key_field: String,
	/// The name of the field this relationship points to (usually "id")
	pub to_field: String,
	/// Whether to use lazy loading by default
	pub lazy: bool,
	/// Phantom data for type parameters
	_phantom_t: PhantomData<T>,
	_phantom_k: PhantomData<K>,
}

impl<T, K> OneToMany<T, K> {
	/// Create a new one-to-many relationship
	///
	/// # Arguments
	///
	/// * `accessor_name` - The name of the accessor on the source model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToMany;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let rel: OneToMany<Post, i64> = OneToMany::new("posts");
	/// assert_eq!(rel.accessor_name(), "posts");
	/// ```
	pub fn new(accessor_name: impl Into<String>) -> Self {
		Self {
			accessor_name: accessor_name.into(),
			foreign_key_field: String::new(),
			to_field: "id".to_string(),
			lazy: true,
			_phantom_t: PhantomData,
			_phantom_k: PhantomData,
		}
	}

	/// Set the foreign key field name on the related model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToMany;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let rel: OneToMany<Post, i64> = OneToMany::new("posts")
	///     .foreign_key("author_id");
	/// assert_eq!(rel.foreign_key_field(), "author_id");
	/// ```
	pub fn foreign_key(mut self, field_name: impl Into<String>) -> Self {
		self.foreign_key_field = field_name.into();
		self
	}

	/// Set the field this relationship points to
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToMany;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let rel: OneToMany<Post, i64> = OneToMany::new("posts")
	///     .to_field("uuid");
	/// assert_eq!(rel.get_to_field(), "uuid");
	/// ```
	pub fn to_field(mut self, to_field: impl Into<String>) -> Self {
		self.to_field = to_field.into();
		self
	}

	/// Set whether to use lazy loading
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToMany;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let rel: OneToMany<Post, i64> = OneToMany::new("posts")
	///     .lazy(false);
	/// assert!(!rel.is_lazy());
	/// ```
	pub fn lazy(mut self, lazy: bool) -> Self {
		self.lazy = lazy;
		self
	}

	/// Get the accessor name
	pub fn accessor_name(&self) -> &str {
		&self.accessor_name
	}

	/// Get the foreign key field name
	pub fn foreign_key_field(&self) -> &str {
		&self.foreign_key_field
	}

	/// Get the to_field name
	pub fn get_to_field(&self) -> &str {
		&self.to_field
	}

	/// Check if lazy loading is enabled
	pub fn is_lazy(&self) -> bool {
		self.lazy
	}
}

impl<T, K> Default for OneToMany<T, K> {
	fn default() -> Self {
		Self::new("items")
	}
}

impl<T, K> ReverseRelationship for OneToMany<T, K> {
	/// Get the reverse accessor name (returns the accessor_name for OneToMany)
	///
	/// Note: For OneToMany, this returns the accessor_name itself since OneToMany
	/// is already the reverse side of a ForeignKey relationship.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{OneToMany, ReverseRelationship};
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let rel: OneToMany<Post, i64> = OneToMany::new("posts");
	/// // For OneToMany, it returns the accessor_name itself
	/// assert_eq!(rel.get_or_generate_reverse_name("User"), "posts");
	/// ```
	fn get_or_generate_reverse_name(&self, _model_name: &str) -> String {
		// OneToMany already represents the reverse side of a relationship
		// So we return the accessor_name directly
		self.accessor_name.clone()
	}

	fn explicit_reverse_name(&self) -> Option<&str> {
		// OneToMany always has an accessor_name, so this always returns Some
		Some(&self.accessor_name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct User {
		id: i64,
		name: String,
	}

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Post {
		id: i64,
		author_id: i64,
		title: String,
	}

	#[test]
	fn test_one_to_many_creation() {
		let rel: OneToMany<Post, i64> = OneToMany::new("posts");
		assert_eq!(rel.accessor_name(), "posts");
		assert_eq!(rel.foreign_key_field(), "");
		assert_eq!(rel.get_to_field(), "id");
		assert!(rel.is_lazy());
	}

	#[test]
	fn test_one_to_many_builder() {
		let rel: OneToMany<Post, i64> = OneToMany::new("posts")
			.foreign_key("author_id")
			.to_field("uuid")
			.lazy(false);

		assert_eq!(rel.accessor_name(), "posts");
		assert_eq!(rel.foreign_key_field(), "author_id");
		assert_eq!(rel.get_to_field(), "uuid");
		assert!(!rel.is_lazy());
	}

	#[test]
	fn test_foreign_key_field() {
		let rel: OneToMany<Post, i64> = OneToMany::new("posts").foreign_key("user_id");
		assert_eq!(rel.foreign_key_field(), "user_id");
	}

	#[test]
	fn test_to_field_customization() {
		let rel: OneToMany<Post, i64> = OneToMany::new("posts").to_field("user_uuid");
		assert_eq!(rel.get_to_field(), "user_uuid");
	}

	#[test]
	fn test_lazy_loading_configuration() {
		let rel1: OneToMany<Post, i64> = OneToMany::new("posts").lazy(true);
		assert!(rel1.is_lazy());

		let rel2: OneToMany<Post, i64> = OneToMany::new("posts").lazy(false);
		assert!(!rel2.is_lazy());
	}

	#[test]
	fn test_default_values() {
		let rel: OneToMany<Post, i64> = OneToMany::new("comments");
		assert_eq!(rel.accessor_name(), "comments");
		assert!(rel.is_lazy());
		assert_eq!(rel.get_to_field(), "id");
	}

	#[test]
	fn test_multiple_relationships() {
		let posts: OneToMany<Post, i64> = OneToMany::new("posts").foreign_key("author_id");
		let comments: OneToMany<Post, i64> = OneToMany::new("comments").foreign_key("user_id");

		assert_eq!(posts.accessor_name(), "posts");
		assert_eq!(posts.foreign_key_field(), "author_id");

		assert_eq!(comments.accessor_name(), "comments");
		assert_eq!(comments.foreign_key_field(), "user_id");
	}
}
