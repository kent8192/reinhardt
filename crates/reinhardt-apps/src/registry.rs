//! Global model registry
//!
//! This module provides a global registry for models, allowing models to be
//! discovered and registered at compile time using the `linkme` crate.
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_apps::registry::{ModelMetadata, get_registered_models};
//!
//! // Register a model (typically done via derive macro)
//! #[linkme::distributed_slice(reinhardt_apps::registry::MODELS)]
//! static MY_MODEL: ModelMetadata = ModelMetadata {
//!     app_label: "myapp",
//!     model_name: "User",
//!     table_name: "users",
//! };
//!
//! // Access registered models
//! let models = get_registered_models();
//! // Note: In doc tests, the model may not be visible due to linkme limitations
//! ```

use linkme::distributed_slice;
use std::collections::HashMap;
use std::sync::{OnceLock, PoisonError, RwLock};

/// Metadata for a registered model
///
/// This structure contains essential information about a model that has been
/// registered in the global model registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMetadata {
	/// The label of the application this model belongs to
	pub app_label: &'static str,

	/// The name of the model (e.g., "User", "Post")
	pub model_name: &'static str,

	/// The database table name for this model
	pub table_name: &'static str,
}

impl ModelMetadata {
	/// Create a new model metadata instance
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::registry::ModelMetadata;
	///
	/// let metadata = ModelMetadata::new("myapp", "User", "users");
	/// assert_eq!(metadata.app_label, "myapp");
	/// assert_eq!(metadata.model_name, "User");
	/// assert_eq!(metadata.table_name, "users");
	/// ```
	pub const fn new(
		app_label: &'static str,
		model_name: &'static str,
		table_name: &'static str,
	) -> Self {
		Self {
			app_label,
			model_name,
			table_name,
		}
	}

	/// Get the fully qualified model name (app_label.model_name)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::registry::ModelMetadata;
	///
	/// let metadata = ModelMetadata::new("myapp", "User", "users");
	/// assert_eq!(metadata.qualified_name(), "myapp.User");
	/// ```
	pub fn qualified_name(&self) -> String {
		format!("{}.{}", self.app_label, self.model_name)
	}
}

/// Global distributed slice for model registration
///
/// This is the global registry where all models are collected at link time.
/// Models can be registered by adding items to this slice using the `#[distributed_slice]`
/// attribute from the `linkme` crate.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::{MODELS, ModelMetadata};
///
/// #[linkme::distributed_slice(MODELS)]
/// static MY_MODEL: ModelMetadata = ModelMetadata {
///     app_label: "myapp",
///     model_name: "User",
///     table_name: "users",
/// };
/// ```
#[distributed_slice]
pub static MODELS: [ModelMetadata];

/// Cache for model lookups by app label.
/// Lazily initialized on first access and immutable thereafter.
static MODEL_CACHE: OnceLock<HashMap<&'static str, Vec<&'static ModelMetadata>>> = OnceLock::new();

/// Returns the cached model metadata indexed by app label.
fn model_cache() -> &'static HashMap<&'static str, Vec<&'static ModelMetadata>> {
	MODEL_CACHE.get_or_init(|| {
		let mut cache: HashMap<&'static str, Vec<&'static ModelMetadata>> = HashMap::new();
		for model in MODELS.iter() {
			cache.entry(model.app_label).or_default().push(model);
		}
		cache
	})
}

/// Get all registered models
///
/// This function returns a slice of all models that have been registered
/// in the global model registry.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::get_registered_models;
///
/// let models = get_registered_models();
/// println!("Found {} registered models", models.len());
/// ```
pub fn get_registered_models() -> &'static [ModelMetadata] {
	&MODELS
}

/// Get models for a specific application.
///
/// This function returns all models that belong to the specified application label.
/// Results are cached for performance on subsequent calls.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::get_models_for_app;
///
/// let auth_models = get_models_for_app("auth");
/// for model in auth_models {
///     println!("Model: {}", model.model_name);
/// }
/// ```
pub fn get_models_for_app(app_label: &str) -> Vec<&'static ModelMetadata> {
	model_cache().get(app_label).cloned().unwrap_or_default()
}

