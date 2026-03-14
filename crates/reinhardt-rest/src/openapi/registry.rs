//! Schema registry for managing reusable component schemas
//!
//! This module provides a centralized registry for managing OpenAPI schemas
//! with automatic deduplication and $ref reference generation.

use super::{Components, RefOr, Schema};
use crate::ComponentsBuilder;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// A registry for managing reusable OpenAPI schemas
///
/// The `SchemaRegistry` provides centralized schema management with automatic
/// deduplication using `$ref` references. When a schema with the same name
/// is registered multiple times, the registry ensures only one definition
/// exists in the `components/schemas` section.
///
/// # Example
///
/// ```rust
/// use reinhardt_rest::openapi::registry::SchemaRegistry;
/// use reinhardt_rest::openapi::{Schema, SchemaExt};
///
/// let registry = SchemaRegistry::new();
///
/// // Register a schema
/// let user_schema = Schema::object_with_properties(
///     vec![
///         ("id", Schema::integer()),
///         ("name", Schema::string()),
///     ],
///     vec!["id", "name"],
/// );
/// registry.register("User", user_schema);
///
/// // Get a $ref to the schema
/// let user_ref = registry.get_ref("User");
/// assert!(user_ref.is_some());
///
/// // Export to components
/// let components = registry.to_components();
/// assert!(components.schemas.contains_key("User"));
/// ```
#[derive(Clone)]
pub struct SchemaRegistry {
	schemas: Arc<Mutex<HashMap<String, Schema>>>,
	references: Arc<Mutex<HashMap<String, usize>>>, // Track reference counts for circular detection
}

impl SchemaRegistry {
	/// Create a new empty schema registry
	pub fn new() -> Self {
		Self {
			schemas: Arc::new(Mutex::new(HashMap::new())),
			references: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Lock the schemas mutex, recovering from poison if necessary
	fn lock_schemas(&self) -> MutexGuard<'_, HashMap<String, Schema>> {
		self.schemas.lock().unwrap_or_else(|e| {
			tracing::warn!("Lock poisoned, recovering: schemas mutex was poisoned");
			e.into_inner()
		})
	}

	/// Lock the references mutex, recovering from poison if necessary
	fn lock_references(&self) -> MutexGuard<'_, HashMap<String, usize>> {
		self.references.lock().unwrap_or_else(|e| {
			tracing::warn!("Lock poisoned, recovering: references mutex was poisoned");
			e.into_inner()
		})
	}

	/// Register a schema with a given name
	///
	/// If a schema with the same name already exists, it will be replaced.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	/// ```
	pub fn register(&self, name: impl Into<String>, schema: Schema) {
		let name = name.into();
		let mut schemas = self.lock_schemas();
		schemas.insert(name, schema);
	}

	/// Get a schema by name
	///
	/// Returns `None` if the schema is not registered.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	///
	/// let schema = registry.get_schema("User");
	/// assert!(schema.is_some());
	/// ```
	pub fn get_schema(&self, name: &str) -> Option<Schema> {
		let schemas = self.lock_schemas();
		schemas.get(name).cloned()
	}

	/// Get a $ref reference to a schema
	///
	/// Returns a `RefOr::Ref` variant pointing to `#/components/schemas/{name}`.
	/// If the schema is not registered, returns `None`.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt, RefOr};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	///
	/// let user_ref = registry.get_ref("User");
	/// assert!(user_ref.is_some());
	///
	/// match user_ref.unwrap() {
	///     RefOr::Ref(ref_obj) => {
	///         assert_eq!(ref_obj.ref_location, "#/components/schemas/User");
	///     }
	///     _ => panic!("Expected Ref variant"),
	/// }
	/// ```
	pub fn get_ref(&self, name: &str) -> Option<RefOr<Schema>> {
		let schemas = self.lock_schemas();
		if schemas.contains_key(name) {
			// Increment reference count for circular detection
			let mut references = self.lock_references();
			*references.entry(name.to_string()).or_insert(0) += 1;

			Some(RefOr::Ref(utoipa::openapi::Ref::new(format!(
				"#/components/schemas/{}",
				name
			))))
		} else {
			None
		}
	}

