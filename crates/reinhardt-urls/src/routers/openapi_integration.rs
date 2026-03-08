//! OpenAPI integration for automatic schema generation
//!
//! This module provides utilities for generating OpenAPI schemas from registered routes.
//! It integrates with the `reinhardt-openapi` crate to provide automatic documentation.
//!
//! # Examples
//!
//! ```
//! use reinhardt_urls::routers::openapi_integration::{OpenApiBuilder, PathItem};
//! use hyper::Method;
//!
//! let mut builder = OpenApiBuilder::new("My API", "1.0.0");
//!
//! // Add routes
//! builder.add_path(
//!     "/api/users/",
//!     PathItem::new()
//!         .with_method(Method::GET, "List all users", Some("api:users:list"))
//! );
//!
//! // Generate OpenAPI spec
//! let spec = builder.build();
//! assert_eq!(spec.info.title, "My API");
//! ```

use crate::routers::introspection::{RouteInfo, RouteInspector};
use hyper::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenAPI specification version
const OPENAPI_VERSION: &str = "3.0.3";

/// OpenAPI specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
	/// OpenAPI version
	pub openapi: String,

	/// API information
	pub info: InfoObject,

	/// Server information
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub servers: Vec<ServerObject>,

	/// API paths
	pub paths: HashMap<String, PathItemObject>,

	/// Reusable components
	#[serde(skip_serializing_if = "Option::is_none")]
	pub components: Option<ComponentsObject>,

	/// Security requirements
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub security: Vec<HashMap<String, Vec<String>>>,

	/// Tags for grouping operations
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub tags: Vec<TagObject>,
}

/// API information object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoObject {
	/// API title
	pub title: String,

	/// API version
	pub version: String,

	/// API description
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	/// Terms of service URL
	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(rename = "termsOfService")]
	pub terms_of_service: Option<String>,

	/// Contact information
	#[serde(skip_serializing_if = "Option::is_none")]
	pub contact: Option<ContactObject>,

	/// License information
	#[serde(skip_serializing_if = "Option::is_none")]
	pub license: Option<LicenseObject>,
}

/// Contact information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactObject {
	/// Contact name.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,

	/// Contact URL.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,

	/// Contact email address.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<String>,
}

/// License information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseObject {
	/// License name (e.g., `"MIT"`, `"Apache-2.0"`).
	pub name: String,

	/// URL to the full license text.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
}

/// Server object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerObject {
	/// Base URL of the server.
	pub url: String,

	/// Human-readable description of the server.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
}

/// Path item object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItemObject {
	/// GET operation for this path.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub get: Option<OperationObject>,

	/// POST operation for this path.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub post: Option<OperationObject>,

	/// PUT operation for this path.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub put: Option<OperationObject>,

	/// PATCH operation for this path.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub patch: Option<OperationObject>,

	/// DELETE operation for this path.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub delete: Option<OperationObject>,

	/// Path-level parameters shared by all operations.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub parameters: Vec<ParameterObject>,
}

/// Operation object (HTTP method on a path)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationObject {
	/// Short summary of the operation.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub summary: Option<String>,

	/// Detailed description of the operation.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	/// Unique identifier for the operation.
	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(rename = "operationId")]
	pub operation_id: Option<String>,

	/// Tags for grouping this operation.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub tags: Vec<String>,

	/// Operation-specific parameters.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub parameters: Vec<ParameterObject>,

	/// Map of HTTP status codes to response descriptions.
	pub responses: HashMap<String, ResponseObject>,
}

/// Parameter object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterObject {
	/// Parameter name.
	pub name: String,

	/// Location of the parameter (`"path"`, `"query"`, `"header"`, or `"cookie"`).
	#[serde(rename = "in")]
	pub location: String,

	/// Human-readable description of the parameter.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	/// Whether the parameter is required.
	pub required: bool,

	/// Schema describing the parameter type.
	pub schema: SchemaObject,
}

/// Response object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseObject {
	/// Human-readable description of the response.
	pub description: String,

	/// Map of media types to their schemas.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub content: Option<HashMap<String, MediaTypeObject>>,
}

/// Media type object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTypeObject {
	/// Schema describing the media type content.
	pub schema: SchemaObject,
}

/// Schema object (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaObject {
	/// The JSON Schema type (e.g., `"string"`, `"integer"`, `"object"`).
	#[serde(rename = "type")]
	pub schema_type: String,

	/// Additional format hint (e.g., `"int64"`, `"date-time"`).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub format: Option<String>,
}