/// Find a model by its qualified name (app_label.model_name)
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::find_model;
///
/// if let Some(model) = find_model("myapp.User") {
///     println!("Found model: {}", model.model_name);
/// } else {
///     println!("Model not found");
/// }
/// ```
pub fn find_model(qualified_name: &str) -> Option<&'static ModelMetadata> {
	let parts: Vec<&str> = qualified_name.split('.').collect();
	if parts.len() != 2 {
		return None;
	}

	let (app_label, model_name) = (parts[0], parts[1]);
	MODELS
		.iter()
		.find(|m| m.app_label == app_label && m.model_name == model_name)
}

/// Metadata for a forward relationship
///
/// This structure contains information about a relationship field defined in a model.
/// It is populated at compile time via derive macros and stored in the global RELATIONSHIPS slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipMetadata {
	/// The model that owns this relationship (e.g., "blog.Post")
	pub from_model: &'static str,

	/// The target model of this relationship (e.g., "auth.User")
	pub to_model: &'static str,

	/// The type of relationship
	pub relationship_type: RelationshipType,

	/// The field name in the source model (e.g., "author", "tags")
	pub field_name: &'static str,

	/// The related name for reverse access (e.g., "posts", "authored_posts")
	/// If None, will be auto-generated as "{model_name}_set"
	pub related_name: Option<&'static str>,

	/// The database column name (for ForeignKey fields)
	/// None for ManyToMany relationships
	pub db_column: Option<&'static str>,

	/// The through table name (for ManyToMany relationships)
	/// None for ForeignKey and OneToOne relationships
	pub through_table: Option<&'static str>,
}

/// Type of relationship between models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
	/// Foreign key relationship (Many-to-One from the perspective of the field)
	ForeignKey,
	/// Many-to-Many relationship
	ManyToMany,
	/// One-to-One relationship
	OneToOne,
}

impl RelationshipMetadata {
	/// Create a new relationship metadata instance
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::registry::{RelationshipMetadata, RelationshipType};
	///
	/// let relationship = RelationshipMetadata::new(
	///     "blog.Post",
	///     "auth.User",
	///     RelationshipType::ForeignKey,
	///     "author",
	///     Some("posts"),
	///     Some("author_id"),
	///     None,
	/// );
	/// assert_eq!(relationship.field_name, "author");
	/// ```
	pub const fn new(
		from_model: &'static str,
		to_model: &'static str,
		relationship_type: RelationshipType,
		field_name: &'static str,
		related_name: Option<&'static str>,
		db_column: Option<&'static str>,
		through_table: Option<&'static str>,
	) -> Self {
		Self {
			from_model,
			to_model,
			relationship_type,
			field_name,
			related_name,
			db_column,
			through_table,
		}
	}

	/// Get the source model name without app label
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::registry::{RelationshipMetadata, RelationshipType};
	///
	/// let relationship = RelationshipMetadata::new(
	///     "blog.Post",
	///     "auth.User",
	///     RelationshipType::ForeignKey,
	///     "author",
	///     None,
	///     None,
	///     None,
	/// );
	/// assert_eq!(relationship.from_model_name(), "Post");
	/// ```
	pub fn from_model_name(&self) -> &str {
		self.from_model
			.split('.')
			.next_back()
			.unwrap_or(self.from_model)
	}

	/// Get the target model name without app label
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::registry::{RelationshipMetadata, RelationshipType};
	///
	/// let relationship = RelationshipMetadata::new(
	///     "blog.Post",
	///     "auth.User",
	///     RelationshipType::ForeignKey,
	///     "author",
	///     None,
	///     None,
	///     None,
	/// );
	/// assert_eq!(relationship.to_model_name(), "User");
	/// ```
	pub fn to_model_name(&self) -> &str {
		self.to_model
			.split('.')
			.next_back()
			.unwrap_or(self.to_model)
	}
}

