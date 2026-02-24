//! OpenAPI schema generator with registry integration
//!
//! This module provides the main schema generator that integrates with the schema registry,
//! enum schema builder, and serde attributes support.

use super::endpoint_inspector::EndpointInspector;
use super::registry::SchemaRegistry;
use super::{OpenApiSchema, PathItem, SchemaError};
use indexmap::IndexMap;

/// Schema generator for OpenAPI schemas
///
/// This is a builder for creating OpenAPI 3.0 schemas with support for:
/// - Schema registry for component reuse
/// - Advanced enum handling
/// - Serde attributes integration
///
/// # Example
///
/// ```rust
/// use reinhardt_rest::openapi::generator::SchemaGenerator;
/// use reinhardt_rest::openapi::{Schema, SchemaExt};
///
/// let mut generator = SchemaGenerator::new()
///     .title("My API")
///     .version("1.0.0")
///     .description("API documentation");
///
/// // Register schemas
/// generator.registry().register("User", Schema::object_with_properties(
///     vec![
///         ("id", Schema::integer()),
///         ("name", Schema::string()),
///     ],
///     vec!["id", "name"],
/// ));
///
/// // Generate OpenAPI schema
/// let schema = generator.generate().unwrap();
/// ```
pub struct SchemaGenerator {
	title: String,
	version: String,
	description: Option<String>,
	registry: SchemaRegistry,
	paths: IndexMap<String, PathItem>,
}

impl SchemaGenerator {
	/// Create a new schema generator
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new();
	/// ```
	pub fn new() -> Self {
		Self {
			title: String::new(),
			version: "1.0.0".to_string(),
			description: None,
			registry: SchemaRegistry::new(),
			paths: IndexMap::new(),
		}
	}

	/// Set the API title
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .title("My API");
	/// ```
	pub fn title(mut self, title: impl Into<String>) -> Self {
		self.title = title.into();
		self
	}

	/// Set the API version
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .version("2.0.0");
	/// ```
	pub fn version(mut self, version: impl Into<String>) -> Self {
		self.version = version.into();
		self
	}

	/// Set the API description
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .description("My awesome API");
	/// ```
	pub fn description(mut self, description: impl Into<String>) -> Self {
		self.description = Some(description.into());
		self
	}

	/// Get a reference to the schema registry
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let mut generator = SchemaGenerator::new();
	/// generator.registry().register("User", Schema::object());
	///
	/// assert!(generator.registry().contains("User"));
	/// ```
	pub fn registry(&mut self) -> &mut SchemaRegistry {
		&mut self.registry
	}

	/// Get the schema registry
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new();
	/// let registry = generator.get_registry();
	/// assert!(registry.is_empty());
	/// ```
	pub fn get_registry(&self) -> &SchemaRegistry {
		&self.registry
	}

	/// Add function-based endpoints from HTTP method decorators
	///
	/// This method uses the `EndpointInspector` to collect endpoint metadata
	/// from the global inventory and adds them to the OpenAPI schema as paths.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .title("My API")
	///     .version("1.0.0")
	///     .add_function_based_endpoints();
	///
	/// // All endpoints decorated with #[get], #[post], etc. are now included
	/// let schema = generator.generate().unwrap();
	/// ```
	pub fn add_function_based_endpoints(mut self) -> Self {
		let inspector = EndpointInspector::new();

		// Extract paths from inventory
		match inspector.extract_paths() {
			Ok(paths) => {
				// Merge with existing paths
				for (path, path_item) in paths {
					self.paths.insert(path, path_item);
				}
			}
			Err(e) => {
				// Log error but don't fail the build
				eprintln!("Warning: Failed to extract function-based endpoints: {}", e);
			}
		}

		self
	}

	/// Add a single server function endpoint (no-op when pages feature is disabled)
	#[cfg(not(feature = "pages"))]
	pub fn add_server_fn<S>(self, _marker: S) -> Self {
		eprintln!("Warning: add_server_fn() requires 'pages' feature to be enabled");
		self
	}

	/// Generate the OpenAPI schema
	///
	/// This generates an OpenAPI 3.0 schema with all registered components.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	/// use reinhardt_rest::openapi::{Schema, SchemaExt};
	///
	/// let mut generator = SchemaGenerator::new()
	///     .title("My API")
	///     .version("1.0.0");
	///
	/// generator.registry().register("User", Schema::object());
	///
	/// let schema = generator.generate().unwrap();
	/// assert_eq!(schema.info.title, "My API");
	/// ```
	pub fn generate(&self) -> Result<OpenApiSchema, SchemaError> {
		use utoipa::openapi::{InfoBuilder, OpenApiBuilder};

		let mut info_builder = InfoBuilder::new().title(&self.title).version(&self.version);

		if let Some(desc) = &self.description {
			info_builder = info_builder.description(Some(desc.as_str()));
		}

		let components = self.registry.to_components();

		let mut builder = OpenApiBuilder::new()
			.info(info_builder.build())
			.components(Some(components));

		// Add paths if any exist
		if !self.paths.is_empty() {
			let mut paths_builder = utoipa::openapi::PathsBuilder::new();
			for (path, path_item) in &self.paths {
				paths_builder = paths_builder.path(path, path_item.clone());
			}
			builder = builder.paths(paths_builder);
		}

		Ok(builder.build())
	}

