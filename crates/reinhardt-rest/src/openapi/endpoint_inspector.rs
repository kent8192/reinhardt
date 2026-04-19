//! Endpoint Inspector for Function-Based Routes
//!
//! Extracts endpoint metadata from HTTP method decorator macros
//! (`#[get]`, `#[post]`, etc.) using the inventory crate.

use super::SchemaError;
use indexmap::IndexMap;
use regex::Regex;
use reinhardt_core::endpoint::{AuthProtection, EndpointMetadata};
use utoipa::openapi::{
	HttpMethod, PathItem, ResponseBuilder,
	content::ContentBuilder,
	extensions::Extensions,
	header::HeaderBuilder,
	path::{
		Operation, OperationBuilder, Parameter, ParameterBuilder, ParameterIn, PathItemBuilder,
	},
	request_body::{RequestBody, RequestBodyBuilder},
	schema::{ObjectBuilder, Schema, SchemaFormat, Type},
	security::SecurityRequirement,
};

/// Configuration for endpoint inspection
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InspectorConfig {
	/// Whether to include function names in summaries
	pub include_function_names: bool,
	/// Default tag when module path inference fails
	pub default_tag: String,
	/// Names of all configured security schemes (used when generating security requirements
	/// for endpoints with `AuthProtection::Protected` or `AuthProtection::Optional`).
	pub security_scheme_names: Vec<String>,
}

impl Default for InspectorConfig {
	fn default() -> Self {
		Self {
			include_function_names: true,
			default_tag: "Default".to_string(),
			security_scheme_names: Vec::new(),
		}
	}
}

