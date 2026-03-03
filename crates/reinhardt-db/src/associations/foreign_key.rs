//! Foreign key relationship definition
//!
//! Provides Foreign Key relationship types for defining one-to-many and many-to-one
//! relationships between models.

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use super::reverse::{ReverseRelationship, generate_reverse_accessor};

/// Cascade action when the referenced object is deleted or updated
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::CascadeAction;
///
/// let action = CascadeAction::Cascade;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CascadeAction {
	/// Do nothing (default behavior, may cause constraint violations)
	#[default]
	NoAction,
	/// Restrict deletion/update if dependent objects exist
	Restrict,
	/// Set foreign key to NULL when referenced object is deleted/updated
	SetNull,
	/// Set foreign key to its default value
	SetDefault,
	/// Cascade deletion/update to dependent objects
	Cascade,
}

/// Foreign key field configuration
///
/// # Type Parameters
///
/// * `T` - The type of the referenced model
/// * `K` - The type of the foreign key field
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::{ForeignKey, CascadeAction};
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
///     title: String,
///     author_id: i64,
/// }
///
/// // Define foreign key relationship
/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
///     .related_name("posts")
///     .on_delete(CascadeAction::Cascade);
/// ```
#[derive(Debug, Clone)]
pub struct ForeignKey<T, K> {
	/// The name of the foreign key field
	pub field_name: String,
	/// The name of the related field on the target model (usually "id")
	pub to_field: String,
	/// The name of the reverse relation accessor on the target model
	pub related_name: Option<String>,
	/// Action to take when referenced object is deleted
	pub on_delete: CascadeAction,
	/// Action to take when referenced object is updated
	pub on_update: CascadeAction,
	/// Whether the foreign key can be null
	pub null: bool,
	/// Database index creation
	pub db_index: bool,
	/// Database constraint name
	pub db_constraint: Option<String>,
	/// Phantom data for type parameters
	_phantom_t: PhantomData<T>,
	_phantom_k: PhantomData<K>,
}

impl<T, K> ForeignKey<T, K> {
	/// Create a new foreign key field
	///
	/// # Arguments
	///
	/// * `field_name` - The name of the foreign key field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("user_id");
	/// assert_eq!(fk.field_name(), "user_id");
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			to_field: "id".to_string(),
			related_name: None,
			on_delete: CascadeAction::default(),
			on_update: CascadeAction::default(),
			null: false,
			db_index: true,
			db_constraint: None,
			_phantom_t: PhantomData,
			_phantom_k: PhantomData,
		}
	}

	/// Set the related field name on the target model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .to_field("user_id");
	/// assert_eq!(fk.get_to_field(), "user_id");
	/// ```
	pub fn to_field(mut self, to_field: impl Into<String>) -> Self {
		self.to_field = to_field.into();
		self
	}

	/// Set the reverse relation accessor name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .related_name("posts");
	/// assert_eq!(fk.get_related_name(), Some("posts"));
	/// ```
	pub fn related_name(mut self, name: impl Into<String>) -> Self {
		self.related_name = Some(name.into());
		self
	}

	/// Set the on_delete cascade action
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{ForeignKey, CascadeAction};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .on_delete(CascadeAction::Cascade);
	/// assert_eq!(fk.get_on_delete(), CascadeAction::Cascade);
	/// ```
	pub fn on_delete(mut self, action: CascadeAction) -> Self {
		self.on_delete = action;
		self
	}

	/// Set the on_update cascade action
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{ForeignKey, CascadeAction};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .on_update(CascadeAction::Cascade);
	/// assert_eq!(fk.get_on_update(), CascadeAction::Cascade);
	/// ```
	pub fn on_update(mut self, action: CascadeAction) -> Self {
		self.on_update = action;
		self
	}

	/// Set whether the foreign key can be null
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .null(true);
	/// assert!(fk.is_null());
	/// ```
	pub fn null(mut self, null: bool) -> Self {
		self.null = null;
		self
	}

	/// Set whether to create database index
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .db_index(false);
	/// assert!(!fk.has_db_index());
	/// ```
	pub fn db_index(mut self, db_index: bool) -> Self {
		self.db_index = db_index;
		self
	}

	/// Set the database constraint name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ForeignKey;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .db_constraint("fk_posts_author");
	/// assert_eq!(fk.get_db_constraint(), Some("fk_posts_author"));
	/// ```
	pub fn db_constraint(mut self, name: impl Into<String>) -> Self {
		self.db_constraint = Some(name.into());
		self
	}

	/// Get the field name
	pub fn field_name(&self) -> &str {
		&self.field_name
	}

	/// Get the to_field name
	pub fn get_to_field(&self) -> &str {
		&self.to_field
	}

	/// Get the related_name
	pub fn get_related_name(&self) -> Option<&str> {
		self.related_name.as_deref()
	}

	/// Get the on_delete action
	pub fn get_on_delete(&self) -> CascadeAction {
		self.on_delete
	}

	/// Get the on_update action
	pub fn get_on_update(&self) -> CascadeAction {
		self.on_update
	}

	/// Check if null is allowed
	pub fn is_null(&self) -> bool {
		self.null
	}

	/// Check if database index should be created
	pub fn has_db_index(&self) -> bool {
		self.db_index
	}

	/// Get the database constraint name
	pub fn get_db_constraint(&self) -> Option<&str> {
		self.db_constraint.as_deref()
	}
}