/// Global distributed slice for relationship registration
///
/// This is the global registry where all relationships are collected at compile time.
/// Relationships can be registered by adding items to this slice using the `#[distributed_slice]`
/// attribute from the `linkme` crate.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::{RELATIONSHIPS, RelationshipMetadata, RelationshipType};
///
/// #[linkme::distributed_slice(RELATIONSHIPS)]
/// static POST_AUTHOR: RelationshipMetadata = RelationshipMetadata {
///     from_model: "blog.Post",
///     to_model: "auth.User",
///     relationship_type: RelationshipType::ForeignKey,
///     field_name: "author",
///     related_name: Some("posts"),
///     db_column: Some("author_id"),
///     through_table: None,
/// };
/// ```
#[distributed_slice]
pub static RELATIONSHIPS: [RelationshipMetadata];

/// Cache for relationship lookups by model.
/// Lazily initialized on first access and immutable thereafter.
static RELATIONSHIP_CACHE: OnceLock<HashMap<&'static str, Vec<&'static RelationshipMetadata>>> =
	OnceLock::new();

/// Returns the cached relationship metadata indexed by source model.
fn relationship_cache() -> &'static HashMap<&'static str, Vec<&'static RelationshipMetadata>> {
	RELATIONSHIP_CACHE.get_or_init(|| {
		let mut cache: HashMap<&'static str, Vec<&'static RelationshipMetadata>> = HashMap::new();
		for rel in RELATIONSHIPS.iter() {
			cache.entry(rel.from_model).or_default().push(rel);
		}
		cache
	})
}

/// Get all registered relationships
///
/// This function returns a slice of all relationships that have been registered
/// in the global relationship registry.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::get_registered_relationships;
///
/// let relationships = get_registered_relationships();
/// println!("Found {} registered relationships", relationships.len());
/// ```
pub fn get_registered_relationships() -> &'static [RelationshipMetadata] {
	&RELATIONSHIPS
}

/// Get all relationships originating from a specific model.
pub fn get_relationships_for_model(model: &str) -> Vec<&'static RelationshipMetadata> {
	relationship_cache().get(model).cloned().unwrap_or_default()
}

/// Find relationships by target model
///
/// This function returns all relationships that point to the specified target model.
/// Useful for discovering reverse relationships.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::get_relationships_to_model;
///
/// let user_reverse_rels = get_relationships_to_model("auth.User");
/// for rel in user_reverse_rels {
///     println!("Reverse relationship from {}.{}", rel.from_model, rel.field_name);
/// }
/// ```
pub fn get_relationships_to_model(target_model: &str) -> Vec<&'static RelationshipMetadata> {
	RELATIONSHIPS
		.iter()
		.filter(|r| r.to_model == target_model)
		.collect()
}

/// Metadata for a reverse relation
///
/// This structure represents a reverse relation that has been dynamically
/// registered for lazy loading or eager loading via select_related.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseRelationMetadata {
	/// The model this reverse relation belongs to (e.g., "User")
	pub on_model: &'static str,

	/// The accessor name (e.g., "posts" or "post_set")
	pub accessor_name: String,

	/// The related model (e.g., "Post")
	pub related_model: &'static str,

	/// The type of reverse relation
	pub relation_type: ReverseRelationType,

	/// The original field name in the related model
	pub through_field: &'static str,
}

/// Type of reverse relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseRelationType {
	/// Reverse of OneToMany (collection)
	ReverseOneToMany,
	/// Reverse of ManyToMany (collection)
	ReverseManyToMany,
	/// Reverse of OneToOne (single object)
	ReverseOneToOne,
}

impl ReverseRelationMetadata {
	/// Create a new reverse relation metadata instance
	pub fn new(
		on_model: &'static str,
		accessor_name: String,
		related_model: &'static str,
		relation_type: ReverseRelationType,
		through_field: &'static str,
	) -> Self {
		Self {
			on_model,
			accessor_name,
			related_model,
			relation_type,
			through_field,
		}
	}
}