/// Normalize a type name by extracting the last path segment.
///
/// Handles fully-qualified paths like `"crate :: models :: CreateUserRequest"`
/// (produced by `quote!().to_string()`) by returning just `"CreateUserRequest"`.
/// Simple names without `::` are returned as-is but still trimmed of whitespace.
fn normalize_type_name(type_str: &str) -> &str {
	match type_str.rsplit_once("::") {
		Some((_, last)) => last.trim(),
		None => type_str.trim(),
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

		// Normalize the type name to handle fully-qualified paths from quote!()
		// e.g., "crate :: models :: CreateUserRequest" → "CreateUserRequest"
		let normalized_type = normalize_type_name(body_type);

		// Try to get schema from global registry first
		let schema = if let Some(registered_schema) =
			super::registry::get_all_schemas().get(normalized_type)
		{
			// Use registered schema
			registered_schema.clone()
		} else {
			// Fallback: Create placeholder schema (empty object)
			Schema::Object(
				ObjectBuilder::new()
					.description(Some(format!("Request body for {}", normalized_type)))
					.build(),
			)
		};

		// Create Content with schema
		let content = ContentBuilder::new().schema(Some(schema)).build();

		// Create RequestBody
		Some(
			RequestBodyBuilder::new()
				.description(Some(format!("Request body containing {}", normalized_type)))
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

		// Add default response (with headers if present)
		let mut default_resp_builder = ResponseBuilder::new().description("Successful response");
		for header in metadata.headers {
			default_resp_builder = default_resp_builder.header(
				header.name,
				HeaderBuilder::new()
					.description(Some(header.description.to_string()))
					.build(),
			);
		}
		builder = builder.response("200", default_resp_builder.build());

		// Add custom responses from metadata
		for response in metadata.responses {
			let mut resp_builder =
				ResponseBuilder::new().description(response.description.to_string());

			// Add any response headers from metadata to custom responses
			for header in metadata.headers {
				resp_builder = resp_builder.header(
					header.name,
					HeaderBuilder::new()
						.description(Some(header.description.to_string()))
						.build(),
				);
			}

			builder = builder.response(response.status.to_string(), resp_builder.build());
		}

		// Add security requirements based on auth protection level.
		// - Public: explicit empty security list (no auth required)
		// - Protected: all configured schemes required
		// - Optional: all configured schemes + anonymous option ({})
		// - None: no security field added (startup validation catches this case)
		match metadata.auth_protection {
			AuthProtection::Public => {
				// Explicitly mark as no security (empty security array entry)
				builder = builder.security(SecurityRequirement::default());
			}
			AuthProtection::Protected => {
				for name in &self.config.security_scheme_names {
					let requirement = SecurityRequirement::new::<&str, [&str; 0], &str>(name, []);
					builder = builder.security(requirement);
				}
				// Fall back to legacy metadata.security when no schemes are configured
				if self.config.security_scheme_names.is_empty() {
					for security_name in metadata.security {
						let requirement =
							SecurityRequirement::new::<&str, [&str; 0], &str>(security_name, []);
						builder = builder.security(requirement);
					}
				}
			}
			AuthProtection::Optional => {
				for name in &self.config.security_scheme_names {
					let requirement = SecurityRequirement::new::<&str, [&str; 0], &str>(name, []);
					builder = builder.security(requirement);
				}
				// Fall back to legacy metadata.security when no schemes are configured
				if self.config.security_scheme_names.is_empty() {
					for security_name in metadata.security {
						let requirement =
							SecurityRequirement::new::<&str, [&str; 0], &str>(security_name, []);
						builder = builder.security(requirement);
					}
				}
				// Add anonymous option (empty requirement = unauthenticated allowed)
				builder = builder.security(SecurityRequirement::default());
			}
			AuthProtection::None => {
				// No security field: startup validation catches this case
			}
		}

		// Add x-guard extension when a guard description is available
		if let Some(desc) = &metadata.guard_description {
			let mut exts = Extensions::default();
			exts.insert("x-guard".to_string(), serde_json::json!(desc));
			builder = builder.extensions(Some(exts));
		}

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
	use crate::openapi::schema_registration::SchemaRegistration;
	use reinhardt_core::endpoint::AuthProtection;
	use utoipa::openapi::schema::ObjectBuilder;

	// Register a test schema for qualified-path lookup verification.
	// This will be present in the global registry under "QualifiedPathTestSchema".
	inventory::submit! {
		SchemaRegistration {
			name: "QualifiedPathTestSchema",
			generator: || {
				Schema::Object(
					ObjectBuilder::new()
						.schema_type(utoipa::openapi::schema::Type::Object)
						.title(Some("QualifiedPathTestSchema"))
						.description(Some("Test schema for qualified path lookup"))
						.property(
							"test_field",
							Schema::Object(
								ObjectBuilder::new()
									.schema_type(utoipa::openapi::schema::Type::String)
									.build(),
							),
						)
						.required("test_field")
						.build(),
				)
			},
		}
	}

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
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::None,
			guard_description: None,
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
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::None,
			guard_description: None,
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
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::None,
			guard_description: None,
		};

		let request_body = inspector.create_request_body(&metadata);
		assert!(request_body.is_some());

		let rb = request_body.unwrap();
		assert!(rb.content.contains_key("application/x-www-form-urlencoded"));
	}

	#[test]
	fn test_create_request_body_fallback_when_type_not_in_registry() {
		// Arrange
		let inspector = EndpointInspector::new();

		// Use a type name that is guaranteed to not exist in the global schema registry
		let metadata = EndpointMetadata {
			path: "/api/nonexistent",
			method: "POST",
			name: Some("create_nonexistent"),
			function_name: "create_nonexistent",
			module_path: "nonexistent::views",
			request_body_type: Some("NonExistentType"),
			request_content_type: Some("application/json"),
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::None,
			guard_description: None,
		};

		// Act
		let request_body = inspector.create_request_body(&metadata);

		// Assert: fallback schema is returned (not None)
		assert!(
			request_body.is_some(),
			"Should return a request body even for unregistered types"
		);

		let rb = request_body.unwrap();
		assert!(
			rb.content.contains_key("application/json"),
			"Fallback request body should have application/json content"
		);

		// Verify fallback schema has a description mentioning the type name
		let content = rb.content.get("application/json").unwrap();
		assert!(
			content.schema.is_some(),
			"Fallback content should have a schema"
		);

		if let Some(utoipa::openapi::RefOr::T(schema)) = &content.schema {
			if let utoipa::openapi::schema::Schema::Object(obj) = schema {
				let description = obj.description.as_deref().unwrap_or("");
				assert!(
					description.contains("NonExistentType"),
					"Fallback schema description should mention the type name, got: {}",
					description
				);
			} else {
				panic!("Fallback schema should be an Object schema");
			}
		} else {
			panic!("Fallback schema should be a concrete schema (not a $ref)");
		}
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

	#[test]
	fn test_normalize_type_name_simple() {
		// Arrange / Act / Assert
		assert_eq!(
			normalize_type_name("CreateUserRequest"),
			"CreateUserRequest"
		);
	}

	#[test]
	fn test_normalize_type_name_fully_qualified() {
		// Arrange: fully-qualified path as produced by quote!().to_string()
		// Act / Assert
		assert_eq!(
			normalize_type_name("crate :: models :: CreateUserRequest"),
			"CreateUserRequest"
		);
	}

	#[test]
	fn test_normalize_type_name_compact_path() {
		// Arrange: compact path without spaces around ::
		// Act / Assert
		assert_eq!(
			normalize_type_name("crate::models::CreateUserRequest"),
			"CreateUserRequest"
		);
	}

	#[test]
	fn test_normalize_type_name_single_segment_with_colons() {
		// Arrange: path with only one :: separator
		// Act / Assert
		assert_eq!(normalize_type_name("models::LoginForm"), "LoginForm");
	}

	#[test]
	fn test_normalize_type_name_empty_string() {
		// Arrange / Act / Assert
		assert_eq!(normalize_type_name(""), "");
	}

	#[test]
	fn test_create_request_body_with_qualified_path_uses_registered_schema() {
		// Arrange: verify the test schema is present in the global registry
		let all_schemas = super::super::registry::get_all_schemas();
		assert!(
			all_schemas.contains_key("QualifiedPathTestSchema"),
			"Test schema 'QualifiedPathTestSchema' should be registered in the global registry"
		);

		let inspector = EndpointInspector::new();

		// Simulate a fully-qualified type path as produced by quote!()
		let metadata = EndpointMetadata {
			path: "/api/test",
			method: "POST",
			name: Some("test_endpoint"),
			function_name: "test_endpoint",
			module_path: "test::views",
			request_body_type: Some("crate :: models :: QualifiedPathTestSchema"),
			request_content_type: Some("application/json"),
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::None,
			guard_description: None,
		};

		// Act
		let request_body = inspector.create_request_body(&metadata);

		// Assert: should return a request body using the registered schema
		assert!(
			request_body.is_some(),
			"Should return a request body even with fully-qualified type path"
		);

		let rb = request_body.unwrap();
		assert!(rb.content.contains_key("application/json"));

		// Verify the schema came from the registry (has title and properties),
		// not the fallback (which only has a description)
		let content = rb.content.get("application/json").unwrap();
		match &content.schema {
			Some(utoipa::openapi::RefOr::T(schema)) => match schema {
				Schema::Object(obj) => {
					// The registered schema has a title set
					assert_eq!(
						obj.title.as_deref(),
						Some("QualifiedPathTestSchema"),
						"Schema should come from the registry (has title), not the fallback"
					);
					// The registered schema has a 'test_field' property
					assert!(
						obj.properties.contains_key("test_field"),
						"Schema should contain 'test_field' property from the registered schema"
					);
					// The registered schema has a specific description
					assert_eq!(
						obj.description.as_deref(),
						Some("Test schema for qualified path lookup"),
						"Schema description should match the registered schema"
					);
				}
				_ => panic!("Expected Object schema from registry, got non-Object variant"),
			},
			Some(utoipa::openapi::RefOr::Ref(_)) => {
				panic!("Expected concrete schema, got a $ref")
			}
			None => panic!("Expected schema in content, got None"),
		}
	}

	#[rstest::rstest]
	#[case::public(AuthProtection::Public)]
	fn test_create_operation_public_has_empty_security(#[case] protection: AuthProtection) {
		// Arrange
		let config = InspectorConfig {
			security_scheme_names: vec!["bearer".to_string()],
			..Default::default()
		};
		let inspector = EndpointInspector::with_config(config);
		let metadata = EndpointMetadata {
			path: "/api/public",
			method: "GET",
			name: Some("public_endpoint"),
			function_name: "public_endpoint",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: protection,
			guard_description: None,
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: Public endpoints have an empty security entry (no auth required)
		let security = json["security"].as_array();
		assert!(
			security.is_some(),
			"Public endpoint should have a security field"
		);
		assert_eq!(
			security.unwrap().len(),
			1,
			"Public endpoint should have exactly one security entry"
		);
		assert!(
			security.unwrap()[0].as_object().unwrap().is_empty(),
			"Public endpoint security entry should be empty object"
		);
	}

	#[rstest::rstest]
	#[case::protected(AuthProtection::Protected)]
	fn test_create_operation_protected_lists_all_schemes(#[case] protection: AuthProtection) {
		// Arrange
		let config = InspectorConfig {
			security_scheme_names: vec!["bearer".to_string(), "cookie".to_string()],
			..Default::default()
		};
		let inspector = EndpointInspector::with_config(config);
		let metadata = EndpointMetadata {
			path: "/api/protected",
			method: "GET",
			name: Some("protected_endpoint"),
			function_name: "protected_endpoint",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: protection,
			guard_description: None,
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: Protected endpoints list all configured security schemes
		let security = json["security"]
			.as_array()
			.expect("security field should be present");
		assert_eq!(
			security.len(),
			2,
			"Protected endpoint should list both schemes"
		);
		assert!(
			security[0].as_object().unwrap().contains_key("bearer"),
			"First security entry should be bearer"
		);
		assert!(
			security[1].as_object().unwrap().contains_key("cookie"),
			"Second security entry should be cookie"
		);
	}

	#[rstest::rstest]
	#[case::optional(AuthProtection::Optional)]
	fn test_create_operation_optional_includes_anonymous(#[case] protection: AuthProtection) {
		// Arrange
		let config = InspectorConfig {
			security_scheme_names: vec!["bearer".to_string()],
			..Default::default()
		};
		let inspector = EndpointInspector::with_config(config);
		let metadata = EndpointMetadata {
			path: "/api/optional",
			method: "GET",
			name: Some("optional_endpoint"),
			function_name: "optional_endpoint",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: protection,
			guard_description: None,
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: Optional endpoints list schemes + empty object (anonymous allowed)
		let security = json["security"]
			.as_array()
			.expect("security field should be present");
		assert_eq!(
			security.len(),
			2,
			"Optional endpoint should have scheme + anonymous entry"
		);
		assert!(
			security[0].as_object().unwrap().contains_key("bearer"),
			"First entry should be the bearer scheme"
		);
		assert!(
			security[1].as_object().unwrap().is_empty(),
			"Second entry should be empty (anonymous option)"
		);
	}

	#[rstest::rstest]
	#[case::none(AuthProtection::None)]
	fn test_create_operation_none_has_no_security_field(#[case] protection: AuthProtection) {
		// Arrange
		let inspector = EndpointInspector::new();
		let metadata = EndpointMetadata {
			path: "/api/unguarded",
			method: "GET",
			name: Some("unguarded"),
			function_name: "unguarded",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: protection,
			guard_description: None,
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: None protection produces no security field
		assert!(
			json["security"].is_null(),
			"None protection should not add a security field"
		);
	}

	#[rstest::rstest]
	fn test_create_operation_guard_description_adds_x_guard_extension() {
		// Arrange
		let inspector = EndpointInspector::new();
		let metadata = EndpointMetadata {
			path: "/api/guarded",
			method: "GET",
			name: Some("guarded"),
			function_name: "guarded",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::Protected,
			guard_description: Some("HasPerm(read:items)"),
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: x-guard extension is set to the guard description
		assert_eq!(
			json["x-guard"].as_str(),
			Some("HasPerm(read:items)"),
			"x-guard extension should contain the guard description"
		);
	}

	#[rstest::rstest]
	fn test_create_operation_no_guard_description_no_x_guard_extension() {
		// Arrange
		let inspector = EndpointInspector::new();
		let metadata = EndpointMetadata {
			path: "/api/no-guard",
			method: "GET",
			name: Some("no_guard"),
			function_name: "no_guard",
			module_path: "app::views",
			request_body_type: None,
			request_content_type: None,
			responses: &[],
			headers: &[],
			security: &[],
			auth_protection: AuthProtection::Public,
			guard_description: None,
		};

		// Act
		let operation = inspector.create_operation(&metadata, vec![]);
		let json = serde_json::to_value(&operation).unwrap();

		// Assert: no x-guard extension when guard_description is None
		assert!(
			json["x-guard"].is_null(),
			"No guard_description should produce no x-guard extension"
		);
	}
}
