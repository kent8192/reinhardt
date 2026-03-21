//! OpenAPI 3.0 types with Reinhardt extensions
//!
//! This module re-exports utoipa's OpenAPI types and provides
//! convenient helper functions and extension traits for easier usage.

// Re-export core utoipa types as Reinhardt's OpenAPI types
pub use utoipa::openapi::{
	Components, Contact, Header, Info, License, OpenApi as OpenApiSchema, PathItem, Paths, RefOr,
	Required, Schema, Server, Tag,
};

// Re-export request/response types
pub use utoipa::openapi::request_body::RequestBody;
pub use utoipa::openapi::response::{Response, Responses};

// Re-export path operation types
pub use utoipa::openapi::path::{Operation, Parameter, ParameterIn};

// Re-export content-related types (MediaType)
pub use utoipa::openapi::Content as MediaType;

// Re-export path-related types
pub use utoipa::openapi::path::ParameterIn as ParameterLocation;

// Re-export security-related types
pub use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};

/// Convenient type alias for API key location configuration.
pub type ApiKeyLocation = utoipa::openapi::security::ApiKeyValue;

/// Convenient type alias for HTTP authentication scheme.
pub type HttpScheme = HttpAuthScheme;

// Re-export builders
pub use utoipa::openapi::path::{OperationBuilder, ParameterBuilder, PathItemBuilder};
pub use utoipa::openapi::request_body::RequestBodyBuilder;
pub use utoipa::openapi::response::{ResponseBuilder, ResponsesBuilder};
pub use utoipa::openapi::schema::{ArrayBuilder, ObjectBuilder, SchemaType};
pub use utoipa::openapi::tag::TagBuilder;
pub use utoipa::openapi::{
	ComponentsBuilder, ContactBuilder, InfoBuilder, OpenApiBuilder, PathsBuilder, ServerBuilder,
};

/// Extension trait for Schema to provide convenient constructor methods
pub trait SchemaExt {
	/// Create a string schema
	fn string() -> Schema;

	/// Create an integer schema
	fn integer() -> Schema;

	/// Create a number (float) schema
	fn number() -> Schema;

	/// Create a boolean schema
	fn boolean() -> Schema;

	/// Create an empty object schema
	fn object() -> Schema;

	/// Create a date schema (string with format: "date")
	fn date() -> Schema;

	/// Create a datetime schema (string with format: "date-time")
	fn datetime() -> Schema;

	/// Create an array schema with the given item schema
	fn array(items: Schema) -> Schema;

	/// Create an object schema with properties and required fields
	fn object_with_properties(
		properties: Vec<(impl Into<String>, Schema)>,
		required: Vec<impl Into<String>>,
	) -> Schema;

	/// Create an object schema with properties, required fields, and description
	fn object_with_description(
		properties: Vec<(impl Into<String>, Schema)>,
		required: Vec<impl Into<String>>,
		description: impl Into<String>,
	) -> Schema;
}

impl SchemaExt for Schema {
	fn string() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::String))
				.build(),
		)
	}

	fn integer() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::Integer))
				.build(),
		)
	}

	fn number() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::Number))
				.build(),
		)
	}

	fn boolean() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::Boolean))
				.build(),
		)
	}

	fn object() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::Object))
				.build(),
		)
	}

	fn date() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::String))
				.format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
					utoipa::openapi::KnownFormat::Date,
				)))
				.build(),
		)
	}

	fn datetime() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(utoipa::openapi::Type::String))
				.format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
					utoipa::openapi::KnownFormat::DateTime,
				)))
				.build(),
		)
	}

	fn array(items: Schema) -> Schema {
		Schema::Array(ArrayBuilder::new().items(RefOr::T(items)).build())
	}

	fn object_with_properties(
		properties: Vec<(impl Into<String>, Schema)>,
		required: Vec<impl Into<String>>,
	) -> Schema {
		let mut builder =
			ObjectBuilder::new().schema_type(SchemaType::Type(utoipa::openapi::Type::Object));

		for (name, schema) in properties {
			builder = builder.property(name, schema);
		}

		for req in required {
			builder = builder.required(req);
		}

		Schema::Object(builder.build())
	}

	fn object_with_description(
		properties: Vec<(impl Into<String>, Schema)>,
		required: Vec<impl Into<String>>,
		description: impl Into<String>,
	) -> Schema {
		let mut builder = ObjectBuilder::new()
			.schema_type(SchemaType::Type(utoipa::openapi::Type::Object))
			.description(Some(description.into()));

		for (name, schema) in properties {
			builder = builder.property(name, schema);
		}

		for req in required {
			builder = builder.required(req);
		}

		Schema::Object(builder.build())
	}
}