/// Components object (reusable schemas)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentsObject {
	/// Map of schema names to their definitions.
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub schemas: HashMap<String, SchemaObject>,
}

/// Tag object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagObject {
	/// Tag name used for grouping operations.
	pub name: String,

	/// Human-readable description of the tag.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
}

/// Builder for creating PathItem objects
#[derive(Debug, Clone, Default)]
pub struct PathItem {
	operations: HashMap<Method, OperationObject>,
	parameters: Vec<ParameterObject>,
}

impl PathItem {
	/// Create a new PathItem builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::PathItem;
	///
	/// let path_item = PathItem::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add an operation for an HTTP method
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::PathItem;
	/// use hyper::Method;
	///
	/// let path_item = PathItem::new()
	///     .with_method(Method::GET, "Get user", Some("users:detail"));
	/// ```
	pub fn with_method(
		mut self,
		method: Method,
		summary: impl Into<String>,
		operation_id: Option<impl Into<String>>,
	) -> Self {
		let operation = OperationObject {
			summary: Some(summary.into()),
			description: None,
			operation_id: operation_id.map(|s| s.into()),
			tags: Vec::new(),
			parameters: Vec::new(),
			responses: HashMap::new(),
		};

		self.operations.insert(method, operation);
		self
	}

	/// Add a path parameter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::PathItem;
	///
	/// let path_item = PathItem::new()
	///     .with_parameter("id", "User ID", "string");
	/// ```
	pub fn with_parameter(
		mut self,
		name: impl Into<String>,
		description: impl Into<String>,
		schema_type: impl Into<String>,
	) -> Self {
		let param = ParameterObject {
			name: name.into(),
			location: "path".to_string(),
			description: Some(description.into()),
			required: true,
			schema: SchemaObject {
				schema_type: schema_type.into(),
				format: None,
			},
		};

		self.parameters.push(param);
		self
	}

	/// Build the PathItemObject
	fn build(self) -> PathItemObject {
		let mut path_item = PathItemObject {
			get: None,
			post: None,
			put: None,
			patch: None,
			delete: None,
			parameters: self.parameters,
		};

		for (method, operation) in self.operations {
			match method {
				Method::GET => path_item.get = Some(operation),
				Method::POST => path_item.post = Some(operation),
				Method::PUT => path_item.put = Some(operation),
				Method::PATCH => path_item.patch = Some(operation),
				Method::DELETE => path_item.delete = Some(operation),
				_ => {}
			}
		}

		path_item
	}
}

/// OpenAPI specification builder
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::openapi_integration::{OpenApiBuilder, PathItem};
/// use hyper::Method;
///
/// let mut builder = OpenApiBuilder::new("My API", "1.0.0");
/// builder.description("A sample API");
/// builder.add_server("https://api.example.com", None);
///
/// builder.add_path(
///     "/users/",
///     PathItem::new().with_method(Method::GET, "List users", Some("users:list"))
/// );
///
/// let spec = builder.build();
/// assert_eq!(spec.info.title, "My API");
/// ```
pub struct OpenApiBuilder {
	info: InfoObject,
	servers: Vec<ServerObject>,
	paths: HashMap<String, PathItemObject>,
	tags: Vec<TagObject>,
}