	/// Generate OpenAPI schema as JSON string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .title("My API")
	///     .version("1.0.0");
	///
	/// let json = generator.to_json().unwrap();
	/// assert!(json.contains("\"title\": \"My API\""));
	/// ```
	pub fn to_json(&self) -> Result<String, SchemaError> {
		let schema = self.generate()?;
		serde_json::to_string_pretty(&schema).map_err(SchemaError::from)
	}

	/// Generate OpenAPI schema as YAML string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_rest::openapi::generator::SchemaGenerator;
	///
	/// let generator = SchemaGenerator::new()
	///     .title("My API")
	///     .version("1.0.0");
	///
	/// let yaml = generator.to_yaml().unwrap();
	/// assert!(yaml.contains("title: My API"));
	/// ```
	pub fn to_yaml(&self) -> Result<String, SchemaError> {
		let schema = self.generate()?;
		serde_yaml::to_string(&schema).map_err(|e| SchemaError::SerializationError(e.to_string()))
	}
}

impl Default for SchemaGenerator {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::openapi::Schema;
	use crate::openapi::SchemaExt;

	#[test]
	fn test_new_generator() {
		let generator = SchemaGenerator::new();
		assert_eq!(generator.version, "1.0.0");
		assert!(generator.title.is_empty());
		assert!(generator.description.is_none());
	}

	#[test]
	fn test_builder_pattern() {
		let generator = SchemaGenerator::new()
			.title("Test API")
			.version("2.0.0")
			.description("Test description");

		assert_eq!(generator.title, "Test API");
		assert_eq!(generator.version, "2.0.0");
		assert_eq!(generator.description, Some("Test description".to_string()));
	}

	#[test]
	fn test_generate_basic_schema() {
		let generator = SchemaGenerator::new()
			.title("My API")
			.version("1.0.0")
			.description("Test API");

		let schema = generator.generate().unwrap();
		assert_eq!(schema.info.title, "My API");
		assert_eq!(schema.info.version, "1.0.0");
		assert_eq!(schema.info.description, Some("Test API".to_string()));
	}

	#[test]
	fn test_registry_integration() {
		let mut generator = SchemaGenerator::new();

		generator.registry().register("User", Schema::object());
		generator.registry().register("Post", Schema::object());

		assert!(generator.registry().contains("User"));
		assert!(generator.registry().contains("Post"));

		let schema = generator.generate().unwrap();
		assert!(schema.components.is_some());

		let components = schema.components.unwrap();
		assert_eq!(components.schemas.len(), 2);
		assert!(components.schemas.contains_key("User"));
		assert!(components.schemas.contains_key("Post"));
	}

	#[test]
	fn test_to_json() {
		let generator = SchemaGenerator::new().title("My API").version("1.0.0");

		let json = generator.to_json().unwrap();

		// Parse JSON to verify structure
		let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");

		// Verify OpenAPI version
		assert_eq!(
			parsed["openapi"].as_str(),
			Some("3.1.0"),
			"OpenAPI version should be 3.1.0"
		);

		// Verify info object
		assert!(parsed["info"].is_object(), "info should be an object");
		assert_eq!(
			parsed["info"]["title"].as_str(),
			Some("My API"),
			"title should match"
		);
		assert_eq!(
			parsed["info"]["version"].as_str(),
			Some("1.0.0"),
			"version should match"
		);

		// Verify paths object exists
		assert!(
			parsed["paths"].is_object(),
			"paths should be an object (can be empty)"
		);

		// Verify components object exists
		assert!(
			parsed["components"].is_object(),
			"components should be an object"
		);
	}

	#[test]
	fn test_to_yaml() {
		let generator = SchemaGenerator::new().title("My API").version("1.0.0");

		let yaml = generator.to_yaml().unwrap();
		assert!(yaml.contains("title: My API"));
		assert!(yaml.contains("version: 1.0.0"));
	}

	#[test]
	fn test_get_registry() {
		let mut generator = SchemaGenerator::new();
		generator.registry().register("User", Schema::object());

		let registry = generator.get_registry();
		assert!(registry.contains("User"));
	}

