//! Endpoint Inspector for Function-Based Routes
//!
//! Extracts endpoint metadata from HTTP method decorator macros
//! (`#[get]`, `#[post]`, etc.) using the inventory crate.

use super::SchemaError;
use indexmap::IndexMap;
use regex::Regex;
use reinhardt_core::endpoint::EndpointMetadata;
use utoipa::openapi::{
	HttpMethod, PathItem, ResponseBuilder,
	content::ContentBuilder,
	path::{
		Operation, OperationBuilder, Parameter, ParameterBuilder, ParameterIn, PathItemBuilder,
	},
	request_body::{RequestBody, RequestBodyBuilder},
	schema::{ObjectBuilder, Schema, SchemaFormat, Type},
};

/// Configuration for endpoint inspection
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InspectorConfig {
	/// Whether to include function names in summaries
	pub include_function_names: bool,
	/// Default tag when module path inference fails
	pub default_tag: String,
}

impl Default for InspectorConfig {
	fn default() -> Self {
		Self {
			include_function_names: true,
			default_tag: "Default".to_string(),
		}
	}
}

/// Endpoint inspector for function-based routes
///
/// Extracts endpoint information from HTTP method decorator macros
/// using the inventory crate to collect metadata at compile time.
pub struct EndpointInspector {
	config: InspectorConfig,
}

impl EndpointInspector {
	/// Create a new endpoint inspector with default configuration
	pub fn new() -> Self {
		Self {
			config: InspectorConfig::default(),
		}
	}

	/// Create a new endpoint inspector with custom configuration
	pub fn with_config(config: InspectorConfig) -> Self {
		Self { config }
	}

	/// Extract all registered endpoints as OpenAPI paths
	///
	/// Collects endpoint metadata from the global inventory and
	/// generates OpenAPI path items for each endpoint.
	pub fn extract_paths(&self) -> Result<IndexMap<String, PathItem>, SchemaError> {
		// Step 1: Group endpoints by normalized path
		let mut path_groups: IndexMap<String, Vec<&EndpointMetadata>> = IndexMap::new();

		for metadata in inventory::iter::<EndpointMetadata>() {
			let normalized_path = self.normalize_path(metadata.path);
			path_groups
				.entry(normalized_path)
				.or_default()
				.push(metadata);
		}

		// Step 2: Build PathItem for each path with all its operations
		let mut paths = IndexMap::new();

		for (path, endpoints) in path_groups {
			let mut builder = PathItemBuilder::new();

			for metadata in endpoints {
				let parameters = self.extract_path_parameters(metadata.path)?;
				let operation = self.create_operation(metadata, parameters);
				let http_method = self.metadata_method_to_http_method(metadata.method)?;

				builder = builder.operation(http_method, operation);
			}

			paths.insert(path, builder.build());
		}

		Ok(paths)
	}

	/// Normalize Django-style path to OpenAPI format
	///
	/// Converts patterns like:
	/// - `{<uuid:user_id>}` → `{user_id}`
	/// - `{<int:id>}` → `{id}`
	/// - `{<str:slug>}` → `{slug}`
	fn normalize_path(&self, path: &str) -> String {
		// Regex to match Django-style path parameters: {<type:name>}
		let re = Regex::new(r"\{<[^:]+:([^>]+)>\}").unwrap();
		re.replace_all(path, "{$1}").to_string()
	}

	/// Convert metadata method string to utoipa HttpMethod enum
	fn metadata_method_to_http_method(&self, method: &str) -> Result<HttpMethod, SchemaError> {
		match method {
			"GET" => Ok(HttpMethod::Get),
			"POST" => Ok(HttpMethod::Post),
			"PUT" => Ok(HttpMethod::Put),
			"PATCH" => Ok(HttpMethod::Patch),
			"DELETE" => Ok(HttpMethod::Delete),
			_ => Err(SchemaError::InspectorError(format!(
				"Unsupported HTTP method: {}",
				method
			))),
		}
	}

	/// Create a RequestBody from endpoint metadata
	///
	/// Only POST/PUT/PATCH methods have request bodies.
	/// Attempts to retrieve schema from global registry; falls back to placeholder if not found.
	fn create_request_body(&self, metadata: &EndpointMetadata) -> Option<RequestBody> {
		// Only POST/PUT/PATCH methods have request bodies
		if !matches!(metadata.method, "POST" | "PUT" | "PATCH") {
			return None;
		}

		let body_type = metadata.request_body_type?;
		let content_type = metadata.request_content_type?;

		// Try to get schema from global registry first
		let schema =
			if let Some(registered_schema) = super::registry::get_all_schemas().get(body_type) {
				// Use registered schema
				registered_schema.clone()
			} else {
				// Fallback: Create placeholder schema (empty object)
				Schema::Object(
					ObjectBuilder::new()
						.description(Some(format!("Request body for {}", body_type)))
						.build(),
				)
			};

		// Create Content with schema
		let content = ContentBuilder::new().schema(Some(schema)).build();

		// Create RequestBody
		Some(
			RequestBodyBuilder::new()
				.description(Some(format!("Request body containing {}", body_type)))
				.required(Some(utoipa::openapi::Required::True))
				.content(content_type, content)
				.build(),
		)
	}