	/// Check if a schema is registered
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// assert!(!registry.contains("User"));
	///
	/// registry.register("User", Schema::object());
	/// assert!(registry.contains("User"));
	/// ```
	pub fn contains(&self, name: &str) -> bool {
		let schemas = self.lock_schemas();
		schemas.contains_key(name)
	}

	/// Get the number of registered schemas
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// assert_eq!(registry.len(), 0);
	///
	/// registry.register("User", Schema::object());
	/// assert_eq!(registry.len(), 1);
	/// ```
	pub fn len(&self) -> usize {
		let schemas = self.lock_schemas();
		schemas.len()
	}

	/// Check if the registry is empty
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// assert!(registry.is_empty());
	///
	/// registry.register("User", Schema::object());
	/// assert!(!registry.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		let schemas = self.lock_schemas();
		schemas.is_empty()
	}

	/// Detect potential circular references
	///
	/// Returns a list of schema names that might be involved in circular references
	/// (referenced more than once).
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	///
	/// // Get reference twice
	/// let _ = registry.get_ref("User");
	/// let _ = registry.get_ref("User");
	///
	/// let circular = registry.detect_circular_references();
	/// assert!(circular.contains(&"User".to_string()));
	/// ```
	pub fn detect_circular_references(&self) -> Vec<String> {
		let references = self.lock_references();
		references
			.iter()
			.filter(|(_, count)| **count > 1)
			.map(|(name, _)| name.clone())
			.collect()
	}

	/// Clear all registered schemas and reset reference counts
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	/// assert!(!registry.is_empty());
	///
	/// registry.clear();
	/// assert!(registry.is_empty());
	/// ```
	pub fn clear(&self) {
		let mut schemas = self.lock_schemas();
		schemas.clear();

		let mut references = self.lock_references();
		references.clear();
	}

	/// Export all registered schemas to OpenAPI Components
	///
	/// This creates a `Components` object with all registered schemas
	/// in the `schemas` section.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry = SchemaRegistry::new();
	/// registry.register("User", Schema::object());
	/// registry.register("Post", Schema::object());
	///
	/// let components = registry.to_components();
	/// assert_eq!(components.schemas.len(), 2);
	/// assert!(components.schemas.contains_key("User"));
	/// assert!(components.schemas.contains_key("Post"));
	/// ```
	pub fn to_components(&self) -> Components {
		let schemas = self.lock_schemas();
		let mut builder = ComponentsBuilder::new();

		for (name, schema) in schemas.iter() {
			builder = builder.schema(name, schema.clone());
		}

		builder.build()
	}

	/// Merge another registry into this one
	///
	/// Schemas from the other registry will overwrite schemas with the same name
	/// in this registry.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::registry::SchemaRegistry;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let registry1 = SchemaRegistry::new();
	/// registry1.register("User", Schema::object());
	///
	/// let registry2 = SchemaRegistry::new();
	/// registry2.register("Post", Schema::object());
	///
	/// registry1.merge(&registry2);
	/// assert_eq!(registry1.len(), 2);
	/// assert!(registry1.contains("User"));
	/// assert!(registry1.contains("Post"));
	/// ```
	pub fn merge(&self, other: &SchemaRegistry) {
		// Clone data from other first to avoid holding both locks simultaneously,
		// which could cause deadlock if merge is called in opposite order
		let other_schemas: HashMap<String, Schema> = {
			let guard = other.lock_schemas();
			guard.clone()
		};
		let other_references: HashMap<String, usize> = {
			let guard = other.lock_references();
			guard.clone()
		};

		let mut schemas = self.lock_schemas();
		for (name, schema) in other_schemas {
			schemas.insert(name, schema);
		}
		drop(schemas);

		let mut references = self.lock_references();
		for (name, count) in other_references {
			*references.entry(name).or_insert(0) += count;
		}
	}
}

