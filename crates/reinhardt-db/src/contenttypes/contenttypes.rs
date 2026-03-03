/// Generic content type system for polymorphic relationships
/// Based on Django's contenttypes framework
///
/// This module provides both string-based (runtime) and type-safe (compile-time)
/// content type registry mechanisms.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Represents a content type (model) in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ContentType {
	pub id: Option<i64>,
	pub app_label: String,
	pub model: String,
}

impl ContentType {
	pub fn new(app_label: impl Into<String>, model: impl Into<String>) -> Self {
		Self {
			id: None,
			app_label: app_label.into(),
			model: model.into(),
		}
	}

	pub fn with_id(mut self, id: i64) -> Self {
		self.id = Some(id);
		self
	}

	/// Get the natural key for this content type
	pub fn natural_key(&self) -> (String, String) {
		(self.app_label.clone(), self.model.clone())
	}

	/// Get fully qualified model name
	pub fn qualified_name(&self) -> String {
		format!("{}.{}", self.app_label, self.model)
	}
}

/// Registry for managing content types
pub struct ContentTypeRegistry {
	types: RwLock<HashMap<(String, String), ContentType>>,
	by_id: RwLock<HashMap<i64, ContentType>>,
	next_id: RwLock<i64>,
}

impl ContentTypeRegistry {
	pub fn new() -> Self {
		Self {
			types: RwLock::new(HashMap::new()),
			by_id: RwLock::new(HashMap::new()),
			next_id: RwLock::new(1),
		}
	}

	/// Register a new content type
	pub fn register(&self, mut ct: ContentType) -> ContentType {
		let key = ct.natural_key();

		// Check if already exists
		if let Some(existing) = self
			.types
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(&key)
		{
			return existing.clone();
		}

		// Assign ID if not present
		if ct.id.is_none() {
			let mut next_id = self.next_id.write().unwrap_or_else(|e| e.into_inner());
			ct.id = Some(*next_id);
			*next_id += 1;
		}

		// Store in both maps
		self.types
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.insert(key, ct.clone());
		if let Some(id) = ct.id {
			self.by_id
				.write()
				.unwrap_or_else(|e| e.into_inner())
				.insert(id, ct.clone());
		}

		ct
	}

	/// Get content type by app label and model name
	pub fn get(&self, app_label: &str, model: &str) -> Option<ContentType> {
		let key = (app_label.to_string(), model.to_string());
		self.types
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(&key)
			.cloned()
	}

	/// Get content type by ID
	pub fn get_by_id(&self, id: i64) -> Option<ContentType> {
		self.by_id
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(&id)
			.cloned()
	}

	/// Get or create a content type
	pub fn get_or_create(&self, app_label: &str, model: &str) -> ContentType {
		if let Some(ct) = self.get(app_label, model) {
			ct
		} else {
			self.register(ContentType::new(app_label, model))
		}
	}

	/// List all registered content types
	pub fn all(&self) -> Vec<ContentType> {
		self.types
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.values()
			.cloned()
			.collect()
	}

	/// Clear all registered types (mainly for testing)
	pub fn clear(&self) {
		self.types
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		self.by_id
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		*self.next_id.write().unwrap_or_else(|e| e.into_inner()) = 1;
	}
}

impl Default for ContentTypeRegistry {
	fn default() -> Self {
		Self::new()
	}
}

use once_cell::sync::Lazy;

pub static CONTENT_TYPE_REGISTRY: Lazy<ContentTypeRegistry> = Lazy::new(ContentTypeRegistry::new);

/// Generic foreign key field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericForeignKey {
	pub content_type_id: Option<i64>,
	pub object_id: Option<i64>,
}

impl GenericForeignKey {
	pub fn new() -> Self {
		Self {
			content_type_id: None,
			object_id: None,
		}
	}

	pub fn set(&mut self, content_type: &ContentType, object_id: i64) {
		self.content_type_id = content_type.id;
		self.object_id = Some(object_id);
	}

	pub fn get_content_type(&self) -> Option<ContentType> {
		self.content_type_id
			.and_then(|id| CONTENT_TYPE_REGISTRY.get_by_id(id))
	}

	pub fn is_set(&self) -> bool {
		self.content_type_id.is_some() && self.object_id.is_some()
	}

	pub fn clear(&mut self) {
		self.content_type_id = None;
		self.object_id = None;
	}
}

impl Default for GenericForeignKey {
	fn default() -> Self {
		Self::new()
	}
}