	/// Create an Operation object for an endpoint
	fn create_operation(
		&self,
		metadata: &EndpointMetadata,
		parameters: Vec<Parameter>,
	) -> Operation {
		let operation_id = metadata.name.unwrap_or(metadata.function_name).to_string();
		let summary = self.generate_summary(metadata);
		let tags = self.infer_tags(metadata.module_path);

		let mut builder = OperationBuilder::new()
			.operation_id(Some(operation_id))
			.summary(Some(summary))
			.tags(Some(tags));

		// Add parameters
		for param in parameters {
			builder = builder.parameter(param);
		}

		// Add request body (if applicable)
		if let Some(request_body) = self.create_request_body(metadata) {
			builder = builder.request_body(Some(request_body));
		}

		// Add default response
		builder = builder.response(
			"200",
			ResponseBuilder::new()
				.description("Successful response")
				.build(),
		);

		builder.build()
	}

	/// Extract path parameters from a path pattern
	///
	/// Parses Django-style path parameters and converts them to OpenAPI parameters.
	///
	/// Supported patterns:
	/// - `{<uuid:user_id>}` → name="user_id", type="string", format="uuid"
	/// - `{<int:id>}` → name="id", type="integer"
	/// - `{<str:slug>}` → name="slug", type="string"
	fn extract_path_parameters(&self, path: &str) -> Result<Vec<Parameter>, SchemaError> {
		let mut parameters = Vec::new();

		// Regex to match Django-style path parameters: {<type:name>}
		let re = Regex::new(r"\{<([^:]+):([^>]+)>\}").unwrap();

		for caps in re.captures_iter(path) {
			let type_str = caps.get(1).unwrap().as_str();
			let name = caps.get(2).unwrap().as_str();

			let (schema_type, format) = self.django_type_to_openapi(type_str);

			// Build schema based on type
			let mut schema_builder = ObjectBuilder::new().schema_type(schema_type);
			if let Some(fmt) = format {
				schema_builder = schema_builder.format(Some(fmt));
			}
			let schema = Schema::Object(schema_builder.build());

			let parameter = ParameterBuilder::new()
				.name(name)
				.parameter_in(ParameterIn::Path)
				.required(utoipa::openapi::Required::True)
				.schema(Some(schema))
				.build();

			parameters.push(parameter);
		}

		Ok(parameters)
	}

	/// Convert Django type specifier to OpenAPI type and format
	///
	/// Mappings:
	/// - `uuid` → (string, uuid)
	/// - `int` → (integer, None)
	/// - `str` → (string, None)
	/// - `slug` → (string, None)
	/// - `path` → (string, None)
	fn django_type_to_openapi(&self, django_type: &str) -> (Type, Option<SchemaFormat>) {
		match django_type {
			"uuid" => (Type::String, Some(SchemaFormat::Custom("uuid".to_string()))),
			"int" => (Type::Integer, None),
			"str" | "slug" | "path" => (Type::String, None),
			_ => (Type::String, None), // Default to string
		}
	}

	/// Generate a summary from endpoint metadata
	fn generate_summary(&self, metadata: &EndpointMetadata) -> String {
		if let Some(name) = metadata.name {
			if self.config.include_function_names {
				format!("{} ({})", name, metadata.function_name)
			} else {
				name.to_string()
			}
		} else if self.config.include_function_names {
			metadata.function_name.to_string()
		} else {
			format!("{} endpoint", metadata.method)
		}
	}

	/// Infer tags from module path
	///
	/// Examples:
	/// - `examples_twitter::apps::auth::views::register` → ["Auth"]
	/// - `examples_twitter::apps::profile::views::fetch_profile` → ["Profile"]
	/// - `examples_twitter::apps::dm::views::messages` → ["Dm"]
	fn infer_tags(&self, module_path: &str) -> Vec<String> {
		// Split module path by ::
		let parts: Vec<&str> = module_path.split("::").collect();

		// Look for pattern: *::apps::<app_name>::*
		for (i, part) in parts.iter().enumerate() {
			if *part == "apps" && i + 1 < parts.len() {
				let app_name = parts[i + 1];
				// Capitalize first letter
				let tag = app_name
					.chars()
					.enumerate()
					.map(|(idx, c)| {
						if idx == 0 {
							c.to_uppercase().collect::<String>()
						} else {
							c.to_string()
						}
					})
					.collect::<String>();
				return vec![tag];
			}
		}

		// Fallback to default tag
		vec![self.config.default_tag.clone()]
	}
}