impl Default for SchemaRegistry {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::openapi::SchemaExt;

	#[test]
	fn test_register_and_get_schema() {
		let registry = SchemaRegistry::new();
		let schema = Schema::object();

		registry.register("User", schema.clone());

		let retrieved = registry.get_schema("User");
		assert!(retrieved.is_some());
	}

	#[test]
	fn test_get_ref() {
		let registry = SchemaRegistry::new();
		registry.register("User", Schema::object());

		let user_ref = registry.get_ref("User");
		assert!(user_ref.is_some());

		match user_ref.unwrap() {
			RefOr::Ref(ref_obj) => {
				assert_eq!(ref_obj.ref_location, "#/components/schemas/User");
			}
			_ => panic!("Expected Ref variant"),
		}
	}

	#[test]
	fn test_get_ref_nonexistent() {
		let registry = SchemaRegistry::new();
		let user_ref = registry.get_ref("User");
		assert!(user_ref.is_none());
	}

	#[test]
	fn test_contains() {
		let registry = SchemaRegistry::new();
		assert!(!registry.contains("User"));

		registry.register("User", Schema::object());
		assert!(registry.contains("User"));
	}

	#[test]
	fn test_len_and_is_empty() {
		let registry = SchemaRegistry::new();
		assert_eq!(registry.len(), 0);
		assert!(registry.is_empty());

		registry.register("User", Schema::object());
		assert_eq!(registry.len(), 1);
		assert!(!registry.is_empty());
	}

	#[test]
	fn test_circular_reference_detection() {
		let registry = SchemaRegistry::new();
		registry.register("User", Schema::object());

		// Get reference multiple times
		let _ = registry.get_ref("User");
		let _ = registry.get_ref("User");
		let _ = registry.get_ref("User");

		let circular = registry.detect_circular_references();
		assert!(circular.contains(&"User".to_string()));
	}

	#[test]
	fn test_clear() {
		let registry = SchemaRegistry::new();
		registry.register("User", Schema::object());
		registry.register("Post", Schema::object());

		assert_eq!(registry.len(), 2);

		registry.clear();
		assert_eq!(registry.len(), 0);
		assert!(registry.is_empty());
	}

	#[test]
	fn test_to_components() {
		let registry = SchemaRegistry::new();
		registry.register("User", Schema::object());
		registry.register("Post", Schema::object());

		let components = registry.to_components();
		assert_eq!(components.schemas.len(), 2);
		assert!(components.schemas.contains_key("User"));
		assert!(components.schemas.contains_key("Post"));
	}

	#[test]
	fn test_merge() {
		let registry1 = SchemaRegistry::new();
		registry1.register("User", Schema::object());

		let registry2 = SchemaRegistry::new();
		registry2.register("Post", Schema::object());

		registry1.merge(&registry2);
		assert_eq!(registry1.len(), 2);
		assert!(registry1.contains("User"));
		assert!(registry1.contains("Post"));
	}