/// Trait for models that can be targets of generic relations
pub trait GenericRelatable {
	fn get_content_type() -> ContentType;
	fn get_object_id(&self) -> i64;
}

/// Helper for building generic relation queries
pub struct GenericRelationQuery {
	content_type: ContentType,
	object_ids: Vec<i64>,
}

impl GenericRelationQuery {
	pub fn new(content_type: ContentType) -> Self {
		Self {
			content_type,
			object_ids: Vec::new(),
		}
	}

	pub fn add_object(&mut self, object_id: i64) {
		self.object_ids.push(object_id);
	}

	pub fn to_sql(&self, table: &str) -> String {
		let ct_id = self.content_type.id.unwrap_or(0);
		let ids = self
			.object_ids
			.iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>()
			.join(", ");

		format!(
			"SELECT * FROM {} WHERE content_type_id = {} AND object_id IN ({})",
			table, ct_id, ids
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_content_type_creation() {
		let ct = ContentType::new("blog", "Post");
		assert_eq!(ct.app_label, "blog");
		assert_eq!(ct.model, "Post");
		assert_eq!(ct.qualified_name(), "blog.Post");
	}

	#[test]
	fn test_content_type_natural_key() {
		let ct = ContentType::new("auth", "User");
		let (app, model) = ct.natural_key();
		assert_eq!(app, "auth");
		assert_eq!(model, "User");
	}

	#[test]
	fn test_registry_register() {
		let registry = ContentTypeRegistry::new();
		let ct = ContentType::new("test", "Model");
		let registered = registry.register(ct);

		assert!(registered.id.is_some());
		assert_eq!(registered.app_label, "test");
	}

	#[test]
	fn test_registry_get() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app1", "Model1"));

		let found = registry.get("app1", "Model1");
		assert!(found.is_some());
		assert_eq!(found.unwrap().model, "Model1");
	}

	#[test]
	fn test_registry_get_by_id() {
		let registry = ContentTypeRegistry::new();
		let ct = registry.register(ContentType::new("app2", "Model2"));
		let id = ct.id.unwrap();

		let found = registry.get_by_id(id);
		assert!(found.is_some());
		assert_eq!(found.unwrap().app_label, "app2");
	}

	#[test]
	fn test_registry_get_or_create() {
		let registry = ContentTypeRegistry::new();

		let ct1 = registry.get_or_create("app3", "Model3");
		let ct2 = registry.get_or_create("app3", "Model3");

		assert_eq!(ct1.id, ct2.id);
	}

	#[test]
	fn test_registry_all() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app4", "Model4"));
		registry.register(ContentType::new("app5", "Model5"));

		let all = registry.all();
		assert_eq!(all.len(), 2);
	}

	#[test]
	fn test_generic_foreign_key() {
		let registry = ContentTypeRegistry::new();
		let ct = registry.register(ContentType::new("test", "Article"));

		let mut gfk = GenericForeignKey::new();
		assert!(!gfk.is_set());

		gfk.set(&ct, 42);
		assert!(gfk.is_set());
		assert_eq!(gfk.object_id, Some(42));
		assert_eq!(gfk.content_type_id, ct.id);

		gfk.clear();
		assert!(!gfk.is_set());
	}

	#[test]
	fn test_generic_foreign_key_get_content_type() {
		let ct = CONTENT_TYPE_REGISTRY.register(ContentType::new("blog", "Comment"));

		let mut gfk = GenericForeignKey::new();
		gfk.set(&ct, 100);

		let retrieved_ct = gfk.get_content_type();
		assert!(retrieved_ct.is_some());
		assert_eq!(retrieved_ct.unwrap().model, "Comment");
	}

	#[test]
	fn test_generic_relation_query() {
		let ct = ContentType::new("shop", "Product").with_id(5);
		let mut query = GenericRelationQuery::new(ct);

		query.add_object(1);
		query.add_object(2);
		query.add_object(3);

		let sql = query.to_sql("ratings");
		assert!(sql.contains("content_type_id = 5"));
		assert!(sql.contains("object_id IN (1, 2, 3)"));
	}

	#[test]
	fn test_registry_clear() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("temp", "TempModel"));

		assert_eq!(registry.all().len(), 1);

		registry.clear();
		assert_eq!(registry.all().len(), 0);
	}
}

// ============================================================================
// Type-safe content type registry (compile-time checked)
// ============================================================================