impl OpenApiBuilder {
	/// Create a new OpenAPI builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::OpenApiBuilder;
	///
	/// let builder = OpenApiBuilder::new("My API", "1.0.0");
	/// ```
	pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
		Self {
			info: InfoObject {
				title: title.into(),
				version: version.into(),
				description: None,
				terms_of_service: None,
				contact: None,
				license: None,
			},
			servers: Vec::new(),
			paths: HashMap::new(),
			tags: Vec::new(),
		}
	}

	/// Set API description
	pub fn description(&mut self, description: impl Into<String>) -> &mut Self {
		self.info.description = Some(description.into());
		self
	}

	/// Add a server
	pub fn add_server(&mut self, url: impl Into<String>, description: Option<String>) -> &mut Self {
		self.servers.push(ServerObject {
			url: url.into(),
			description,
		});
		self
	}

	/// Add a tag
	pub fn add_tag(&mut self, name: impl Into<String>, description: Option<String>) -> &mut Self {
		self.tags.push(TagObject {
			name: name.into(),
			description,
		});
		self
	}

	/// Add a path
	pub fn add_path(&mut self, path: impl Into<String>, path_item: PathItem) -> &mut Self {
		self.paths.insert(path.into(), path_item.build());
		self
	}

	/// Build the OpenAPI specification
	pub fn build(self) -> OpenApiSpec {
		OpenApiSpec {
			openapi: OPENAPI_VERSION.to_string(),
			info: self.info,
			servers: self.servers,
			paths: self.paths,
			components: None,
			security: Vec::new(),
			tags: self.tags,
		}
	}

	/// Build from a RouteInspector
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::introspection::RouteInspector;
	/// use reinhardt_urls::routers::openapi_integration::OpenApiBuilder;
	/// use hyper::Method;
	///
	/// let mut inspector = RouteInspector::new();
	/// inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
	///
	/// let spec = OpenApiBuilder::from_inspector(
	///     "My API",
	///     "1.0.0",
	///     &inspector
	/// ).build();
	///
	/// assert_eq!(spec.paths.len(), 1);
	/// ```
	pub fn from_inspector(
		title: impl Into<String>,
		version: impl Into<String>,
		inspector: &RouteInspector,
	) -> Self {
		let mut builder = Self::new(title, version);

		// Extract tags from namespaces
		for namespace in inspector.all_namespaces() {
			builder.add_tag(&namespace, None);
		}

		// Add paths
		for route in inspector.all_routes() {
			builder.add_route_info(route);
		}

		builder
	}

	/// Add a route from RouteInfo
	fn add_route_info(&mut self, route: &RouteInfo) {
		let mut path_item = PathItem::new();

		// Add parameters from path
		for param in &route.params {
			path_item = path_item.with_parameter(param, format!("{} parameter", param), "string");
		}

		// Add operations for each method
		for method_str in &route.methods {
			// Parse string back to Method
			if let Ok(method) = method_str.parse::<Method>() {
				let operation_id = route.name.clone();
				let summary = route
					.metadata
					.get("summary")
					.cloned()
					.unwrap_or_else(|| format!("{} {}", method.as_str(), route.path));

				path_item = path_item.with_method(method, summary, operation_id);
			}
		}

		self.paths.insert(route.path.clone(), path_item.build());
	}
}

impl OpenApiSpec {
	/// Export as JSON
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::OpenApiBuilder;
	///
	/// let spec = OpenApiBuilder::new("My API", "1.0.0").build();
	/// let json = spec.to_json().unwrap();
	/// assert!(json.contains("My API"));
	/// ```
	pub fn to_json(&self) -> Result<String, serde_json::Error> {
		serde_json::to_string_pretty(self)
	}

	/// Export as YAML
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::openapi_integration::OpenApiBuilder;
	///
	/// let spec = OpenApiBuilder::new("My API", "1.0.0").build();
	/// let yaml = spec.to_yaml().unwrap();
	/// assert!(yaml.contains("My API"));
	/// ```
	pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
		serde_yaml::to_string(self)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_openapi_builder_basic() {
		let mut builder = OpenApiBuilder::new("Test API", "1.0.0");
		builder.description("A test API");

		let spec = builder.build();
		assert_eq!(spec.info.title, "Test API");
		assert_eq!(spec.info.version, "1.0.0");
		assert_eq!(spec.info.description, Some("A test API".to_string()));
	}

	#[test]
	fn test_openapi_builder_with_server() {
		let mut builder = OpenApiBuilder::new("Test API", "1.0.0");
		builder.add_server(
			"https://api.example.com",
			Some("Production server".to_string()),
		);

		let spec = builder.build();
		assert_eq!(spec.servers.len(), 1);
		assert_eq!(spec.servers[0].url, "https://api.example.com");
	}

	#[test]
	fn test_path_item_builder() {
		let path_item = PathItem::new()
			.with_method(Method::GET, "Get user", Some("users:detail"))
			.with_parameter("id", "User ID", "string");

		let path_item_obj = path_item.build();
		assert!(path_item_obj.get.is_some());
		assert_eq!(path_item_obj.parameters.len(), 1);
	}

	#[test]
	fn test_openapi_builder_from_inspector() {
		let mut inspector = RouteInspector::new();
		inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
		inspector.add_route(
			"/users/{id}/",
			vec![Method::GET],
			Some("users:detail"),
			None,
		);

		let spec = OpenApiBuilder::from_inspector("Test API", "1.0.0", &inspector).build();

		assert_eq!(spec.paths.len(), 2);
		assert!(spec.paths.contains_key("/users/"));
		assert!(spec.paths.contains_key("/users/{id}/"));
	}

	#[test]
	fn test_openapi_spec_to_json() {
		let spec = OpenApiBuilder::new("Test API", "1.0.0").build();
		let json = spec.to_json().unwrap();
		assert!(json.contains("Test API"));
		assert!(json.contains("3.0.3"));
	}
}
