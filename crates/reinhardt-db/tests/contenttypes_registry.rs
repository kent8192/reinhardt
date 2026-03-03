use reinhardt_db::contenttypes::{
	CONTENT_TYPE_REGISTRY, ContentType, ContentTypeRegistry, GenericForeignKey,
};
use rstest::rstest;
use serial_test::serial;

// ============================================================================
// ContentType basic tests
// ============================================================================

#[rstest]
fn test_content_type_new_sets_fields() {
	// Arrange
	let app_label = "blog";
	let model = "Post";

	// Act
	let ct = ContentType::new(app_label, model);

	// Assert
	assert_eq!(ct.app_label, "blog");
	assert_eq!(ct.model, "Post");
	assert_eq!(ct.id, None);
}

#[rstest]
fn test_content_type_with_id() {
	// Arrange
	let ct = ContentType::new("auth", "User");

	// Act
	let ct_with_id = ct.with_id(42);

	// Assert
	assert_eq!(ct_with_id.id, Some(42));
	assert_eq!(ct_with_id.app_label, "auth");
	assert_eq!(ct_with_id.model, "User");
}

#[rstest]
fn test_content_type_natural_key() {
	// Arrange
	let ct = ContentType::new("shop", "Product");

	// Act
	let (app, model) = ct.natural_key();

	// Assert
	assert_eq!(app, "shop");
	assert_eq!(model, "Product");
}

#[rstest]
fn test_content_type_qualified_name() {
	// Arrange
	let ct = ContentType::new("myapp", "Article");

	// Act
	let name = ct.qualified_name();

	// Assert
	assert_eq!(name, "myapp.Article");
}

#[rstest]
fn test_content_type_clone() {
	// Arrange
	let ct = ContentType::new("auth", "User").with_id(10);

	// Act
	let cloned = ct.clone();

	// Assert
	assert_eq!(ct, cloned);
	assert_eq!(ct.id, cloned.id);
	assert_eq!(ct.app_label, cloned.app_label);
	assert_eq!(ct.model, cloned.model);
}

#[rstest]
fn test_content_type_partial_eq_same() {
	// Arrange
	let ct1 = ContentType::new("app", "Model");
	let ct2 = ContentType::new("app", "Model");

	// Act & Assert
	assert_eq!(ct1, ct2);
}

#[rstest]
fn test_content_type_partial_eq_different_app() {
	// Arrange
	let ct1 = ContentType::new("app1", "Model");
	let ct2 = ContentType::new("app2", "Model");

	// Act & Assert
	assert_ne!(ct1, ct2);
}

#[rstest]
fn test_content_type_partial_eq_different_model() {
	// Arrange
	let ct1 = ContentType::new("app", "Model1");
	let ct2 = ContentType::new("app", "Model2");

	// Act & Assert
	assert_ne!(ct1, ct2);
}

#[rstest]
fn test_content_type_serialize_deserialize() {
	// Arrange
	let ct = ContentType::new("blog", "Post").with_id(5);

	// Act
	let json = serde_json::to_string(&ct).expect("serialization should succeed");
	let deserialized: ContentType =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert_eq!(ct, deserialized);
	assert_eq!(deserialized.id, Some(5));
	assert_eq!(deserialized.app_label, "blog");
	assert_eq!(deserialized.model, "Post");
}

// ============================================================================
// ContentTypeRegistry tests
// ============================================================================

#[rstest]
fn test_registry_new_creates_empty() {
	// Arrange & Act
	let registry = ContentTypeRegistry::new();

	// Assert
	assert_eq!(registry.all().len(), 0);
}

#[rstest]
fn test_registry_register_auto_assigns_id() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	let ct = ContentType::new("app", "Model");

	// Act
	let registered = registry.register(ct);

	// Assert
	assert!(registered.id.is_some());
	assert_eq!(registered.id, Some(1));
	assert_eq!(registered.app_label, "app");
	assert_eq!(registered.model, "Model");
}

#[rstest]
fn test_registry_register_increments_id() {
	// Arrange
	let registry = ContentTypeRegistry::new();

	// Act
	let ct1 = registry.register(ContentType::new("app1", "Model1"));
	let ct2 = registry.register(ContentType::new("app2", "Model2"));

	// Assert
	assert_eq!(ct1.id, Some(1));
	assert_eq!(ct2.id, Some(2));
}