impl<T, K> Default for ForeignKey<T, K> {
	fn default() -> Self {
		Self::new("id")
	}
}

impl<T, K> ReverseRelationship for ForeignKey<T, K> {
	/// Get the reverse accessor name, generating one if not explicitly set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{ForeignKey, ReverseRelationship};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let fk: ForeignKey<User, i64> = ForeignKey::new("author_id");
	/// assert_eq!(fk.get_or_generate_reverse_name("Post"), "post_set");
	///
	/// let fk_with_name: ForeignKey<User, i64> = ForeignKey::new("author_id")
	///     .related_name("posts");
	/// assert_eq!(fk_with_name.get_or_generate_reverse_name("Post"), "posts");
	/// ```
	fn get_or_generate_reverse_name(&self, model_name: &str) -> String {
		self.related_name
			.clone()
			.unwrap_or_else(|| generate_reverse_accessor(model_name))
	}

	fn explicit_reverse_name(&self) -> Option<&str> {
		self.related_name.as_deref()
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

	#[test]
	fn test_foreign_key_creation() {
		let fk: ForeignKey<User, i64> = ForeignKey::new("author_id");
		assert_eq!(fk.field_name(), "author_id");
		assert_eq!(fk.get_to_field(), "id");
		assert_eq!(fk.get_related_name(), None);
		assert_eq!(fk.get_on_delete(), CascadeAction::NoAction);
		assert_eq!(fk.get_on_update(), CascadeAction::NoAction);
		assert!(!fk.is_null());
		assert!(fk.has_db_index());
	}

	#[test]
	fn test_foreign_key_builder() {
		let fk: ForeignKey<User, i64> = ForeignKey::new("author_id")
			.related_name("posts")
			.on_delete(CascadeAction::Cascade)
			.on_update(CascadeAction::SetNull)
			.null(true)
			.db_index(false)
			.db_constraint("fk_posts_author");

		assert_eq!(fk.field_name(), "author_id");
		assert_eq!(fk.get_related_name(), Some("posts"));
		assert_eq!(fk.get_on_delete(), CascadeAction::Cascade);
		assert_eq!(fk.get_on_update(), CascadeAction::SetNull);
		assert!(fk.is_null());
		assert!(!fk.has_db_index());
		assert_eq!(fk.get_db_constraint(), Some("fk_posts_author"));
	}

	#[test]
	fn test_cascade_action_default() {
		assert_eq!(CascadeAction::default(), CascadeAction::NoAction);
	}

	#[test]
	fn test_cascade_actions() {
		let actions = vec![
			CascadeAction::NoAction,
			CascadeAction::Restrict,
			CascadeAction::SetNull,
			CascadeAction::SetDefault,
			CascadeAction::Cascade,
		];

		for action in actions {
			let fk: ForeignKey<User, i64> = ForeignKey::new("test_id").on_delete(action);
			assert_eq!(fk.get_on_delete(), action);
		}
	}

	#[test]
	fn test_to_field_customization() {
		let fk: ForeignKey<User, i64> = ForeignKey::new("author_id").to_field("user_id");
		assert_eq!(fk.get_to_field(), "user_id");
	}

	#[test]
	fn test_null_configuration() {
		let fk1: ForeignKey<User, i64> = ForeignKey::new("author_id").null(true);
		assert!(fk1.is_null());

		let fk2: ForeignKey<User, i64> = ForeignKey::new("author_id").null(false);
		assert!(!fk2.is_null());
	}

	#[test]
	fn test_db_index_configuration() {
		let fk1: ForeignKey<User, i64> = ForeignKey::new("author_id").db_index(true);
		assert!(fk1.has_db_index());

		let fk2: ForeignKey<User, i64> = ForeignKey::new("author_id").db_index(false);
		assert!(!fk2.has_db_index());
	}
}
