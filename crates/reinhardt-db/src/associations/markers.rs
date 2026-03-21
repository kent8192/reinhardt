//! Marker types for relationship field annotations.
//!
//! These types are used as field types to indicate relationship definitions
//! when using the `#[rel]` attribute macro.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_db::orm::Model;
//! use reinhardt_macros::model;
//! use reinhardt_db::associations::ManyToManyField;
//! use uuid::Uuid;
//!
//! #[model(app_label = "users")]
//! pub struct User {
//!     #[field(primary_key = true)]
//!     pub id: Uuid,
//!
//!     // User -> User relationship (self-referential)
//!     #[rel(many_to_many, related_name = "followers")]
//!     pub following: ManyToManyField<User, User>,
//! }
//!
//! #[model(app_label = "users")]
//! pub struct Group {
//!     #[field(primary_key = true)]
//!     pub id: Uuid,
//!
//!     // Group -> User relationship
//!     #[rel(many_to_many, related_name = "groups")]
//!     pub members: ManyToManyField<Group, User>,
//! }
//! ```

use std::marker::PhantomData;

// Import DatabaseConnection and QueryRow for method signatures
use crate::orm::{DatabaseConnection, QueryRow};

/// Configuration for ManyToMany relationship operations
///
/// This struct groups together the parameters needed to identify and work with
/// a ManyToMany relationship through a junction table.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ManyToManyConfig<PK> {
	/// Primary key of the source instance
	pub source_pk: PK,
	/// Name of the junction table
	pub through_table: String,
	/// Column name for the source foreign key in junction table
	pub source_field: String,
	/// Column name for the target foreign key in junction table
	pub target_field: String,
}

impl<PK> ManyToManyConfig<PK> {
	/// Create a new ManyToManyConfig
	pub fn new(
		source_pk: PK,
		through_table: String,
		source_field: String,
		target_field: String,
	) -> Self {
		Self {
			source_pk,
			through_table,
			source_field,
			target_field,
		}
	}
}

/// Marker type for ManyToMany relationship fields.
///
/// This type is used as the field type for `#[rel(many_to_many, ...)]` attributes.
/// It indicates that the field represents a many-to-many relationship.
///
/// The type parameters:
/// - `Source`: The source model type (the model containing this field)
/// - `Target`: The target model type (the related model)
/// - `S`: Relationship metadata type (defaults to `()`)
///
/// The `S` parameter allows storing relationship-specific metadata at the type level,
/// such as through-table configuration, ordering, or custom relationship behavior.
///
/// The intermediate table is automatically generated based on the source and target types:
/// - Table name: `{app_label}_{source_table}_{field_name}`
/// - Source FK: `{source_table}_id`
/// - Target FK: `{target_table}_id`
///
/// # Example
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_db::orm::Model;
/// use reinhardt_macros::model;
/// use reinhardt_db::associations::ManyToManyField;
/// use uuid::Uuid;
///
/// #[model(app_label = "auth", table_name = "users")]
/// pub struct User {
///     #[field(primary_key = true)]
///     pub id: Uuid,
///
///     // User -> Group relationship
///     #[rel(many_to_many, related_name = "members")]
///     pub groups: ManyToManyField<User, Group>,
///
///     // Self-referential relationship (following)
///     #[rel(many_to_many, related_name = "followers")]
///     pub following: ManyToManyField<User, User>,
/// }
///
/// // With custom metadata type:
/// struct OrderedRelation;
///
/// #[model(app_label = "project", table_name = "tasks")]
/// pub struct Task {
///     #[rel(many_to_many)]
///     pub tags: ManyToManyField<Task, Tag, OrderedRelation>,
/// }
///
/// // Usage with ManyToManyAccessor:
/// use reinhardt_db::orm::ManyToManyAccessor;
///
/// let user = User::find_by_id(&db, user_id).await?;
/// let accessor = ManyToManyAccessor::new(&user, "groups", ());
///
/// // Add relationship
/// accessor.add(&group).await?;
///
/// // Get all related
/// let groups = accessor.all().await?;
///
/// // Remove relationship
/// accessor.remove(&group).await?;
///
/// // Clear all
/// accessor.clear().await?;
///
/// # Ok(())
/// # }
/// ```
/// Marker type for ManyToMany relationship.
///
/// `Default` is implemented manually to avoid requiring `Source` and `Target` to implement `Default`.
/// This is because the derive macro would add `Source: Default, Target: Default` bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
pub struct ManyToManyField<Source, Target, S = ()>(PhantomData<(Source, Target, S)>);