	#[test]
	fn test_merge_overwrites() {
		let registry1 = SchemaRegistry::new();
		registry1.register("User", Schema::integer());

		let registry2 = SchemaRegistry::new();
		registry2.register("User", Schema::string());

		registry1.merge(&registry2);
		assert_eq!(registry1.len(), 1);

		let schema = registry1.get_schema("User").unwrap();
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(
					obj.schema_type,
					utoipa::openapi::schema::SchemaType::Type(utoipa::openapi::Type::String)
				));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[test]
	fn test_replace_schema() {
		let registry = SchemaRegistry::new();
		registry.register("User", Schema::integer());

		let schema1 = registry.get_schema("User").unwrap();
		match schema1 {
			Schema::Object(obj) => {
				assert!(matches!(
					obj.schema_type,
					utoipa::openapi::schema::SchemaType::Type(utoipa::openapi::Type::Integer)
				));
			}
			_ => panic!("Expected Object schema"),
		}

		// Replace with new schema
		registry.register("User", Schema::string());

		let schema2 = registry.get_schema("User").unwrap();
		match schema2 {
			Schema::Object(obj) => {
				assert!(matches!(
					obj.schema_type,
					utoipa::openapi::schema::SchemaType::Type(utoipa::openapi::Type::String)
				));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[test]
	fn test_to_components_json_structure() {
		let registry = SchemaRegistry::new();

		// Register schemas with properties
		registry.register(
			"User",
			Schema::object_with_properties(
				vec![
					("id", Schema::integer()),
					("name", Schema::string()),
					("email", Schema::string()),
				],
				vec!["id", "name"],
			),
		);

		registry.register(
			"Post",
			Schema::object_with_properties(
				vec![
					("id", Schema::integer()),
					("title", Schema::string()),
					("content", Schema::string()),
				],
				vec!["id", "title"],
			),
		);

		// Verify registry state before serialization
		assert_eq!(registry.len(), 2, "Registry should contain 2 schemas");
		assert!(registry.contains("User"), "Registry should contain User");
		assert!(registry.contains("Post"), "Registry should contain Post");

		let components = registry.to_components();

		// Serialize to JSON
		let json = serde_json::to_string_pretty(&components)
			.expect("Failed to serialize components to JSON");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse JSON string");

		// Verify top-level structure
		assert!(
			parsed.is_object(),
			"Components JSON should be an object, got: {:?}",
			parsed
		);
		assert!(
			parsed["schemas"].is_object(),
			"schemas field should be an object, got: {:?}",
			parsed["schemas"]
		);
		let schemas = &parsed["schemas"];

		// Verify User schema structure
		assert!(
			schemas["User"].is_object(),
			"User schema should be an object, got: {:?}",
			schemas["User"]
		);
		let user_schema = &schemas["User"];
		assert_eq!(
			user_schema["type"].as_str(),
			Some("object"),
			"User type should be 'object', got: {:?}",
			user_schema["type"]
		);

		// Verify User properties
		let user_props = &user_schema["properties"];
		assert!(
			user_props.is_object(),
			"User properties should be an object, got: {:?}",
			user_props
		);
		assert!(
			user_props["id"].is_object(),
			"User.id should be an object, got: {:?}",
			user_props["id"]
		);
		assert_eq!(
			user_props["id"]["type"].as_str(),
			Some("integer"),
			"User.id type should be 'integer', got: {:?}",
			user_props["id"]["type"]
		);
		assert!(
			user_props["name"].is_object(),
			"User.name should be an object, got: {:?}",
			user_props["name"]
		);
		assert_eq!(
			user_props["name"]["type"].as_str(),
			Some("string"),
			"User.name type should be 'string', got: {:?}",
			user_props["name"]["type"]
		);
		assert!(
			user_props["email"].is_object(),
			"User.email should be an object, got: {:?}",
			user_props["email"]
		);
		assert_eq!(
			user_props["email"]["type"].as_str(),
			Some("string"),
			"User.email type should be 'string', got: {:?}",
			user_props["email"]["type"]
		);

		// Verify User required fields
		let user_required = &user_schema["required"];
		assert!(
			user_required.is_array(),
			"User required should be an array, got: {:?}",
			user_required
		);
		let user_req_arr = user_required.as_array().unwrap();
		assert_eq!(
			user_req_arr.len(),
			2,
			"User required should have 2 items, got: {:?}",
			user_req_arr
		);

		// Verify Post schema structure
		assert!(
			schemas["Post"].is_object(),
			"Post schema should be an object, got: {:?}",
			schemas["Post"]
		);
		let post_schema = &schemas["Post"];
		assert_eq!(
			post_schema["type"].as_str(),
			Some("object"),
			"Post type should be 'object', got: {:?}",
			post_schema["type"]
		);

		// Verify Post properties
		let post_props = &post_schema["properties"];
		assert!(
			post_props.is_object(),
			"Post properties should be an object, got: {:?}",
			post_props
		);
		assert!(
			post_props["id"].is_object(),
			"Post.id should be an object, got: {:?}",
			post_props["id"]
		);
		assert_eq!(
			post_props["id"]["type"].as_str(),
			Some("integer"),
			"Post.id type should be 'integer', got: {:?}",
			post_props["id"]["type"]
		);
		assert!(
			post_props["title"].is_object(),
			"Post.title should be an object, got: {:?}",
			post_props["title"]
		);
		assert_eq!(
			post_props["title"]["type"].as_str(),
			Some("string"),
			"Post.title type should be 'string', got: {:?}",
			post_props["title"]["type"]
		);
		assert!(
			post_props["content"].is_object(),
			"Post.content should be an object, got: {:?}",
			post_props["content"]
		);
		assert_eq!(
			post_props["content"]["type"].as_str(),
			Some("string"),
			"Post.content type should be 'string', got: {:?}",
			post_props["content"]["type"]
		);

		// Verify Post required fields
		let post_required = &post_schema["required"];
		assert!(
			post_required.is_array(),
			"Post required should be an array, got: {:?}",
			post_required
		);
		let post_req_arr = post_required.as_array().unwrap();
		assert_eq!(
			post_req_arr.len(),
			2,
			"Post required should have 2 items, got: {:?}",
			post_req_arr
		);
	}

	#[test]
	fn test_registry_with_refs_json_validation() {
		let registry = SchemaRegistry::new();

		// Register User schema
		registry.register(
			"User",
			Schema::object_with_properties(
				vec![("id", Schema::integer()), ("name", Schema::string())],
				vec!["id", "name"],
			),
		);

		// Verify registry state before getting ref
		assert_eq!(registry.len(), 1, "Registry should contain 1 schema");
		assert!(registry.contains("User"), "Registry should contain User");

		// Get a reference to User - verify it works
		let user_ref = registry.get_ref("User");
		assert!(
			user_ref.is_some(),
			"get_ref should return Some for registered User schema"
		);

		// Build Post schema with author as a reference manually
		// Using ObjectBuilder directly to add RefOr property
		use utoipa::openapi::schema::ObjectBuilder;
		let mut post_builder = ObjectBuilder::new()
			.schema_type(utoipa::openapi::schema::SchemaType::Type(
				utoipa::openapi::Type::Object,
			))
			.property("id", Schema::integer())
			.property("title", Schema::string())
			.required("id")
			.required("title")
			.required("author");

		// Add author property as a RefOr
		post_builder = post_builder.property("author", user_ref.unwrap());

		// Register Post schema
		registry.register("Post", Schema::Object(post_builder.build()));

		// Verify registry state after registering Post
		assert_eq!(registry.len(), 2, "Registry should contain 2 schemas");
		assert!(registry.contains("Post"), "Registry should contain Post");

		let components = registry.to_components();

		// Serialize to JSON
		let json = serde_json::to_string_pretty(&components)
			.expect("Failed to serialize components to JSON");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse JSON string");

		// Verify top-level structure
		assert!(
			parsed.is_object(),
			"Components JSON should be an object, got: {:?}",
			parsed
		);
		assert!(
			parsed["schemas"].is_object(),
			"schemas field should be an object, got: {:?}",
			parsed["schemas"]
		);

		// Verify User schema exists
		assert!(
			parsed["schemas"]["User"].is_object(),
			"User schema should exist, got: {:?}",
			parsed["schemas"]["User"]
		);
		let user_schema = &parsed["schemas"]["User"];
		assert_eq!(
			user_schema["type"].as_str(),
			Some("object"),
			"User type should be 'object', got: {:?}",
			user_schema["type"]
		);

		// Verify Post schema exists
		assert!(
			parsed["schemas"]["Post"].is_object(),
			"Post schema should exist, got: {:?}",
			parsed["schemas"]["Post"]
		);
		let post_schema = &parsed["schemas"]["Post"];
		assert_eq!(
			post_schema["type"].as_str(),
			Some("object"),
			"Post type should be 'object', got: {:?}",
			post_schema["type"]
		);

		// Verify Post schema has $ref to User in author property
		let post_props = &post_schema["properties"];
		assert!(
			post_props.is_object(),
			"Post properties should be an object, got: {:?}",
			post_props
		);
		assert!(
			post_props["author"].is_object(),
			"author property should exist, got: {:?}",
			post_props["author"]
		);
		let author_prop = &post_props["author"];

		// The author property should have a $ref
		assert!(
			author_prop["$ref"].is_string(),
			"author should have $ref field, got: {:?}",
			author_prop
		);
		assert_eq!(
			author_prop["$ref"].as_str(),
			Some("#/components/schemas/User"),
			"author $ref should point to User schema, got: {:?}",
			author_prop["$ref"]
		);

		// Verify Post has other properties
		assert!(
			post_props["id"].is_object(),
			"Post.id should exist, got: {:?}",
			post_props["id"]
		);
		assert!(
			post_props["title"].is_object(),
			"Post.title should exist, got: {:?}",
			post_props["title"]
		);
	}

	#[test]
	fn test_empty_registry_json_structure() {
		let registry = SchemaRegistry::new();

		// Verify registry is empty before serialization
		assert_eq!(registry.len(), 0, "Registry should be empty initially");
		assert!(registry.is_empty(), "Registry should be empty");

		let components = registry.to_components();

		// Serialize to JSON
		let json = serde_json::to_string_pretty(&components)
			.expect("Failed to serialize empty components to JSON");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse JSON string");

		// Verify top-level structure
		assert!(
			parsed.is_object(),
			"Components JSON should be an object, got: {:?}",
			parsed
		);

		// Empty components may not serialize the schemas field, or it may be an empty object
		// Both are valid representations
		if let Some(schemas) = parsed.get("schemas").filter(|s| !s.is_null()) {
			assert!(
				schemas.is_object(),
				"schemas field should be an object if present, got: {:?}",
				schemas
			);
			let schemas_obj = schemas.as_object().unwrap();
			assert_eq!(
				schemas_obj.len(),
				0,
				"schemas object should be empty, got: {:?}",
				schemas_obj
			);
		}
		// If schemas field is not present or is null, that's also valid for empty components
	}
}

//================================================================================
// Global Schema Registry (Inventory-based Auto-registration)
//================================================================================

use super::schema_registration::SchemaRegistration;
use std::sync::LazyLock;

/// Global schema registry initialized from inventory
///
/// This static registry is automatically populated at startup by collecting
/// all `SchemaRegistration` entries submitted via the `inventory` crate.
/// Types annotated with `#[derive(Schema)]` are automatically registered.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_rest::openapi::{Schema, ToSchema};
///
/// #[derive(Schema)]
/// pub struct User {
///     pub id: i64,
///     pub name: String,
/// }
///
/// // The macro automatically generates:
/// // inventory::submit! {
/// //     SchemaRegistration::new("User", User::schema)
/// // }
///
/// // Later, access all registered schemas:
/// use reinhardt_rest::openapi::registry::get_all_schemas;
/// let schemas = get_all_schemas();
/// assert!(schemas.contains_key("User"));
/// ```
pub static GLOBAL_SCHEMA_REGISTRY: LazyLock<HashMap<&'static str, Schema>> = LazyLock::new(|| {
	let mut registry = HashMap::new();

	// Collect all registered schemas from inventory
	for registration in inventory::iter::<SchemaRegistration> {
		let schema = (registration.generator)();
		registry.insert(registration.name, schema);
	}

	registry
});

/// Get all registered schemas from the global registry
///
/// Returns a reference to the global schema registry, which contains all schemas
/// registered via `#[derive(Schema)]` macro.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_rest::openapi::registry::get_all_schemas;
///
/// let schemas = get_all_schemas();
/// for (name, schema) in schemas.iter() {
///     println!("Registered schema: {}", name);
/// }
/// ```
pub fn get_all_schemas() -> &'static HashMap<&'static str, Schema> {
	&GLOBAL_SCHEMA_REGISTRY
}