#[rstest]
fn test_registry_get_by_name() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	registry.register(ContentType::new("blog", "Post"));

	// Act
	let found = registry.get("blog", "Post");
	let not_found = registry.get("blog", "Comment");

	// Assert
	assert!(found.is_some());
	assert_eq!(found.unwrap().model, "Post");
	assert!(not_found.is_none());
}

#[rstest]
fn test_registry_get_by_id() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	let ct = registry.register(ContentType::new("auth", "User"));
	let id = ct.id.unwrap();

	// Act
	let found = registry.get_by_id(id);
	let not_found = registry.get_by_id(99999);

	// Assert
	assert!(found.is_some());
	assert_eq!(found.unwrap().app_label, "auth");
	assert!(not_found.is_none());
}

#[rstest]
fn test_registry_get_or_create_creates_new() {
	// Arrange
	let registry = ContentTypeRegistry::new();

	// Act
	let ct = registry.get_or_create("myapp", "MyModel");

	// Assert
	assert!(ct.id.is_some());
	assert_eq!(ct.app_label, "myapp");
	assert_eq!(ct.model, "MyModel");
}

#[rstest]
fn test_registry_get_or_create_returns_existing() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	let original = registry.register(ContentType::new("myapp", "MyModel"));

	// Act
	let retrieved = registry.get_or_create("myapp", "MyModel");

	// Assert
	assert_eq!(original.id, retrieved.id);
	assert_eq!(original.app_label, retrieved.app_label);
	assert_eq!(original.model, retrieved.model);
}

#[rstest]
fn test_registry_all_returns_all_registered() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	registry.register(ContentType::new("app1", "Model1"));
	registry.register(ContentType::new("app2", "Model2"));
	registry.register(ContentType::new("app3", "Model3"));

	// Act
	let all = registry.all();

	// Assert
	assert_eq!(all.len(), 3);
}

#[rstest]
fn test_registry_clear_resets_registry() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	registry.register(ContentType::new("app1", "Model1"));
	registry.register(ContentType::new("app2", "Model2"));
	assert_eq!(registry.all().len(), 2);

	// Act
	registry.clear();

	// Assert
	assert_eq!(registry.all().len(), 0);
	assert!(registry.get("app1", "Model1").is_none());
	assert!(registry.get_by_id(1).is_none());
}

#[rstest]
fn test_registry_clear_resets_id_counter() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	registry.register(ContentType::new("app1", "Model1"));
	registry.clear();

	// Act
	let ct = registry.register(ContentType::new("app2", "Model2"));

	// Assert
	assert_eq!(ct.id, Some(1));
}

#[rstest]
fn test_registry_duplicate_registration_returns_existing() {
	// Arrange
	let registry = ContentTypeRegistry::new();
	let first = registry.register(ContentType::new("blog", "Post"));

	// Act
	let duplicate = registry.register(ContentType::new("blog", "Post"));

	// Assert
	assert_eq!(first.id, duplicate.id);
	assert_eq!(first.app_label, duplicate.app_label);
	assert_eq!(first.model, duplicate.model);
	assert_eq!(registry.all().len(), 1);
}

// ============================================================================
// GenericForeignKey tests
// ============================================================================

#[rstest]
fn test_generic_foreign_key_new() {
	// Arrange & Act
	let gfk = GenericForeignKey::new();

	// Assert
	assert!(!gfk.is_set());
	assert_eq!(gfk.content_type_id, None);
	assert_eq!(gfk.object_id, None);
}

#[rstest]
fn test_generic_foreign_key_set() {
	// Arrange
	let ct = ContentType::new("blog", "Post").with_id(7);
	let mut gfk = GenericForeignKey::new();

	// Act
	gfk.set(&ct, 42);

	// Assert
	assert!(gfk.is_set());
	assert_eq!(gfk.content_type_id, Some(7));
	assert_eq!(gfk.object_id, Some(42));
}