impl<Source, Target, S> Default for ManyToManyField<Source, Target, S> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<Source, Target, S> ManyToManyField<Source, Target, S> {
	/// Creates a new ManyToManyField marker.
	///
	/// This constructor hides the internal `PhantomData` implementation,
	/// providing a clean API for users without exposing implementation details.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToManyField;
	///
	/// struct User;
	/// struct Tag;
	///
	/// // User -> Tag relationship with default metadata type ()
	/// let field: ManyToManyField<User, Tag> = ManyToManyField::new();
	///
	/// // Self-referential relationship
	/// let following: ManyToManyField<User, User> = ManyToManyField::new();
	///
	/// // Custom metadata type
	/// struct CustomMetadata;
	/// let field_with_metadata: ManyToManyField<User, Tag, CustomMetadata> = ManyToManyField::new();
	/// ```
	#[inline]
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl<Source, Target, S> ManyToManyField<Source, Target, S>
where
	Source: std::fmt::Display,
	Target: std::fmt::Display,
{
	/// Get a manager for this ManyToMany relationship
	///
	/// This is an internal helper method that creates a `ManyToManyManager`
	/// with the appropriate configuration for this field.
	///
	/// # Arguments
	///
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	///
	/// # Type Parameters
	///
	/// * `PK` - Primary key type (must implement Display and Clone)
	fn get_manager<PK>(
		&self,
		config: ManyToManyConfig<PK>,
	) -> crate::prelude::many_to_many_manager::ManyToManyManager<Source, Target, PK>
	where
		PK: std::fmt::Display + Clone,
	{
		crate::prelude::many_to_many_manager::ManyToManyManager::new(
			config.source_pk,
			config.through_table,
			config.source_field,
			config.target_field,
		)
	}

	/// Add a target instance to this ManyToMany relationship
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	/// * `target_pk` - Primary key of the target instance to add
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_db::associations::ManyToManyConfig;
	///
	/// // Add a user to a group
	/// let config = ManyToManyConfig::new(
	///     group.id,
	///     "auth_group_members".to_string(),
	///     "group_id".to_string(),
	///     "user_id".to_string(),
	/// );
	/// group.members.add_with_db(&db, config, user.id).await?;
	/// ```
	pub async fn add_with_db<PK, TPK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
		target_pk: TPK,
	) -> reinhardt_core::exception::Result<()>
	where
		PK: std::fmt::Display + Clone,
		TPK: std::fmt::Display,
	{
		self.get_manager(config).add_with_db(conn, target_pk).await
	}