/// Extension trait for OpenApiSchema to provide convenient methods
pub trait OpenApiSchemaExt {
	/// Create a new OpenApiSchema with title and version
	fn create(title: impl Into<String>, version: impl Into<String>) -> OpenApiSchema;

	/// Add a path to the schema
	fn add_path(&mut self, path: String, item: PathItem);

	/// Add a tag to the schema
	fn add_tag(&mut self, name: String, description: Option<String>);
}

impl OpenApiSchemaExt for OpenApiSchema {
	fn create(title: impl Into<String>, version: impl Into<String>) -> OpenApiSchema {
		OpenApiBuilder::new()
			.info(InfoBuilder::new().title(title).version(version).build())
			.build()
	}

	fn add_path(&mut self, path: String, item: PathItem) {
		self.paths.paths.insert(path, item);
	}

	fn add_tag(&mut self, name: String, description: Option<String>) {
		let mut builder = TagBuilder::new().name(name);
		if let Some(desc) = description {
			builder = builder.description(Some(desc));
		}
		let tag = builder.build();

		if self.tags.is_none() {
			self.tags = Some(Vec::new());
		}
		if let Some(tags) = &mut self.tags {
			tags.push(tag);
		}
	}
}

/// Extension trait for Operation to provide convenient methods
pub trait OperationExt {
	/// Create a new Operation with default values
	fn create() -> Operation;

	/// Add a parameter to the operation
	fn add_parameter(&mut self, parameter: Parameter);

	/// Add a response to the operation
	fn add_response(&mut self, status: impl Into<String>, response: Response);
}

impl OperationExt for Operation {
	fn create() -> Operation {
		// Operation is non-exhaustive, so we must use Default
		Default::default()
	}

	fn add_parameter(&mut self, parameter: Parameter) {
		if self.parameters.is_none() {
			self.parameters = Some(Vec::new());
		}
		if let Some(params) = &mut self.parameters {
			params.push(parameter);
		}
	}

	fn add_response(&mut self, status: impl Into<String>, response: Response) {
		self.responses
			.responses
			.insert(status.into(), response.into());
	}
}

/// Extension trait for Responses to provide collection methods
pub trait ResponsesExt {
	/// Get the number of responses
	fn len(&self) -> usize;

	/// Check if responses collection is empty
	fn is_empty(&self) -> bool;

	/// Check if a specific status code exists
	fn contains_key(&self, status: &str) -> bool;
}

impl ResponsesExt for Responses {
	fn len(&self) -> usize {
		self.responses.len()
	}

	fn is_empty(&self) -> bool {
		self.responses.is_empty()
	}

	fn contains_key(&self, status: &str) -> bool {
		self.responses.contains_key(status)
	}
}

/// Extension trait for Components to provide convenient methods
pub trait ComponentsExt {
	/// Add a schema to the components
	fn add_schema(&mut self, name: String, schema: Schema);
}

impl ComponentsExt for Components {
	fn add_schema(&mut self, name: String, schema: Schema) {
		self.schemas.insert(name, schema.into());
	}
}

/// Extension trait for PathItem to provide constructor
pub trait PathItemExt {
	/// Create a new PathItem
	fn create() -> PathItem;
}

impl PathItemExt for PathItem {
	fn create() -> PathItem {
		PathItem::default()
	}
}

/// Extension trait for Parameter to provide convenient constructors
pub trait ParameterExt {
	/// Create a new Parameter with ParameterBuilder
	fn new_simple(
		name: impl Into<String>,
		location: ParameterIn,
		schema: Schema,
		required: bool,
	) -> Parameter;