/// Trait for models that can be registered as content types
///
/// Implement this trait for each model in your application.
/// The compiler will ensure that only valid model types can be used.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::contenttypes::ModelType;
///
/// pub struct UserModel;
/// impl ModelType for UserModel {
///     const APP_LABEL: &'static str = "auth";
///     const MODEL_NAME: &'static str = "User";
/// }
/// ```
pub trait ModelType {
	/// The app label for this model
	const APP_LABEL: &'static str;

	/// The model name
	const MODEL_NAME: &'static str;
}

impl ContentTypeRegistry {
	/// Type-safe get method
	///
	/// Get a content type using compile-time verified model types.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::{ContentTypeRegistry, ModelType};
	///
	/// pub struct UserModel;
	/// impl ModelType for UserModel {
	///     const APP_LABEL: &'static str = "auth";
	///     const MODEL_NAME: &'static str = "User";
	/// }
	///
	/// let registry = ContentTypeRegistry::new();
	/// let ct = registry.get_typed::<UserModel>();
	/// ```
	pub fn get_typed<M: ModelType>(&self) -> Option<ContentType> {
		self.get(M::APP_LABEL, M::MODEL_NAME)
	}

	/// Type-safe get_or_create method
	///
	/// Get or create a content type using compile-time verified model types.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::contenttypes::{ContentTypeRegistry, ModelType};
	///
	/// pub struct PostModel;
	/// impl ModelType for PostModel {
	///     const APP_LABEL: &'static str = "blog";
	///     const MODEL_NAME: &'static str = "Post";
	/// }
	///
	/// let registry = ContentTypeRegistry::new();
	/// let ct = registry.get_or_create_typed::<PostModel>();
	/// ```
	pub fn get_or_create_typed<M: ModelType>(&self) -> ContentType {
		self.get_or_create(M::APP_LABEL, M::MODEL_NAME)
	}

	/// Type-safe register method
	///
	/// Register a content type using compile-time verified model types.
	pub fn register_typed<M: ModelType>(&self) -> ContentType {
		self.register(ContentType::new(M::APP_LABEL, M::MODEL_NAME))
	}
}

impl GenericForeignKey {
	/// Type-safe set method
	///
	/// Set the generic foreign key using a typed model.
	pub fn set_typed<M: ModelType>(&mut self, registry: &ContentTypeRegistry, object_id: i64) {
		let ct = registry.get_or_create_typed::<M>();
		self.set(&ct, object_id);
	}
}

#[cfg(test)]
mod typed_tests {
	use super::*;

	// Test model types
	struct UserModel;
	impl ModelType for UserModel {
		const APP_LABEL: &'static str = "auth";
		const MODEL_NAME: &'static str = "User";
	}

	struct PostModel;
	impl ModelType for PostModel {
		const APP_LABEL: &'static str = "blog";
		const MODEL_NAME: &'static str = "Post";
	}

	struct CommentModel;
	impl ModelType for CommentModel {
		const APP_LABEL: &'static str = "blog";
		const MODEL_NAME: &'static str = "Comment";
	}

	#[test]
	fn test_contenttypes_typed_register() {
		let registry = ContentTypeRegistry::new();
		let ct = registry.register_typed::<UserModel>();

		assert_eq!(ct.app_label, "auth");
		assert_eq!(ct.model, "User");
		assert!(ct.id.is_some());
	}

	#[test]
	fn test_contenttypes_typed_get() {
		let registry = ContentTypeRegistry::new();
		registry.register_typed::<PostModel>();

		let ct = registry.get_typed::<PostModel>();
		assert!(ct.is_some());
		assert_eq!(ct.unwrap().model, "Post");
	}

	#[test]
	fn test_contenttypes_typed_get_not_found() {
		let registry = ContentTypeRegistry::new();

		let ct = registry.get_typed::<CommentModel>();
		assert!(ct.is_none());
	}

	#[test]
	fn test_typed_get_or_create() {
		let registry = ContentTypeRegistry::new();

		let ct1 = registry.get_or_create_typed::<UserModel>();
		let ct2 = registry.get_or_create_typed::<UserModel>();

		assert_eq!(ct1.id, ct2.id);
	}