	/// Remove a target instance from this ManyToMany relationship
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	/// * `target_pk` - Primary key of the target instance to remove
	pub async fn remove_with_db<PK, TPK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
		target_pk: TPK,
	) -> reinhardt_core::exception::Result<()>
	where
		PK: std::fmt::Display + Clone,
		TPK: std::fmt::Display,
	{
		self.get_manager(config)
			.remove_with_db(conn, target_pk)
			.await
	}

	/// Check if a target instance exists in this ManyToMany relationship
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	/// * `target_pk` - Primary key of the target instance to check
	///
	/// # Returns
	///
	/// * `Ok(true)` if the relationship exists
	/// * `Ok(false)` if not
	pub async fn contains_with_db<PK, TPK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
		target_pk: TPK,
	) -> reinhardt_core::exception::Result<bool>
	where
		PK: std::fmt::Display + Clone,
		TPK: std::fmt::Display,
	{
		self.get_manager(config)
			.contains_with_db(conn, target_pk)
			.await
	}

	/// Get all target instances in this ManyToMany relationship
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	/// * `target_table` - Name of the target table
	/// * `target_pk_field` - Name of primary key column in target table
	///
	/// # Returns
	///
	/// Vector of QueryRow representing all related target instances
	///
	/// Note: Returns `Vec<QueryRow>` instead of `Vec<Target>` for flexibility.
	/// Callers can deserialize QueryRow to their target type as needed.
	pub async fn all_with_db<PK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
		target_table: &str,
		target_pk_field: &str,
	) -> reinhardt_core::exception::Result<Vec<QueryRow>>
	where
		PK: std::fmt::Display + Clone,
	{
		self.get_manager(config)
			.all_with_db(conn, target_table, target_pk_field)
			.await
	}

	/// Clear all relationships for the source instance
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	pub async fn clear_with_db<PK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
	) -> reinhardt_core::exception::Result<()>
	where
		PK: std::fmt::Display + Clone,
	{
		self.get_manager(config).clear_with_db(conn).await
	}

	/// Count the number of related target instances
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `config` - Configuration containing source_pk, through_table, source_field, and target_field
	///
	/// # Returns
	///
	/// Number of related instances
	pub async fn count_with_db<PK>(
		&self,
		conn: &DatabaseConnection,
		config: ManyToManyConfig<PK>,
	) -> reinhardt_core::exception::Result<usize>
	where
		PK: std::fmt::Display + Clone,
	{
		self.get_manager(config).count_with_db(conn).await
	}
}

/// Marker type for ForeignKey relationship fields.
///
/// This type is used as the field type for `#[rel(foreign_key, ...)]` attributes.
/// It represents a many-to-one relationship where multiple instances of the
/// containing model can reference a single instance of `T`.
///
/// The type parameter:
/// - `T`: The target model type (the related model)
///
/// When using this type, the macro automatically:
/// - Generates a `{field_name}_id` column for the foreign key value
/// - Infers the target model from `T` (no need for `to = Model` attribute)
/// - Generates lazy load accessor method `{field_name}(&self, conn)` -> `Result<Option<T>>`
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::associations::ForeignKeyField;
/// use reinhardt_macros::model;
///
/// #[model(app_label = "blog", table_name = "posts")]
/// pub struct Post {
///     #[field(primary_key = true)]
///     pub id: i64,
///
///     // Generates `author_id` column automatically
///     #[rel(foreign_key, related_name = "posts")]
///     pub author: ForeignKeyField<User>,
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
pub struct ForeignKeyField<T>(PhantomData<T>);

