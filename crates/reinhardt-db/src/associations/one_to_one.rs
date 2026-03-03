//! One-to-One relationship definition
//!
//! Provides One-to-One relationship types for defining bidirectional unique
//! relationships between models.

use std::marker::PhantomData;

use super::foreign_key::CascadeAction;
use super::reverse::{ReverseRelationship, generate_reverse_accessor_singular};

/// One-to-One relationship field
///
/// Represents a unique one-to-one relationship between two models.
/// This is similar to ForeignKey but enforces uniqueness on the foreign key field.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
/// * `K` - The type of the primary key field
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::{OneToOne, CascadeAction};
///
/// #[derive(Clone)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// #[derive(Clone)]
/// struct UserProfile {
///     id: i64,
///     user_id: i64,
///     bio: String,
/// }
///
/// // Define one-to-one relationship
/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
///     .related_name("profile")
///     .on_delete(CascadeAction::Cascade);
/// ```
#[derive(Debug, Clone)]
pub struct OneToOne<T, K> {
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
	/// Database index creation (always true for one-to-one)
	pub db_index: bool,
	/// Database constraint name
	pub db_constraint: Option<String>,
	/// Whether this is the parent side of the relationship
	pub parent_link: bool,
	/// Phantom data for type parameters
	_phantom_t: PhantomData<T>,
	_phantom_k: PhantomData<K>,
}

impl<T, K> OneToOne<T, K> {
	/// Create a new one-to-one relationship field
	///
	/// # Arguments
	///
	/// * `field_name` - The name of the foreign key field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id");
	/// assert_eq!(rel.get_field_name(), "user_id");
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
			parent_link: false,
			_phantom_t: PhantomData,
			_phantom_k: PhantomData,
		}
	}

	/// Set the related field name on the target model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .to_field("uuid");
	/// assert_eq!(rel.get_to_field(), "uuid");
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
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .related_name("profile");
	/// assert_eq!(rel.get_related_name(), Some("profile"));
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
	/// use reinhardt_db::associations::{OneToOne, CascadeAction};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .on_delete(CascadeAction::Cascade);
	/// assert_eq!(rel.get_on_delete(), CascadeAction::Cascade);
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
	/// use reinhardt_db::associations::{OneToOne, CascadeAction};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .on_update(CascadeAction::SetNull);
	/// assert_eq!(rel.get_on_update(), CascadeAction::SetNull);
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
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .null(true);
	/// assert!(rel.is_null());
	/// ```
	pub fn null(mut self, null: bool) -> Self {
		self.null = null;
		self
	}

	/// Set the database constraint name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .db_constraint("fk_profile_user");
	/// assert_eq!(rel.get_db_constraint(), Some("fk_profile_user"));
	/// ```
	pub fn db_constraint(mut self, name: impl Into<String>) -> Self {
		self.db_constraint = Some(name.into());
		self
	}

	/// Set whether this is a parent link (for model inheritance)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::OneToOne;
	///
	/// #[derive(Clone)]
	/// struct BaseModel {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<BaseModel, i64> = OneToOne::new("base_ptr_id")
	///     .parent_link(true);
	/// assert!(rel.is_parent_link());
	/// ```
	pub fn parent_link(mut self, parent_link: bool) -> Self {
		self.parent_link = parent_link;
		self
	}

	/// Get the field name
	pub fn get_field_name(&self) -> &str {
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

	/// Check if database index should be created (always true for one-to-one)
	pub fn has_db_index(&self) -> bool {
		self.db_index
	}

	/// Get the database constraint name
	pub fn get_db_constraint(&self) -> Option<&str> {
		self.db_constraint.as_deref()
	}

	/// Check if this is a parent link
	pub fn is_parent_link(&self) -> bool {
		self.parent_link
	}
}

impl<T, K> Default for OneToOne<T, K> {
	fn default() -> Self {
		Self::new("id")
	}
}

impl<T, K> ReverseRelationship for OneToOne<T, K> {
	/// Get the reverse accessor name, generating one if not explicitly set
	///
	/// For one-to-one relationships, generates a singular accessor name.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{OneToOne, ReverseRelationship};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let rel: OneToOne<User, i64> = OneToOne::new("user_id");
	/// assert_eq!(rel.get_or_generate_reverse_name("UserProfile"), "user_profile");
	///
	/// let rel_with_name: OneToOne<User, i64> = OneToOne::new("user_id")
	///     .related_name("profile");
	/// assert_eq!(rel_with_name.get_or_generate_reverse_name("UserProfile"), "profile");
	/// ```
	fn get_or_generate_reverse_name(&self, model_name: &str) -> String {
		self.related_name
			.clone()
			.unwrap_or_else(|| generate_reverse_accessor_singular(model_name))
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

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct UserProfile {
		id: i64,
		user_id: i64,
		bio: String,
	}

	#[test]
	fn test_one_to_one_creation() {
		let rel: OneToOne<User, i64> = OneToOne::new("user_id");
		assert_eq!(rel.get_field_name(), "user_id");
		assert_eq!(rel.get_to_field(), "id");
		assert_eq!(rel.get_related_name(), None);
		assert_eq!(rel.get_on_delete(), CascadeAction::NoAction);
		assert_eq!(rel.get_on_update(), CascadeAction::NoAction);
		assert!(!rel.is_null());
		assert!(rel.has_db_index());
		assert!(!rel.is_parent_link());
	}

	#[test]
	fn test_one_to_one_builder() {
		let rel: OneToOne<User, i64> = OneToOne::new("user_id")
			.related_name("profile")
			.on_delete(CascadeAction::Cascade)
			.on_update(CascadeAction::SetNull)
			.null(true)
			.db_constraint("fk_profile_user")
			.parent_link(true);

		assert_eq!(rel.get_field_name(), "user_id");
		assert_eq!(rel.get_related_name(), Some("profile"));
		assert_eq!(rel.get_on_delete(), CascadeAction::Cascade);
		assert_eq!(rel.get_on_update(), CascadeAction::SetNull);
		assert!(rel.is_null());
		assert!(rel.has_db_index());
		assert_eq!(rel.get_db_constraint(), Some("fk_profile_user"));
		assert!(rel.is_parent_link());
	}

	#[test]
	fn test_to_field_customization() {
		let rel: OneToOne<User, i64> = OneToOne::new("user_id").to_field("uuid");
		assert_eq!(rel.get_to_field(), "uuid");
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
			let rel: OneToOne<User, i64> = OneToOne::new("user_id").on_delete(action);
			assert_eq!(rel.get_on_delete(), action);
		}
	}

	#[test]
	fn test_null_configuration() {
		let rel1: OneToOne<User, i64> = OneToOne::new("user_id").null(true);
		assert!(rel1.is_null());

		let rel2: OneToOne<User, i64> = OneToOne::new("user_id").null(false);
		assert!(!rel2.is_null());
	}

	#[test]
	fn test_parent_link() {
		let rel1: OneToOne<User, i64> = OneToOne::new("base_ptr_id").parent_link(true);
		assert!(rel1.is_parent_link());

		let rel2: OneToOne<User, i64> = OneToOne::new("user_id").parent_link(false);
		assert!(!rel2.is_parent_link());
	}

	#[test]
	fn test_db_index_always_true() {
		// One-to-one relationships always have a database index
		let rel: OneToOne<User, i64> = OneToOne::new("user_id");
		assert!(rel.has_db_index());
	}
}