impl Default for EndpointInspector {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_normalize_path() {
		let inspector = EndpointInspector::new();

		assert_eq!(
			inspector.normalize_path("/users/{<uuid:user_id>}/"),
			"/users/{user_id}/"
		);
		assert_eq!(
			inspector.normalize_path("/posts/{<int:id>}/"),
			"/posts/{id}/"
		);
		assert_eq!(
			inspector.normalize_path("/articles/{<str:slug>}/"),
			"/articles/{slug}/"
		);
	}

	#[test]
	fn test_django_type_to_openapi() {
		let inspector = EndpointInspector::new();

		let (uuid_type, uuid_format) = inspector.django_type_to_openapi("uuid");
		assert!(matches!(uuid_type, Type::String));
		assert!(matches!(uuid_format, Some(SchemaFormat::Custom(_))));

		let (int_type, int_format) = inspector.django_type_to_openapi("int");
		assert!(matches!(int_type, Type::Integer));
		assert!(int_format.is_none());

		let (str_type, str_format) = inspector.django_type_to_openapi("str");
		assert!(matches!(str_type, Type::String));
		assert!(str_format.is_none());
	}

	#[test]
	fn test_infer_tags() {
		let inspector = EndpointInspector::new();

		assert_eq!(
			inspector.infer_tags("examples_twitter::apps::auth::views::register"),
			vec!["Auth".to_string()]
		);
		assert_eq!(
			inspector.infer_tags("examples_twitter::apps::profile::views::fetch_profile"),
			vec!["Profile".to_string()]
		);
		assert_eq!(
			inspector.infer_tags("my_project::some_module::function"),
			vec!["Default".to_string()]
		);
	}

	#[test]
	fn test_create_request_body_for_post() {
		let inspector = EndpointInspector::new();

		let metadata = EndpointMetadata {
			path: "/api/users",
			method: "POST",
			name: Some("create_user"),
			function_name: "create_user",
			module_path: "users::views",
			request_body_type: Some("CreateUserRequest"),
			request_content_type: Some("application/json"),
		};

		let request_body = inspector.create_request_body(&metadata);
		assert!(request_body.is_some());

		let rb = request_body.unwrap();
		assert!(rb.required.is_some());
		assert!(matches!(
			rb.required.unwrap(),
			utoipa::openapi::Required::True
		));
		assert!(rb.content.contains_key("application/json"));
	}

	#[test]
	fn test_create_request_body_for_get() {
		let inspector = EndpointInspector::new();

		let metadata = EndpointMetadata {
			path: "/api/users",
			method: "GET",
			name: Some("list_users"),
			function_name: "list_users",
			module_path: "users::views",
			request_body_type: None,
			request_content_type: None,
		};

		let request_body = inspector.create_request_body(&metadata);
		assert!(request_body.is_none());
	}

	#[test]
	fn test_create_request_body_for_form() {
		let inspector = EndpointInspector::new();

		let metadata = EndpointMetadata {
			path: "/api/login",
			method: "POST",
			name: Some("login"),
			function_name: "login",
			module_path: "auth::views",
			request_body_type: Some("LoginForm"),
			request_content_type: Some("application/x-www-form-urlencoded"),
		};

		let request_body = inspector.create_request_body(&metadata);
		assert!(request_body.is_some());

		let rb = request_body.unwrap();
		assert!(rb.content.contains_key("application/x-www-form-urlencoded"));
	}

	#[test]
	fn test_metadata_method_to_http_method() {
		let inspector = EndpointInspector::new();

		// Test valid methods
		assert!(matches!(
			inspector.metadata_method_to_http_method("GET"),
			Ok(HttpMethod::Get)
		));
		assert!(matches!(
			inspector.metadata_method_to_http_method("POST"),
			Ok(HttpMethod::Post)
		));
		assert!(matches!(
			inspector.metadata_method_to_http_method("PUT"),
			Ok(HttpMethod::Put)
		));
		assert!(matches!(
			inspector.metadata_method_to_http_method("PATCH"),
			Ok(HttpMethod::Patch)
		));
		assert!(matches!(
			inspector.metadata_method_to_http_method("DELETE"),
			Ok(HttpMethod::Delete)
		));

		// Test invalid method
		assert!(inspector.metadata_method_to_http_method("INVALID").is_err());
	}
}
