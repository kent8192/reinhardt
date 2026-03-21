//! OpenAPI schema generation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenAPI 3.0 specification document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAPISpec {
	/// OpenAPI specification version (e.g., `"3.0.0"`).
	pub openapi: String,
	/// Metadata about the API.
	pub info: Info,
	/// Available API paths and their operations.
	pub paths: HashMap<String, PathItem>,
	/// Reusable schema components.
	pub components: Option<Components>,
}

impl OpenAPISpec {
	/// Create a new `OpenAPISpec` with the given info and OpenAPI version 3.0.0.
	pub fn new(info: Info) -> Self {
		Self {
			openapi: "3.0.0".to_string(),
			info,
			paths: HashMap::new(),
			components: None,
		}
	}
}

/// API metadata including title, version, and description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
	/// Title of the API.
	pub title: String,
	/// Version of the API.
	pub version: String,
	/// Optional description of the API.
	pub description: Option<String>,
}

impl Info {
	/// Create a new `Info` with the given title and version.
	pub fn new(title: String, version: String) -> Self {
		Self {
			title,
			version,
			description: None,
		}
	}
}

/// Operations available on a single API path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
	/// GET operation.
	pub get: Option<Operation>,
	/// POST operation.
	pub post: Option<Operation>,
	/// PUT operation.
	pub put: Option<Operation>,
	/// DELETE operation.
	pub delete: Option<Operation>,
	/// PATCH operation.
	pub patch: Option<Operation>,
}

impl PathItem {
	/// Create a new empty `PathItem` with no operations.
	pub fn new() -> Self {
		Self {
			get: None,
			post: None,
			put: None,
			delete: None,
			patch: None,
		}
	}
}

impl Default for PathItem {
	fn default() -> Self {
		Self::new()
	}
}

/// A single API operation (e.g., GET /users).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
	/// Short summary of the operation.
	pub summary: Option<String>,
	/// Detailed description of the operation.
	pub description: Option<String>,
	/// Parameters accepted by the operation.
	pub parameters: Vec<Parameter>,
	/// Possible responses keyed by HTTP status code.
	pub responses: HashMap<String, Response>,
}

impl Operation {
	/// Create a new empty `Operation`.
	pub fn new() -> Self {
		Self {
			summary: None,
			description: None,
			parameters: Vec::new(),
			responses: HashMap::new(),
		}
	}
}

impl Default for Operation {
	fn default() -> Self {
		Self::new()
	}
}

/// An API operation parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
	/// Name of the parameter.
	pub name: String,
	/// Where the parameter is located (query, header, path, or cookie).
	pub location: ParameterLocation,
	/// Whether the parameter is required.
	pub required: bool,
	/// Schema describing the parameter type.
	pub schema: Schema,
}

impl Parameter {
	/// Create a new `Parameter` with the given name and location.
	pub fn new(name: String, location: ParameterLocation) -> Self {
		Self {
			name,
			location,
			required: false,
			schema: Schema::new("string".to_string()),
		}
	}
}

/// Location of an API parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
	/// Query string parameter.
	Query,
	/// HTTP header parameter.
	Header,
	/// URL path parameter.
	Path,
	/// Cookie parameter.
	Cookie,
}

/// An API response definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
	/// Description of the response.
	pub description: String,
	/// Response content keyed by media type (e.g., `"application/json"`).
	pub content: Option<HashMap<String, MediaType>>,
}

impl Response {
	/// Create a new `Response` with the given description.
	pub fn new(description: String) -> Self {
		Self {
			description,
			content: None,
		}
	}
}

/// Media type definition with its associated schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
	/// Schema describing the media type content.
	pub schema: Schema,
}

impl MediaType {
	/// Create a new `MediaType` with the given schema.
	pub fn new(schema: Schema) -> Self {
		Self { schema }
	}
}

/// JSON Schema definition for API types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
	/// The data type (e.g., `"string"`, `"object"`, `"integer"`).
	#[serde(rename = "type")]
	pub schema_type: String,
	/// Nested property schemas for object types.
	pub properties: Option<HashMap<String, Schema>>,
}

impl Schema {
	/// Create a new `Schema` with the given type.
	pub fn new(schema_type: String) -> Self {
		Self {
			schema_type,
			properties: None,
		}
	}
}

/// Reusable OpenAPI components container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
	/// Named schema definitions.
	pub schemas: HashMap<String, Schema>,
}

impl Components {
	/// Create a new empty `Components`.
	pub fn new() -> Self {
		Self {
			schemas: HashMap::new(),
		}
	}
}

impl Default for Components {
	fn default() -> Self {
		Self::new()
	}
}

/// Generator for producing JSON Schema objects.
pub struct SchemaGenerator;

impl SchemaGenerator {
	/// Create a new `SchemaGenerator`.
	pub fn new() -> Self {
		Self
	}

	/// Generate a default object schema.
	pub fn generate(&self) -> Schema {
		Schema::new("object".to_string())
	}
}

impl Default for SchemaGenerator {
	fn default() -> Self {
		Self::new()
	}
}

/// Information about a single API endpoint.
#[derive(Debug, Clone)]
pub struct EndpointInfo {
	/// URL path of the endpoint.
	pub path: String,
	/// HTTP method (e.g., `"GET"`, `"POST"`).
	pub method: String,
	/// Operation metadata for this endpoint.
	pub operation: Operation,
}

impl EndpointInfo {
	/// Create a new `EndpointInfo` with the given path and method.
	pub fn new(path: String, method: String) -> Self {
		Self {
			path,
			method,
			operation: Operation::new(),
		}
	}
}