	#[test]
	fn test_registry_with_nested_schemas() {
		let mut generator = SchemaGenerator::new();

		// Register User schema
		generator.registry().register(
			"User",
			Schema::object_with_properties(
				vec![("id", Schema::integer()), ("name", Schema::string())],
				vec!["id", "name"],
			),
		);

		// Register Post schema with reference to User
		// In practice, you'd build the schema differently to include refs
		generator.registry().register(
			"Post",
			Schema::object_with_properties(
				vec![("id", Schema::integer()), ("title", Schema::string())],
				vec!["id", "title"],
			),
		);

		let schema = generator.generate().unwrap();
		let components = schema.components.unwrap();

		assert_eq!(components.schemas.len(), 2);
		assert!(components.schemas.contains_key("User"));
		assert!(components.schemas.contains_key("Post"));

		// Verify get_ref returns a reference
		let user_ref = generator.registry().get_ref("User");
		assert!(user_ref.is_some());
		match user_ref.unwrap() {
			utoipa::openapi::RefOr::Ref(_) => {
				// Successfully got a reference
			}
			_ => panic!("Expected Ref variant"),
		}
	}

	#[test]
	fn test_empty_registry() {
		let generator = SchemaGenerator::new().title("Empty API").version("1.0.0");

		let schema = generator.generate().unwrap();
		assert!(schema.components.is_some());

		let components = schema.components.unwrap();
		assert_eq!(components.schemas.len(), 0);
	}

	#[test]
	fn test_to_json_with_schemas() {
		let mut generator = SchemaGenerator::new()
			.title("Test API")
			.version("1.0.0")
			.description("API with schemas");

		// Register schemas
		generator.registry().register(
			"User",
			Schema::object_with_properties(
				vec![("id", Schema::integer()), ("name", Schema::string())],
				vec!["id", "name"],
			),
		);

		generator.registry().register(
			"Post",
			Schema::object_with_properties(
				vec![("id", Schema::integer()), ("title", Schema::string())],
				vec!["id"],
			),
		);

		let json = generator.to_json().unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");

		// Verify OpenAPI structure
		assert_eq!(parsed["openapi"].as_str(), Some("3.1.0"));

		// Verify info
		assert_eq!(parsed["info"]["title"].as_str(), Some("Test API"));
		assert_eq!(parsed["info"]["version"].as_str(), Some("1.0.0"));
		assert_eq!(
			parsed["info"]["description"].as_str(),
			Some("API with schemas")
		);

		// Verify components/schemas
		let components = &parsed["components"];
		assert!(components.is_object(), "components should be an object");

		let schemas = &components["schemas"];
		assert!(schemas.is_object(), "schemas should be an object");

		// Verify User schema
		let user_schema = &schemas["User"];
		assert!(user_schema.is_object(), "User schema should be an object");
		assert_eq!(user_schema["type"].as_str(), Some("object"));

		let user_props = &user_schema["properties"];
		assert!(
			user_props.is_object(),
			"User properties should be an object"
		);
		assert!(user_props["id"].is_object(), "id property should exist");
		assert!(user_props["name"].is_object(), "name property should exist");

		let user_required = &user_schema["required"];
		assert!(user_required.is_array(), "required should be an array");
		assert_eq!(user_required.as_array().unwrap().len(), 2);

		// Verify Post schema
		let post_schema = &schemas["Post"];
		assert!(post_schema.is_object(), "Post schema should be an object");
		assert_eq!(post_schema["type"].as_str(), Some("object"));

		let post_props = &post_schema["properties"];
		assert!(
			post_props.is_object(),
			"Post properties should be an object"
		);
		assert!(post_props["id"].is_object(), "id property should exist");
		assert!(
			post_props["title"].is_object(),
			"title property should exist"
		);
	}

	#[test]
	fn test_json_schema_validation() {
		let mut generator = SchemaGenerator::new()
			.title("Validation Test")
			.version("2.0.0");

		generator.registry().register(
			"Product",
			Schema::object_with_properties(
				vec![
					("id", Schema::integer()),
					("name", Schema::string()),
					("price", Schema::number()),
					("in_stock", Schema::boolean()),
				],
				vec!["id", "name", "price"],
			),
		);

		let json = generator.to_json().unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");

		// Validate Product schema structure
		let product = &parsed["components"]["schemas"]["Product"];
		assert_eq!(product["type"].as_str(), Some("object"));

		// Validate all properties exist
		let props = &product["properties"];
		assert!(props["id"].is_object());
		assert!(props["name"].is_object());
		assert!(props["price"].is_object());
		assert!(props["in_stock"].is_object());

		// Validate property types
		assert_eq!(props["id"]["type"].as_str(), Some("integer"));
		assert_eq!(props["name"]["type"].as_str(), Some("string"));
		assert_eq!(props["price"]["type"].as_str(), Some("number"));
		assert_eq!(props["in_stock"]["type"].as_str(), Some("boolean"));

		// Validate required fields
		let required = product["required"].as_array().unwrap();
		assert_eq!(required.len(), 3);
		assert!(required.contains(&serde_json::Value::String("id".to_string())));
		assert!(required.contains(&serde_json::Value::String("name".to_string())));
		assert!(required.contains(&serde_json::Value::String("price".to_string())));
		assert!(!required.contains(&serde_json::Value::String("in_stock".to_string())));
	}
}