	#[test]
	fn test_typed_generic_foreign_key() {
		// Clean up global registry first
		CONTENT_TYPE_REGISTRY.clear();

		let registry = ContentTypeRegistry::new();
		let mut gfk = GenericForeignKey::new();

		gfk.set_typed::<PostModel>(&registry, 42);

		assert!(gfk.is_set());
		assert_eq!(gfk.object_id, Some(42));

		// Note: get_content_type uses global registry, so we need to register there too
		CONTENT_TYPE_REGISTRY
			.register(ContentType::new("blog", "Post").with_id(gfk.content_type_id.unwrap()));

		let ct = gfk.get_content_type();
		assert!(ct.is_some());
		assert_eq!(ct.unwrap().model, "Post");

		// Clean up
		CONTENT_TYPE_REGISTRY.clear();
	}

	#[test]
	fn test_contenttypes_typed_and_regular_mixed() {
		let registry = ContentTypeRegistry::new();

		// Register using typed method
		registry.register_typed::<UserModel>();

		// Can access using both methods
		let typed = registry.get_typed::<UserModel>();
		let regular = registry.get("auth", "User");

		assert!(typed.is_some());
		assert!(regular.is_some());
		assert_eq!(typed.unwrap().id, regular.unwrap().id);
	}
}

// ============================================================================
// Comprehensive tests inspired by Django's contenttypes tests
// ============================================================================
//
// See docs/IMPLEMENTATION_NOTES.md for unimplemented test categories and future additions

#[cfg(test)]
mod inspired_tests {
	use super::*;

	// Test helper: Setup function that clears registry before each test
	// Note: Each test creates its own registry instance to ensure isolation
	fn setup_registry() -> ContentTypeRegistry {
		let registry = ContentTypeRegistry::new();
		registry.clear();
		registry
	}

	// ========== ContentType Model Tests ==========

	/// Test: ContentType string representation
	#[test]
	fn test_content_type_str() {
		let ct = ContentType::new("contenttypes_tests", "site");
		assert_eq!(ct.qualified_name(), "contenttypes_tests.site");
	}

	/// Test: ContentType natural key retrieval
	#[test]
	fn test_content_type_natural_key_retrieval() {
		let ct = ContentType::new("auth", "user");
		let (app, model) = ct.natural_key();
		assert_eq!(app, "auth");
		assert_eq!(model, "user");
	}

	/// Test: Unknown model handling
	#[test]
	fn test_content_type_unknown_model() {
		let ct = ContentType::new("contenttypes_tests", "unknown");
		assert_eq!(ct.model, "unknown");
		assert_eq!(ct.qualified_name(), "contenttypes_tests.unknown");
	}

	// ========== Registry Cache Tests ==========

	/// Test: get_or_create with same params returns same instance
	/// Inspired by: test_get_for_models_creation
	#[test]
	fn test_get_or_create_returns_same_instance() {
		let registry = setup_registry();

		let ct1 = registry.get_or_create("app1", "Model1");
		let ct2 = registry.get_or_create("app1", "Model1");

		assert_eq!(ct1.id, ct2.id);
		assert_eq!(ct1.app_label, ct2.app_label);
		assert_eq!(ct1.model, ct2.model);
	}

	/// Test: Registry with empty initial state
	/// Inspired by: test_get_for_models_empty_cache
	#[test]
	fn test_registry_empty_initial_state() {
		let registry = setup_registry();

		let ct = registry.get_or_create("contenttypes", "contenttype");
		assert!(ct.id.is_some());
		assert_eq!(ct.app_label, "contenttypes");
		assert_eq!(ct.model, "contenttype");
	}

	/// Test: Registry with partial state
	/// Inspired by: test_get_for_models_partial_cache
	#[test]
	fn test_registry_partial_state() {
		let registry = setup_registry();

		// Pre-register one content type
		registry.get_or_create("app1", "Model1");

		// Get existing and create new
		let ct1 = registry.get("app1", "Model1");
		let ct2 = registry.get_or_create("app2", "Model2");

		assert!(ct1.is_some());
		assert!(ct2.id.is_some());
	}

	/// Test: Registry with full state
	/// Inspired by: test_get_for_models_full_cache
	#[test]
	fn test_registry_full_state() {
		let registry = setup_registry();

		// Pre-register all content types
		registry.get_or_create("contenttypes", "contenttype");
		registry.get_or_create("app1", "model1");

		// All should be available without creation
		let ct1 = registry.get("contenttypes", "contenttype");
		let ct2 = registry.get("app1", "model1");

		assert!(ct1.is_some());
		assert!(ct2.is_some());
	}