/// Builder storage used during initialization phase only.
/// Reverse relations are added here before finalization.
static REVERSE_RELATIONS_BUILDER: RwLock<Vec<ReverseRelationMetadata>> = RwLock::new(Vec::new());

/// Finalized map indexed by model name (read-only after initialization).
/// This is populated by `finalize_reverse_relations()` and accessed by
/// `get_reverse_relations_for_model()`.
static REVERSE_RELATIONS: OnceLock<HashMap<String, Vec<ReverseRelationMetadata>>> = OnceLock::new();

/// Registers a reverse relation during the initialization phase.
///
/// Must be called before `finalize_reverse_relations()`.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::{register_reverse_relation, ReverseRelationMetadata, ReverseRelationType};
///
/// let reverse_relation = ReverseRelationMetadata::new(
///     "User",
///     "posts".to_string(),
///     "Post",
///     ReverseRelationType::ReverseOneToMany,
///     "author",
/// );
/// register_reverse_relation(reverse_relation).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`crate::AppError::RegistryState`] if called after `finalize_reverse_relations()`.
pub fn register_reverse_relation(relation: ReverseRelationMetadata) -> Result<(), crate::AppError> {
	if REVERSE_RELATIONS.get().is_some() {
		return Err(crate::AppError::RegistryState(
			"Cannot register reverse relations after finalization".to_string(),
		));
	}
	// Recover from poisoned lock to prevent cascading panics
	let mut builder = REVERSE_RELATIONS_BUILDER
		.write()
		.unwrap_or_else(PoisonError::into_inner);
	builder.push(relation);
	Ok(())
}

/// Finalizes the reverse relations, making them immutable.
///
/// This function should be called at the end of `Apps::populate()` after all
/// reverse relations have been registered. After this call, `register_reverse_relation()`
/// will panic if called again.
pub fn finalize_reverse_relations() {
	if REVERSE_RELATIONS.get().is_some() {
		return;
	}
	// Recover from poisoned lock to prevent cascading panics
	let builder = REVERSE_RELATIONS_BUILDER
		.read()
		.unwrap_or_else(PoisonError::into_inner);
	let mut indexed = HashMap::new();
	for relation in builder.iter() {
		indexed
			.entry(relation.on_model.to_string())
			.or_insert_with(Vec::new)
			.push(relation.clone());
	}
	let _ = REVERSE_RELATIONS.set(indexed);
}

/// Returns all reverse relations for a specific model.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::registry::get_reverse_relations_for_model;
///
/// let relations = get_reverse_relations_for_model("User");
/// for relation in relations {
///     println!("Reverse relation: {}", relation.accessor_name);
/// }
/// ```
pub fn get_reverse_relations_for_model(model_name: &str) -> Vec<ReverseRelationMetadata> {
	REVERSE_RELATIONS
		.get()
		.and_then(|m| m.get(model_name))
		.cloned()
		.unwrap_or_default()
}