impl<T> Default for ForeignKeyField<T> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<T> ForeignKeyField<T> {
	/// Creates a new ForeignKeyField marker.
	///
	/// This constructor hides the internal `PhantomData` implementation.
	#[inline]
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

/// Marker type for OneToOne relationship fields.
///
/// This type is used as the field type for `#[rel(one_to_one, ...)]` attributes.
/// It represents a one-to-one relationship where each instance of the
/// containing model references exactly one instance of `T`.
///
/// The type parameter:
/// - `T`: The target model type (the related model)
///
/// When using this type, the macro automatically:
/// - Generates a `{field_name}_id` column for the foreign key value
/// - Infers the target model from `T` (no need for `to = Model` attribute)
/// - Generates lazy load accessor method `{field_name}(&self, conn)` -> `Result<Option<T>>`
/// - Adds UNIQUE constraint to the generated column
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::associations::OneToOneField;
/// use reinhardt_macros::model;
///
/// #[model(app_label = "auth", table_name = "profiles")]
/// pub struct Profile {
///     #[field(primary_key = true)]
///     pub id: i64,
///
///     // Generates `user_id` column with UNIQUE constraint
///     #[rel(one_to_one, related_name = "profile")]
///     pub user: OneToOneField<User>,
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
pub struct OneToOneField<T>(PhantomData<T>);

impl<T> Default for OneToOneField<T> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<T> OneToOneField<T> {
	/// Creates a new OneToOneField marker.
	///
	/// This constructor hides the internal `PhantomData` implementation.
	#[inline]
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

/// Marker type for OneToMany relationship fields (reverse of ForeignKey).
///
/// This type is used as the field type for `#[rel(one_to_many, ...)]` attributes.
/// It represents the reverse side of a ForeignKey relationship.
///
/// The type parameters:
/// - `T`: The related model type
/// - `S`: Relationship metadata type (defaults to `()`)
///
/// # Example
///
/// ```rust,ignore
/// // On User model
/// #[rel(one_to_many, foreign_key = "author_id")]
/// pub posts: OneToManyField<Post>,
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OneToManyField<T, S = ()>(PhantomData<(T, S)>);

impl<T, S> OneToManyField<T, S> {
	/// Creates a new OneToManyField marker.
	///
	/// This constructor hides the internal `PhantomData` implementation.
	#[inline]
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

/// Marker type for PolymorphicManyToMany relationship fields.
///
/// This type is used as the field type for `#[rel(polymorphic_many_to_many, ...)]` attributes.
/// It indicates a polymorphic many-to-many relationship.
///
/// The type parameters:
/// - `K`: The key type (usually `i64` or `Uuid`)
/// - `S`: Relationship metadata type (defaults to `()`)
///
/// # Example
///
/// ```rust,ignore
/// #[rel(polymorphic_many_to_many, name = "taggable")]
/// pub tags: PolymorphicManyToManyField<Uuid>,
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PolymorphicManyToManyField<K, S = ()>(PhantomData<(K, S)>);

impl<K, S> PolymorphicManyToManyField<K, S> {
	/// Creates a new PolymorphicManyToManyField marker.
	///
	/// This constructor hides the internal `PhantomData` implementation.
	#[inline]
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_many_to_many_field_creation() {
		struct User;
		struct Group;
		// User -> Group relationship
		let _field: ManyToManyField<User, Group> = ManyToManyField::new();
	}

	#[test]
	fn test_many_to_many_field_self_referential() {
		struct User;
		// User -> User (self-referential, e.g., following)
		let _field: ManyToManyField<User, User> = ManyToManyField::new();
	}

	#[test]
	fn test_one_to_many_field_creation() {
		struct Post;
		let _field: OneToManyField<Post> = OneToManyField::new();
	}

	#[test]
	fn test_polymorphic_many_to_many_field_creation() {
		let _field: PolymorphicManyToManyField<i64> = PolymorphicManyToManyField::new();
	}

	#[test]
	fn test_default_impl() {
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
		struct Article;
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
		struct Tag;
		// Article -> Tag relationship
		let field1: ManyToManyField<Article, Tag> = ManyToManyField::default();
		let field2: ManyToManyField<Article, Tag> = ManyToManyField::new();
		assert_eq!(field1, field2);
	}

	#[test]
	fn test_many_to_many_field_with_metadata() {
		struct User;
		struct Tag;
		struct OrderedRelation;

		// Default metadata type ()
		let _field1: ManyToManyField<User, Tag> = ManyToManyField::new();

		// Custom metadata type
		let _field2: ManyToManyField<User, Tag, OrderedRelation> = ManyToManyField::new();
	}

	#[test]
	fn test_foreign_key_field_creation() {
		struct User;
		let _field: ForeignKeyField<User> = ForeignKeyField::new();
	}

	#[test]
	fn test_foreign_key_field_default() {
		#[derive(Debug, PartialEq)]
		struct User;
		let field1: ForeignKeyField<User> = ForeignKeyField::default();
		let field2: ForeignKeyField<User> = ForeignKeyField::new();
		assert_eq!(field1, field2);
	}

	#[test]
	fn test_one_to_one_field_creation() {
		struct User;
		let _field: OneToOneField<User> = OneToOneField::new();
	}

	#[test]
	fn test_one_to_one_field_default() {
		#[derive(Debug, PartialEq)]
		struct User;
		let field1: OneToOneField<User> = OneToOneField::default();
		let field2: OneToOneField<User> = OneToOneField::new();
		assert_eq!(field1, field2);
	}
}