	/// Test: Create content type if it doesn't exist
	/// Inspired by: test_get_for_model_create_contenttype
	#[test]
	fn test_get_or_create_creates_if_missing() {
		let registry = setup_registry();

		let ct = registry.get_or_create("contenttypes_tests", "modelcreatedonthefly");
		assert_eq!(ct.app_label, "contenttypes_tests");
		assert_eq!(ct.model, "modelcreatedonthefly");
		assert!(ct.id.is_some());
	}

	/// Test: Separate registries don't share state
	/// Inspired by: test_cache_not_shared_between_managers
	#[test]
	fn test_registries_not_shared() {
		let registry1 = ContentTypeRegistry::new();
		let registry2 = ContentTypeRegistry::new();

		registry1.get_or_create("app1", "Model1");

		// registry2 should not have the content type from registry1
		let ct = registry2.get("app1", "Model1");
		assert!(ct.is_none());
	}

	/// Test: Missing model handling
	/// Inspired by: test_missing_model
	#[test]
	fn test_missing_model_display() {
		let registry = setup_registry();

		let ct = ContentType::new("contenttypes", "OldModel").with_id(999);
		let registered = registry.register(ct.clone());

		assert_eq!(registered.model, "OldModel");

		// Stale ContentTypes can be fetched by ID
		let ct_fetched = registry.get_by_id(999);
		assert!(ct_fetched.is_some());
	}

	/// Test: Missing model with existing model name in another app
	/// Inspired by: test_missing_model_with_existing_model_name
	#[test]
	fn test_missing_model_with_existing_name() {
		let registry = setup_registry();

		// Create a stale ContentType that matches name of existing model
		registry.register(ContentType::new("contenttypes", "author"));

		// get_or_create should work for different app
		let ct_author = registry.get_or_create("contenttypes_tests", "Author");

		assert_eq!(ct_author.app_label, "contenttypes_tests");
		assert_eq!(ct_author.model, "Author");
	}

	// ========== GenericForeignKey Tests ==========

	/// Test: GenericForeignKey respects deleted objects
	/// Inspired by: test_get_object_cache_respects_deleted_objects
	#[test]
	fn test_generic_foreign_key_respects_deletion() {
		let registry = setup_registry();
		let ct = registry.get_or_create("test", "Question");

		let mut gfk = GenericForeignKey::new();
		gfk.set(&ct, 42);

		assert!(gfk.is_set());
		assert_eq!(gfk.object_id, Some(42));

		// Clear simulates deletion
		gfk.clear();
		assert!(!gfk.is_set());
		assert_eq!(gfk.object_id, None);
	}

	/// Test: Clear cached generic relation
	/// Inspired by: test_clear_cached_generic_relation
	#[test]
	fn test_clear_cached_generic_relation() {
		let registry = setup_registry();
		let ct = registry.get_or_create("test", "Question");

		let mut gfk = GenericForeignKey::new();
		gfk.set(&ct, 100);

		let old_ct_id = gfk.content_type_id;

		// Clear and reset
		gfk.clear();
		gfk.set(&ct, 200);

		let new_ct_id = gfk.content_type_id;
		assert_eq!(old_ct_id, new_ct_id);
		assert_eq!(gfk.object_id, Some(200));
	}

	/// Test: GenericForeignKey get content type
	/// Inspired by: test_get_content_type_no_arguments
	/// ========== ContentType Operations Tests ==========
	/// Test: ContentType ID uniqueness
	#[test]
	fn test_content_type_id_uniqueness() {
		let registry = setup_registry();

		let ct1 = registry.get_or_create("app1", "Model1");
		let ct2 = registry.get_or_create("app2", "Model2");

		assert_ne!(ct1.id, ct2.id);
	}

	/// Test: ContentType registry all() method
	#[test]
	fn test_registry_all_listing() {
		let registry = setup_registry();

		registry.get_or_create("app1", "Model1");
		registry.get_or_create("app2", "Model2");
		registry.get_or_create("app3", "Model3");

		let all = registry.all();
		assert_eq!(all.len(), 3);
	}

	/// Test: ContentType with special characters
	#[test]
	fn test_content_type_special_characters() {
		let registry = setup_registry();

		let ct = registry.get_or_create("my_app", "My_Model");
		assert_eq!(ct.app_label, "my_app");
		assert_eq!(ct.model, "My_Model");
	}

	// ========== GenericRelationQuery Tests ==========