/// Resets all global registry caches for test isolation.
///
/// This clears the `MODEL_CACHE`, `RELATIONSHIP_CACHE`, and `REVERSE_RELATIONS`
/// `OnceLock` instances so that each test can start with a clean slate.
/// Also clears the `REVERSE_RELATIONS_BUILDER` `RwLock` vec.
///
/// # Safety
///
/// This function replaces static `OnceLock` values using `std::ptr::write`.
/// It is only safe to call from a single-threaded test context (e.g., with
/// `#[serial]`) where no other thread is concurrently reading these statics.
#[cfg(test)]
pub fn reset_global_registry() {
	use std::sync::PoisonError;

	// Clear the builder vec
	let mut builder = REVERSE_RELATIONS_BUILDER
		.write()
		.unwrap_or_else(PoisonError::into_inner);
	builder.clear();
	drop(builder);

	// SAFETY: We replace each OnceLock in-place with a fresh instance.
	// This is safe only when called from a single-threaded test context
	// (enforced by #[serial]) where no concurrent readers exist.
	unsafe {
		let model_cache_ptr = std::ptr::addr_of!(MODEL_CACHE)
			as *mut OnceLock<HashMap<&'static str, Vec<&'static ModelMetadata>>>;
		std::ptr::write(model_cache_ptr, OnceLock::new());

		let rel_cache_ptr = std::ptr::addr_of!(RELATIONSHIP_CACHE)
			as *mut OnceLock<HashMap<&'static str, Vec<&'static RelationshipMetadata>>>;
		std::ptr::write(rel_cache_ptr, OnceLock::new());

		let rev_rel_ptr = std::ptr::addr_of!(REVERSE_RELATIONS)
			as *mut OnceLock<HashMap<String, Vec<ReverseRelationMetadata>>>;
		std::ptr::write(rev_rel_ptr, OnceLock::new());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// Test model registrations
	#[distributed_slice(MODELS)]
	static TEST_USER_MODEL: ModelMetadata = ModelMetadata {
		app_label: "auth",
		model_name: "User",
		table_name: "auth_users",
	};

	#[distributed_slice(MODELS)]
	static TEST_POST_MODEL: ModelMetadata = ModelMetadata {
		app_label: "blog",
		model_name: "Post",
		table_name: "blog_posts",
	};

	#[distributed_slice(MODELS)]
	static TEST_COMMENT_MODEL: ModelMetadata = ModelMetadata {
		app_label: "blog",
		model_name: "Comment",
		table_name: "blog_comments",
	};

	#[test]
	fn test_model_metadata_new() {
		let metadata = ModelMetadata::new("myapp", "MyModel", "my_table");
		assert_eq!(metadata.app_label, "myapp");
		assert_eq!(metadata.model_name, "MyModel");
		assert_eq!(metadata.table_name, "my_table");
	}

	#[test]
	fn test_qualified_name() {
		let metadata = ModelMetadata::new("auth", "User", "users");
		assert_eq!(metadata.qualified_name(), "auth.User");
	}

	#[test]
	fn test_find_model_invalid_format() {
		let model = find_model("InvalidFormat");
		assert!(model.is_none());

		let model = find_model("too.many.parts");
		assert!(model.is_none());
	}

	#[test]
	fn test_model_metadata_equality() {
		let meta1 = ModelMetadata::new("app", "Model", "table");
		let meta2 = ModelMetadata::new("app", "Model", "table");
		let meta3 = ModelMetadata::new("app", "Other", "table");

		assert_eq!(meta1, meta2);
		assert_ne!(meta1, meta3);
	}

	#[test]
	fn test_reverse_relation_metadata_new() {
		let relation = ReverseRelationMetadata::new(
			"User",
			"posts".to_string(),
			"Post",
			ReverseRelationType::ReverseOneToMany,
			"author",
		);

		assert_eq!(relation.on_model, "User");
		assert_eq!(relation.accessor_name, "posts");
		assert_eq!(relation.related_model, "Post");
		assert_eq!(
			relation.relation_type,
			ReverseRelationType::ReverseOneToMany
		);
		assert_eq!(relation.through_field, "author");
	}

	#[rstest]
	fn test_get_reverse_relations_for_nonexistent_model() {
		// Before finalization, returns empty vec
		let relations = get_reverse_relations_for_model("NonExistent");
		assert!(relations.is_empty());
	}

	#[test]
	fn test_reverse_relation_types() {
		assert_eq!(
			ReverseRelationType::ReverseOneToMany,
			ReverseRelationType::ReverseOneToMany
		);
		assert_ne!(
			ReverseRelationType::ReverseOneToMany,
			ReverseRelationType::ReverseManyToMany
		);
		assert_ne!(
			ReverseRelationType::ReverseOneToMany,
			ReverseRelationType::ReverseOneToOne
		);
	}

	// Test relationship metadata
	#[distributed_slice(RELATIONSHIPS)]
	static TEST_POST_AUTHOR: RelationshipMetadata = RelationshipMetadata {
		from_model: "blog.Post",
		to_model: "auth.User",
		relationship_type: RelationshipType::ForeignKey,
		field_name: "author",
		related_name: Some("posts"),
		db_column: Some("author_id"),
		through_table: None,
	};

	#[distributed_slice(RELATIONSHIPS)]
	static TEST_POST_TAGS: RelationshipMetadata = RelationshipMetadata {
		from_model: "blog.Post",
		to_model: "blog.Tag",
		relationship_type: RelationshipType::ManyToMany,
		field_name: "tags",
		related_name: Some("posts"),
		db_column: None,
		through_table: Some("blog_post_tags"),
	};

	#[test]
	fn test_relationship_metadata_new() {
		let relationship = RelationshipMetadata::new(
			"blog.Post",
			"auth.User",
			RelationshipType::ForeignKey,
			"author",
			Some("posts"),
			Some("author_id"),
			None,
		);

		assert_eq!(relationship.from_model, "blog.Post");
		assert_eq!(relationship.to_model, "auth.User");
		assert_eq!(relationship.relationship_type, RelationshipType::ForeignKey);
		assert_eq!(relationship.field_name, "author");
		assert_eq!(relationship.related_name, Some("posts"));
		assert_eq!(relationship.db_column, Some("author_id"));
		assert_eq!(relationship.through_table, None);
	}

	#[test]
	fn test_relationship_metadata_model_names() {
		let relationship = RelationshipMetadata::new(
			"blog.Post",
			"auth.User",
			RelationshipType::ForeignKey,
			"author",
			None,
			None,
			None,
		);

		assert_eq!(relationship.from_model_name(), "Post");
		assert_eq!(relationship.to_model_name(), "User");
	}

	#[test]
	fn test_relationship_types() {
		assert_eq!(RelationshipType::ForeignKey, RelationshipType::ForeignKey);
		assert_ne!(RelationshipType::ForeignKey, RelationshipType::ManyToMany);
		assert_ne!(RelationshipType::ForeignKey, RelationshipType::OneToOne);
	}

	#[test]
	fn test_relationship_metadata_equality() {
		let rel1 = RelationshipMetadata::new(
			"app.Model",
			"app.Other",
			RelationshipType::ForeignKey,
			"field",
			None,
			None,
			None,
		);
		let rel2 = RelationshipMetadata::new(
			"app.Model",
			"app.Other",
			RelationshipType::ForeignKey,
			"field",
			None,
			None,
			None,
		);
		let rel3 = RelationshipMetadata::new(
			"app.Model",
			"app.Other",
			RelationshipType::ManyToMany,
			"field",
			None,
			None,
			None,
		);

		assert_eq!(rel1, rel2);
		assert_ne!(rel1, rel3);
	}

	#[rstest]
	fn test_rwlock_poison_recovery_write() {
		// Arrange
		let lock = RwLock::new(vec![1, 2, 3]);

		// Poison the lock by panicking inside a write guard
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = lock.write().unwrap();
			panic!("intentional panic to poison the lock");
		}));

		// Act: recover from poisoned lock using PoisonError::into_inner
		let mut guard = lock.write().unwrap_or_else(PoisonError::into_inner);
		guard.push(4);

		// Assert: data is accessible and intact after recovery
		assert_eq!(*guard, vec![1, 2, 3, 4]);
	}

	#[rstest]
	fn test_rwlock_poison_recovery_read() {
		// Arrange
		let lock = RwLock::new(vec![10, 20, 30]);

		// Poison the lock by panicking inside a write guard
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = lock.write().unwrap();
			panic!("intentional panic to poison the lock");
		}));

		// Act: recover from poisoned lock using PoisonError::into_inner
		let guard = lock.read().unwrap_or_else(PoisonError::into_inner);

		// Assert: data is readable after recovery
		assert_eq!(*guard, vec![10, 20, 30]);
	}
}