#[rstest]
fn test_generic_foreign_key_is_set_requires_both_fields() {
	// Arrange
	let mut gfk = GenericForeignKey::new();

	// Act & Assert - neither field set
	assert!(!gfk.is_set());

	// Only content_type_id set
	gfk.content_type_id = Some(1);
	assert!(!gfk.is_set());

	// Only object_id set
	gfk.content_type_id = None;
	gfk.object_id = Some(1);
	assert!(!gfk.is_set());

	// Both set
	gfk.content_type_id = Some(1);
	assert!(gfk.is_set());
}

#[rstest]
fn test_generic_foreign_key_clear() {
	// Arrange
	let ct = ContentType::new("shop", "Product").with_id(3);
	let mut gfk = GenericForeignKey::new();
	gfk.set(&ct, 100);
	assert!(gfk.is_set());

	// Act
	gfk.clear();

	// Assert
	assert!(!gfk.is_set());
	assert_eq!(gfk.content_type_id, None);
	assert_eq!(gfk.object_id, None);
}

#[rstest]
#[serial(content_type_registry)]
fn test_generic_foreign_key_get_content_type() {
	// Arrange
	CONTENT_TYPE_REGISTRY.clear();
	let ct = CONTENT_TYPE_REGISTRY.register(ContentType::new("blog", "Article"));
	let mut gfk = GenericForeignKey::new();
	gfk.set(&ct, 55);

	// Act
	let retrieved = gfk.get_content_type();

	// Assert
	assert!(retrieved.is_some());
	let retrieved_ct = retrieved.unwrap();
	assert_eq!(retrieved_ct.app_label, "blog");
	assert_eq!(retrieved_ct.model, "Article");

	// Cleanup
	CONTENT_TYPE_REGISTRY.clear();
}

#[rstest]
#[serial(content_type_registry)]
fn test_generic_foreign_key_get_content_type_not_registered() {
	// Arrange
	CONTENT_TYPE_REGISTRY.clear();
	let mut gfk = GenericForeignKey::new();
	gfk.content_type_id = Some(9999);
	gfk.object_id = Some(1);

	// Act
	let retrieved = gfk.get_content_type();

	// Assert
	assert!(retrieved.is_none());

	// Cleanup
	CONTENT_TYPE_REGISTRY.clear();
}

// ============================================================================
// Global CONTENT_TYPE_REGISTRY tests
// ============================================================================

#[rstest]
#[serial(content_type_registry)]
fn test_global_registry_register_and_retrieve() {
	// Arrange
	CONTENT_TYPE_REGISTRY.clear();

	// Act
	let ct = CONTENT_TYPE_REGISTRY.register(ContentType::new("global_app", "GlobalModel"));

	// Assert
	let found = CONTENT_TYPE_REGISTRY.get("global_app", "GlobalModel");
	assert!(found.is_some());
	assert_eq!(found.unwrap().id, ct.id);

	let found_by_id = CONTENT_TYPE_REGISTRY.get_by_id(ct.id.unwrap());
	assert!(found_by_id.is_some());
	assert_eq!(found_by_id.unwrap().app_label, "global_app");

	// Cleanup
	CONTENT_TYPE_REGISTRY.clear();
}

#[rstest]
#[serial(content_type_registry)]
fn test_global_registry_get_or_create() {
	// Arrange
	CONTENT_TYPE_REGISTRY.clear();

	// Act
	let ct1 = CONTENT_TYPE_REGISTRY.get_or_create("test_app", "TestModel");
	let ct2 = CONTENT_TYPE_REGISTRY.get_or_create("test_app", "TestModel");

	// Assert
	assert_eq!(ct1.id, ct2.id);
	assert_eq!(CONTENT_TYPE_REGISTRY.all().len(), 1);

	// Cleanup
	CONTENT_TYPE_REGISTRY.clear();
}

#[rstest]
#[serial(content_type_registry)]
fn test_global_registry_clear() {
	// Arrange
	CONTENT_TYPE_REGISTRY.clear();
	CONTENT_TYPE_REGISTRY.register(ContentType::new("temp", "TempModel"));
	assert_eq!(CONTENT_TYPE_REGISTRY.all().len(), 1);

	// Act
	CONTENT_TYPE_REGISTRY.clear();

	// Assert
	assert_eq!(CONTENT_TYPE_REGISTRY.all().len(), 0);
}