	/// Test: Generic relation query SQL generation
	#[test]
	fn test_generic_relation_query_sql() {
		let ct = ContentType::new("shop", "Product").with_id(5);
		let mut query = GenericRelationQuery::new(ct);

		query.add_object(10);
		query.add_object(20);
		query.add_object(30);

		let sql = query.to_sql("ratings");
		assert!(sql.contains("content_type_id = 5"));
		assert!(sql.contains("object_id IN (10, 20, 30)"));
		assert!(sql.contains("FROM ratings"));
	}

	/// Test: Generic relation query with empty objects
	#[test]
	fn test_generic_relation_query_empty() {
		let ct = ContentType::new("test", "Model").with_id(1);
		let query = GenericRelationQuery::new(ct);

		let sql = query.to_sql("items");
		assert!(sql.contains("content_type_id = 1"));
		assert!(sql.contains("object_id IN ()"));
	}

	/// Test: Generic relation query with single object
	#[test]
	fn test_generic_relation_query_single() {
		let ct = ContentType::new("blog", "Post").with_id(3);
		let mut query = GenericRelationQuery::new(ct);

		query.add_object(42);

		let sql = query.to_sql("comments");
		assert!(sql.contains("content_type_id = 3"));
		assert!(sql.contains("object_id IN (42)"));
	}

	// ========== Edge Cases and Error Conditions ==========

	/// Test: Registry clear removes all content types
	#[test]
	fn test_registry_clear_removes_all() {
		let registry = setup_registry();

		registry.get_or_create("app1", "Model1");
		registry.get_or_create("app2", "Model2");

		assert_eq!(registry.all().len(), 2);

		registry.clear();
		assert_eq!(registry.all().len(), 0);
	}

	/// Test: Get non-existent content type returns None
	#[test]
	fn test_get_nonexistent_returns_none() {
		let registry = setup_registry();

		let ct = registry.get("nonexistent", "Model");
		assert!(ct.is_none());
	}

	/// Test: Get by non-existent ID returns None
	#[test]
	fn test_get_by_nonexistent_id_returns_none() {
		let registry = setup_registry();

		let ct = registry.get_by_id(99999);
		assert!(ct.is_none());
	}

	/// Test: ContentType equality
	#[test]
	fn test_content_type_equality() {
		let ct1 = ContentType::new("app", "Model");
		let ct2 = ContentType::new("app", "Model");

		assert_eq!(ct1, ct2);
	}

	/// Test: ContentType inequality with different app
	#[test]
	fn test_content_type_inequality_different_app() {
		let ct1 = ContentType::new("app1", "Model");
		let ct2 = ContentType::new("app2", "Model");

		assert_ne!(ct1, ct2);
	}

	/// Test: ContentType inequality with different model
	#[test]
	fn test_content_type_inequality_different_model() {
		let ct1 = ContentType::new("app", "Model1");
		let ct2 = ContentType::new("app", "Model2");

		assert_ne!(ct1, ct2);
	}

	/// Test: GenericForeignKey default state
	#[test]
	fn test_generic_foreign_key_default() {
		let gfk = GenericForeignKey::default();

		assert!(!gfk.is_set());
		assert_eq!(gfk.content_type_id, None);
		assert_eq!(gfk.object_id, None);
	}

	/// Test: ContentType cloning
	#[test]
	fn test_content_type_clone() {
		let ct1 = ContentType::new("app", "Model").with_id(42);
		let ct2 = ct1.clone();

		assert_eq!(ct1, ct2);
		assert_eq!(ct1.id, ct2.id);
	}

	/// Test: Registry handles concurrent-like operations
	#[test]
	fn test_registry_multiple_operations() {
		let registry = setup_registry();

		// Simulate multiple operations
		let ct1 = registry.get_or_create("app1", "Model1");
		let ct2 = registry.get_or_create("app2", "Model2");
		let ct1_again = registry.get("app1", "Model1");
		let ct2_by_id = registry.get_by_id(ct2.id.unwrap());

		assert!(ct1_again.is_some());
		assert_eq!(ct1.id, ct1_again.unwrap().id);
		assert!(ct2_by_id.is_some());
		assert_eq!(ct2.id, ct2_by_id.unwrap().id);
	}

	/// Test: Case sensitivity in app and model names
	#[test]
	fn test_case_sensitivity() {
		let registry = setup_registry();

		let ct1 = registry.get_or_create("App", "Model");
		let ct2 = registry.get_or_create("app", "model");

		// They should be different
		assert_ne!(ct1.id, ct2.id);
	}
}