	/// Create a new Parameter with description
	fn new_with_description(
		name: impl Into<String>,
		location: ParameterIn,
		schema: Schema,
		required: bool,
		description: impl Into<String>,
	) -> Parameter;
}

impl ParameterExt for Parameter {
	fn new_simple(
		name: impl Into<String>,
		location: ParameterIn,
		schema: Schema,
		required: bool,
	) -> Parameter {
		ParameterBuilder::new()
			.name(name)
			.parameter_in(location)
			.schema(Some(schema))
			.required(if required {
				utoipa::openapi::Required::True
			} else {
				utoipa::openapi::Required::False
			})
			.build()
	}

	fn new_with_description(
		name: impl Into<String>,
		location: ParameterIn,
		schema: Schema,
		required: bool,
		description: impl Into<String>,
	) -> Parameter {
		ParameterBuilder::new()
			.name(name)
			.parameter_in(location)
			.schema(Some(schema))
			.required(if required {
				utoipa::openapi::Required::True
			} else {
				utoipa::openapi::Required::False
			})
			.description(Some(description.into()))
			.build()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_schema_helpers() {
		// Test string schema
		let string_schema = Schema::string();
		let json =
			serde_json::to_string(&string_schema).expect("Failed to serialize string schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse string schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("string"),
			"String schema type should be 'string'"
		);

		// Test integer schema
		let integer_schema = Schema::integer();
		let json =
			serde_json::to_string(&integer_schema).expect("Failed to serialize integer schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse integer schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("integer"),
			"Integer schema type should be 'integer'"
		);

		// Test boolean schema
		let boolean_schema = Schema::boolean();
		let json =
			serde_json::to_string(&boolean_schema).expect("Failed to serialize boolean schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse boolean schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("boolean"),
			"Boolean schema type should be 'boolean'"
		);

		// Test number schema
		let number_schema = Schema::number();
		let json =
			serde_json::to_string(&number_schema).expect("Failed to serialize number schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse number schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("number"),
			"Number schema type should be 'number'"
		);

		// Test object schema
		let object_schema = Schema::object();
		let json =
			serde_json::to_string(&object_schema).expect("Failed to serialize object schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse object schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("object"),
			"Object schema type should be 'object'"
		);

		// Test date schema
		let date_schema = Schema::date();
		let json = serde_json::to_string(&date_schema).expect("Failed to serialize date schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse date schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("string"),
			"Date schema type should be 'string'"
		);
		assert_eq!(
			parsed["format"].as_str(),
			Some("date"),
			"Date schema format should be 'date'"
		);

		// Test datetime schema
		let datetime_schema = Schema::datetime();
		let json =
			serde_json::to_string(&datetime_schema).expect("Failed to serialize datetime schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse datetime schema JSON");
		assert_eq!(
			parsed["type"].as_str(),
			Some("string"),
			"Datetime schema type should be 'string'"
		);
		assert_eq!(
			parsed["format"].as_str(),
			Some("date-time"),
			"Datetime schema format should be 'date-time'"
		);
	}

	#[test]
	fn test_openapi_schema_new() {
		let schema = <OpenApiSchema as OpenApiSchemaExt>::create("Test API", "1.0.0");

		assert_eq!(
			schema.info.title, "Test API",
			"OpenAPI schema title should match"
		);
		assert_eq!(
			schema.info.version, "1.0.0",
			"OpenAPI schema version should match"
		);

		// Validate JSON structure
		let json = serde_json::to_string(&schema).expect("Failed to serialize OpenAPI schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse OpenAPI schema JSON");

		assert_eq!(
			parsed["openapi"].as_str(),
			Some("3.1.0"),
			"OpenAPI version should be 3.1.0"
		);
		assert!(parsed["info"].is_object(), "Info should be an object");
		assert_eq!(
			parsed["info"]["title"].as_str(),
			Some("Test API"),
			"Info title should match"
		);
		assert_eq!(
			parsed["info"]["version"].as_str(),
			Some("1.0.0"),
			"Info version should match"
		);
	}

	#[test]
	fn test_operation_ext() {
		let mut operation = <Operation as OperationExt>::create();
		let param = ParameterBuilder::new()
			.name("id")
			.parameter_in(ParameterIn::Path)
			.required(Required::True)
			.build();

		operation.add_parameter(param);

		assert!(
			operation.parameters.is_some(),
			"Operation should have parameters"
		);
		assert_eq!(
			operation.parameters.as_ref().unwrap().len(),
			1,
			"Operation should have exactly 1 parameter"
		);

		// Validate JSON structure
		let json = serde_json::to_string(&operation).expect("Failed to serialize Operation");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse Operation JSON");

		assert!(
			parsed["parameters"].is_array(),
			"Parameters should be an array"
		);
		let params = parsed["parameters"]
			.as_array()
			.expect("Parameters should be an array");
		assert_eq!(params.len(), 1, "Should have exactly 1 parameter");
		assert_eq!(
			params[0]["name"].as_str(),
			Some("id"),
			"Parameter name should be 'id'"
		);
		assert_eq!(
			params[0]["in"].as_str(),
			Some("path"),
			"Parameter location should be 'path'"
		);
		assert_eq!(
			params[0]["required"],
			serde_json::Value::Bool(true),
			"Parameter should be required"
		);
	}

	#[test]
	fn test_responses_ext() {
		let response = ResponseBuilder::new().description("Success").build();

		let mut responses = ResponsesBuilder::new().build();
		responses
			.responses
			.insert("200".to_string(), response.into());

		assert_eq!(responses.len(), 1, "Responses should have exactly 1 entry");
		assert!(!responses.is_empty(), "Responses should not be empty");
		assert!(
			responses.contains_key("200"),
			"Responses should contain key '200'"
		);

		// Validate JSON structure
		let json = serde_json::to_string(&responses).expect("Failed to serialize Responses");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse Responses JSON");

		assert!(parsed.is_object(), "Responses should be an object");
		assert!(
			parsed["200"].is_object(),
			"Response '200' should be an object"
		);
		assert_eq!(
			parsed["200"]["description"].as_str(),
			Some("Success"),
			"Response description should be 'Success'"
		);
	}

	#[test]
	fn test_openapi_schema_json_structure() {
		let mut schema = <OpenApiSchema as OpenApiSchemaExt>::create("Test API", "1.0.0");

		// Add a path
		let path_item = PathItemBuilder::new().build();
		schema.add_path("/users".to_string(), path_item);

		// Add a tag
		schema.add_tag("users".to_string(), Some("User operations".to_string()));

		// Serialize to JSON
		let json =
			serde_json::to_string_pretty(&schema).expect("Failed to serialize OpenAPI schema");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse OpenAPI schema JSON");

		// Verify OpenAPI version
		assert_eq!(
			parsed["openapi"].as_str(),
			Some("3.1.0"),
			"OpenAPI version should be 3.1.0"
		);

		// Verify info structure
		assert!(parsed["info"].is_object(), "Info should be an object");
		assert_eq!(
			parsed["info"]["title"].as_str(),
			Some("Test API"),
			"Info title should be 'Test API'"
		);
		assert_eq!(
			parsed["info"]["version"].as_str(),
			Some("1.0.0"),
			"Info version should be '1.0.0'"
		);

		// Verify paths structure
		assert!(parsed["paths"].is_object(), "Paths should be an object");
		assert!(
			parsed["paths"]["/users"].is_object(),
			"Path '/users' should be an object"
		);

		// Verify tags structure
		assert!(parsed["tags"].is_array(), "Tags should be an array");
		let tags = parsed["tags"].as_array().expect("Tags should be an array");
		assert_eq!(tags.len(), 1, "Should have exactly 1 tag");
		assert_eq!(
			tags[0]["name"].as_str(),
			Some("users"),
			"Tag name should be 'users'"
		);
		assert_eq!(
			tags[0]["description"].as_str(),
			Some("User operations"),
			"Tag description should be 'User operations'"
		);
	}

	#[test]
	fn test_schema_with_components() {
		// Create components with schemas
		let mut components = ComponentsBuilder::new();
		components = components.schema("User", Schema::object());
		components = components.schema("Post", Schema::object());

		let mut api_schema = OpenApiBuilder::new()
			.info(InfoBuilder::new().title("API").version("1.0.0").build())
			.components(Some(components.build()))
			.build();

		api_schema.add_path("/users".to_string(), PathItemBuilder::new().build());

		// Serialize and validate JSON structure
		let json = serde_json::to_string_pretty(&api_schema)
			.expect("Failed to serialize API schema with components");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse API schema JSON");

		// Verify components/schemas structure
		assert!(
			parsed["components"].is_object(),
			"Components should be an object"
		);
		assert!(
			parsed["components"]["schemas"].is_object(),
			"Components.schemas should be an object"
		);

		let schemas = &parsed["components"]["schemas"];
		assert!(
			schemas["User"].is_object(),
			"User schema should be an object"
		);
		assert_eq!(
			schemas["User"]["type"].as_str(),
			Some("object"),
			"User schema type should be 'object'"
		);
		assert!(
			schemas["Post"].is_object(),
			"Post schema should be an object"
		);
		assert_eq!(
			schemas["Post"]["type"].as_str(),
			Some("object"),
			"Post schema type should be 'object'"
		);

		// Verify paths exist
		assert!(parsed["paths"].is_object(), "Paths should be an object");
		assert!(
			parsed["paths"]["/users"].is_object(),
			"Path '/users' should be an object"
		);
	}

	#[test]
	fn test_parameter_json_structure() {
		let param = Parameter::new_simple("id", ParameterIn::Path, Schema::integer(), true);

		let json = serde_json::to_string_pretty(&param).expect("Failed to serialize Parameter");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse Parameter JSON");

		// Verify parameter structure
		assert_eq!(
			parsed["name"].as_str(),
			Some("id"),
			"Parameter name should be 'id'"
		);
		assert_eq!(
			parsed["in"].as_str(),
			Some("path"),
			"Parameter location should be 'path'"
		);
		assert_eq!(
			parsed["required"],
			serde_json::Value::Bool(true),
			"Parameter should be required"
		);

		// Verify schema
		assert!(
			parsed["schema"].is_object(),
			"Parameter schema should be an object"
		);
		assert_eq!(
			parsed["schema"]["type"].as_str(),
			Some("integer"),
			"Parameter schema type should be 'integer'"
		);
	}

	#[test]
	fn test_operation_json_structure() {
		let mut operation = <Operation as OperationExt>::create();

		// Add parameter
		let param = Parameter::new_simple("id", ParameterIn::Path, Schema::integer(), true);
		operation.add_parameter(param);

		// Add response
		let response = ResponseBuilder::new().description("Success").build();
		operation.add_response("200", response);

		let json = serde_json::to_string_pretty(&operation).expect("Failed to serialize Operation");
		let parsed: serde_json::Value =
			serde_json::from_str(&json).expect("Failed to parse Operation JSON");

		// Verify parameters
		assert!(
			parsed["parameters"].is_array(),
			"Operation parameters should be an array"
		);
		let params = parsed["parameters"]
			.as_array()
			.expect("Parameters should be an array");
		assert_eq!(params.len(), 1, "Should have exactly 1 parameter");
		assert_eq!(
			params[0]["name"].as_str(),
			Some("id"),
			"Parameter name should be 'id'"
		);
		assert_eq!(
			params[0]["in"].as_str(),
			Some("path"),
			"Parameter location should be 'path'"
		);
		assert_eq!(
			params[0]["required"],
			serde_json::Value::Bool(true),
			"Parameter should be required"
		);
		assert!(
			params[0]["schema"].is_object(),
			"Parameter schema should be an object"
		);
		assert_eq!(
			params[0]["schema"]["type"].as_str(),
			Some("integer"),
			"Parameter schema type should be 'integer'"
		);

		// Verify responses
		assert!(
			parsed["responses"].is_object(),
			"Operation responses should be an object"
		);
		assert!(
			parsed["responses"]["200"].is_object(),
			"Response '200' should be an object"
		);
		assert_eq!(
			parsed["responses"]["200"]["description"].as_str(),
			Some("Success"),
			"Response description should be 'Success'"
		);
	}
}
