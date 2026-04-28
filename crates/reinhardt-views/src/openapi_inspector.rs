//! ViewSet inspector for OpenAPI schema generation
//!
//! This module provides functionality to extract OpenAPI schema information from ViewSets,
//! including paths, operations, parameters, and request/response schemas.

use crate::viewsets::{ActionMetadata, ViewSet};
use hyper::Method;
use reinhardt_rest::openapi::{
	Operation, Parameter, PathItem, RefOr, RequestBody, Response, /* Responses, */ Schema,
};
use std::collections::HashMap;
use utoipa::openapi::ContentBuilder;
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::path::{HttpMethod, OperationBuilder, ParameterBuilder, PathItemBuilder};
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::response::ResponseBuilder;
use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

/// Inspects ViewSets to extract schema information
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
/// # use reinhardt_views::viewsets::ModelViewSet;
/// # use reinhardt_db::orm::{FieldSelector, Model};
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// # }
/// #
/// # #[derive(Clone)] struct UserFields;
/// # impl FieldSelector for UserFields { fn with_alias(self, _: &str) -> Self { self } }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<i64> { self.id }
/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// #
/// # #[derive(Debug, Clone)]
/// # struct UserSerializer;
/// #
/// # let viewset = ModelViewSet::<User, UserSerializer>::new("users");
/// let inspector = ViewSetInspector::new();
///
/// // Extract path information
/// let paths = inspector.extract_paths(&viewset, "/api/users");
/// ```
pub struct ViewSetInspector {
	/// Configuration for schema generation
	config: InspectorConfig,
}

/// Configuration for the ViewSet inspector
#[derive(Debug, Clone)]
pub struct InspectorConfig {
	/// Include description in operations
	pub include_descriptions: bool,
	/// Include tags in operations
	pub include_tags: bool,
	/// Default response description
	pub default_response_description: String,
}

impl Default for InspectorConfig {
	fn default() -> Self {
		Self {
			include_descriptions: true,
			include_tags: true,
			default_response_description: "Successful operation".to_string(),
		}
	}
}

impl ViewSetInspector {
	/// Create a new ViewSet inspector with default configuration
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// let inspector = ViewSetInspector::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: InspectorConfig::default(),
		}
	}

	/// Create a new ViewSet inspector with custom configuration
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::{ViewSetInspector, InspectorConfig};
	/// let config = InspectorConfig {
	///     include_descriptions: false,
	///     include_tags: true,
	///     default_response_description: "Success".to_string(),
	/// };
	/// let inspector = ViewSetInspector::with_config(config);
	/// ```
	pub fn with_config(config: InspectorConfig) -> Self {
		Self { config }
	}

	/// Extract path items from a ViewSet
	///
	/// This method generates OpenAPI PathItems for all actions in the ViewSet,
	/// including both standard CRUD operations and custom actions.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// # use reinhardt_views::viewsets::ModelViewSet;
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String }
	/// # #[derive(Clone)] struct UserFields;
	/// # impl FieldSelector for UserFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Debug, Clone)]
	/// # struct UserSerializer;
	/// #
	/// # let viewset = ModelViewSet::<User, UserSerializer>::new("users");
	/// let inspector = ViewSetInspector::new();
	/// let paths = inspector.extract_paths(&viewset, "/api/users");
	///
	/// assert!(!paths.is_empty());
	/// ```
	pub fn extract_paths<V: ViewSet>(
		&self,
		viewset: &V,
		base_path: &str,
	) -> HashMap<String, PathItem> {
		let mut paths = HashMap::new();

		// Extract standard CRUD paths
		let list_detail_paths = self.extract_crud_paths(viewset, base_path);
		paths.extend(list_detail_paths);

		// Extract custom action paths
		let custom_paths = self.extract_custom_action_paths(viewset, base_path);
		paths.extend(custom_paths);

		paths
	}

	/// Extract CRUD operations for standard ViewSet actions
	fn extract_crud_paths<V: ViewSet>(
		&self,
		viewset: &V,
		base_path: &str,
	) -> HashMap<String, PathItem> {
		let mut paths = HashMap::new();
		let basename = viewset.get_basename();

		// List and Create (collection endpoint)
		let collection_path = format!("{}/", base_path.trim_end_matches('/'));
		let mut collection_item = PathItemBuilder::new();

		// GET - List
		collection_item =
			collection_item.operation(HttpMethod::Get, self.create_list_operation(basename));

		// POST - Create
		collection_item =
			collection_item.operation(HttpMethod::Post, self.create_create_operation(basename));

		paths.insert(collection_path, collection_item.build());

		// Retrieve, Update, Delete (detail endpoint)
		let detail_path = format!("{}{{id}}/", base_path.trim_end_matches('/'));
		let mut detail_item = PathItemBuilder::new();

		// GET - Retrieve
		detail_item =
			detail_item.operation(HttpMethod::Get, self.create_retrieve_operation(basename));

		// PUT - Update
		detail_item =
			detail_item.operation(HttpMethod::Put, self.create_update_operation(basename));

		// PATCH - Partial Update
		detail_item =
			detail_item.operation(HttpMethod::Patch, self.create_patch_operation(basename));

		// DELETE - Destroy
		detail_item =
			detail_item.operation(HttpMethod::Delete, self.create_destroy_operation(basename));

		paths.insert(detail_path, detail_item.build());

		paths
	}

	/// Extract paths for custom actions
	fn extract_custom_action_paths<V: ViewSet>(
		&self,
		viewset: &V,
		base_path: &str,
	) -> HashMap<String, PathItem> {
		let mut paths = HashMap::new();
		let extra_actions = viewset.get_extra_actions();
		let basename = viewset.get_basename();

		for action in extra_actions {
			let path = if action.detail {
				format!(
					"{}{{id}}/{}/",
					base_path.trim_end_matches('/'),
					action.get_url_path()
				)
			} else {
				format!(
					"{}{}/",
					base_path.trim_end_matches('/'),
					action.get_url_path()
				)
			};

			let operation = self.create_custom_operation(&action, basename);
			let mut path_item = PathItemBuilder::new();

			// Add operation for each method
			for method in &action.methods {
				let http_method = self.hyper_method_to_utoipa(method);
				path_item = path_item.operation(http_method, operation.clone());
			}

			paths.insert(path, path_item.build());
		}

		paths
	}

	/// Extract operations from a ViewSet
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// # use reinhardt_views::viewsets::ModelViewSet;
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)] struct UserFields;
	/// # impl FieldSelector for UserFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Debug, Clone)]
	/// # struct UserSerializer;
	/// #
	/// # let viewset = ModelViewSet::<User, UserSerializer>::new("users");
	/// let inspector = ViewSetInspector::new();
	/// let operations = inspector.extract_operations(&viewset);
	///
	/// assert!(!operations.is_empty());
	/// ```
	pub fn extract_operations<V: ViewSet>(&self, viewset: &V) -> Vec<Operation> {
		let mut operations = Vec::new();
		let basename = viewset.get_basename();

		// Standard CRUD operations
		operations.push(self.create_list_operation(basename));
		operations.push(self.create_retrieve_operation(basename));
		operations.push(self.create_create_operation(basename));
		operations.push(self.create_update_operation(basename));
		operations.push(self.create_patch_operation(basename));
		operations.push(self.create_destroy_operation(basename));

		// Custom actions
		for action in viewset.get_extra_actions() {
			operations.push(self.create_custom_operation(&action, basename));
		}

		operations
	}

	/// Extract schemas for request/response bodies
	///
	/// This is a fallback method when generic type information is not available.
	/// For better type safety, prefer using `extract_schema<T: ToSchema>()` instead.
	///
	/// # Arguments
	///
	/// * `model_name` - The name of the model type to include in the schema
	///
	/// # Returns
	///
	/// A basic object schema with `id` and `name` properties as placeholders.
	/// This method creates a generic schema structure that can be used when
	/// the actual model type is not known at compile time.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// let inspector = ViewSetInspector::new();
	/// let schema = inspector.extract_model_schema("User");
	///
	/// // Schema can be used in OpenAPI components
	/// ```
	///
	/// # Note
	///
	/// This method returns a placeholder schema. For actual model field extraction,
	/// implement the `ToSchema` trait on your model and use `extract_schema::<T>()`.
	pub fn extract_model_schema(&self, model_name: &str) -> Schema {
		// Create a basic object schema as placeholder
		// This is a fallback method when type information is not available at compile time.
		// For actual field extraction, use extract_schema::<T>() with the ToSchema trait.
		let id_schema = ObjectBuilder::new()
			.schema_type(SchemaType::Type(Type::Integer))
			.build();
		let name_schema = ObjectBuilder::new()
			.schema_type(SchemaType::Type(Type::String))
			.build();

		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Object))
				.property("id", Schema::Object(id_schema))
				.property("name", Schema::Object(name_schema))
				.title(Some(model_name))
				.build(),
		)
	}

	/// Extract schema from a type implementing ToSchema trait
	///
	/// This method uses the `ToSchema` trait to extract actual model field information
	/// at compile time. This is the preferred way to generate schemas for models
	/// with known types.
	///
	/// # Type Parameters
	///
	/// * `T` - The model type that implements `ToSchema`
	///
	/// # Returns
	///
	/// The schema generated by the model's `ToSchema` implementation, which includes
	/// all field definitions, types, and constraints defined in the model.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// # use reinhardt_rest::openapi::{ToSchema, Schema};
	/// # use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
	/// #
	/// # #[derive(Debug, Clone)]
	/// # struct User {
	/// #     id: i64,
	/// #     username: String,
	/// #     email: String,
	/// #     is_active: bool,
	/// # }
	/// #
	/// # impl ToSchema for User {
	/// #     fn schema() -> Schema {
	/// #         let id_schema = ObjectBuilder::new()
	/// #             .schema_type(SchemaType::Type(Type::Integer))
	/// #             .build();
	/// #         let username_schema = ObjectBuilder::new()
	/// #             .schema_type(SchemaType::Type(Type::String))
	/// #             .build();
	/// #         let email_schema = ObjectBuilder::new()
	/// #             .schema_type(SchemaType::Type(Type::String))
	/// #             .build();
	/// #         let is_active_schema = ObjectBuilder::new()
	/// #             .schema_type(SchemaType::Type(Type::Boolean))
	/// #             .build();
	/// #
	/// #         Schema::Object(
	/// #             ObjectBuilder::new()
	/// #                 .schema_type(SchemaType::Type(Type::Object))
	/// #                 .property("id", Schema::Object(id_schema))
	/// #                 .property("username", Schema::Object(username_schema))
	/// #                 .property("email", Schema::Object(email_schema))
	/// #                 .property("is_active", Schema::Object(is_active_schema))
	/// #                 .required("id")
	/// #                 .required("username")
	/// #                 .required("email")
	/// #                 .build(),
	/// #         )
	/// #     }
	/// #
	/// #     fn schema_name() -> Option<String> {
	/// #         Some("User".to_string())
	/// #     }
	/// # }
	/// let inspector = ViewSetInspector::new();
	/// let schema = inspector.extract_schema::<User>();
	///
	/// // Schema contains all User fields: id, username, email, is_active
	/// ```
	///
	/// # Note
	///
	/// You can use the `#[derive(Schema)]` macro (when available) to automatically
	/// generate the `ToSchema` implementation for your models.
	pub fn extract_schema<T: reinhardt_rest::ToSchema>(&self) -> Schema {
		T::schema()
	}

	/// Extract schema with model name from a type implementing ToSchema trait
	///
	/// This is a convenience method that extracts both the schema and its name
	/// from a type implementing `ToSchema`.
	///
	/// # Type Parameters
	///
	/// * `T` - The model type that implements `ToSchema`
	///
	/// # Returns
	///
	/// A tuple of (schema, optional_name) where the name comes from
	/// `T::schema_name()`.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_views::openapi_inspector::ViewSetInspector;
	/// # use reinhardt_rest::openapi::{ToSchema, Schema};
	/// # use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
	/// #
	/// # #[derive(Debug, Clone)]
	/// # struct User {
	/// #     id: i64,
	/// #     username: String,
	/// # }
	/// #
	/// # impl ToSchema for User {
	/// #     fn schema() -> Schema {
	/// #         Schema::Object(ObjectBuilder::new().build())
	/// #     }
	/// #     fn schema_name() -> Option<String> {
	/// #         Some("User".to_string())
	/// #     }
	/// # }
	/// let inspector = ViewSetInspector::new();
	/// let (schema, name) = inspector.extract_schema_with_name::<User>();
	///
	/// assert_eq!(name, Some("User".to_string()));
	/// ```
	pub fn extract_schema_with_name<T: reinhardt_rest::ToSchema>(
		&self,
	) -> (Schema, Option<String>) {
		(T::schema(), T::schema_name())
	}

	// Helper methods for creating operations

	fn create_list_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("list_{}", basename)))
			.summary(Some(format!("List {}", basename)))
			.response(
				"200",
				self.create_response("List of items", Some(Self::create_array_schema())),
			);

		if self.config.include_descriptions {
			builder = builder.description(Some(format!("Retrieve a list of {} items", basename)));
		}

		builder.build()
	}

	fn create_retrieve_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("retrieve_{}", basename)))
			.summary(Some(format!("Retrieve {}", basename)))
			.parameter(self.create_id_parameter())
			.response(
				"200",
				self.create_response("Item details", Some(Self::create_object_schema())),
			)
			.response("404", self.create_response("Not found", None));

		if self.config.include_descriptions {
			builder =
				builder.description(Some(format!("Retrieve a single {} item by ID", basename)));
		}

		builder.build()
	}

	fn create_create_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("create_{}", basename)))
			.summary(Some(format!("Create {}", basename)))
			.request_body(Some(self.create_request_body("Item to create")))
			.response(
				"201",
				self.create_response("Created item", Some(Self::create_object_schema())),
			)
			.response("400", self.create_response("Bad request", None));

		if self.config.include_descriptions {
			builder = builder.description(Some(format!("Create a new {} item", basename)));
		}

		builder.build()
	}

	fn create_update_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("update_{}", basename)))
			.summary(Some(format!("Update {}", basename)))
			.parameter(self.create_id_parameter())
			.request_body(Some(self.create_request_body("Item to update")))
			.response(
				"200",
				self.create_response("Updated item", Some(Self::create_object_schema())),
			)
			.response("400", self.create_response("Bad request", None))
			.response("404", self.create_response("Not found", None));

		if self.config.include_descriptions {
			builder = builder.description(Some(format!("Update an existing {} item", basename)));
		}

		builder.build()
	}

	fn create_patch_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("partial_update_{}", basename)))
			.summary(Some(format!("Partial update {}", basename)))
			.parameter(self.create_id_parameter())
			.request_body(Some(self.create_request_body("Fields to update")))
			.response(
				"200",
				self.create_response("Updated item", Some(Self::create_object_schema())),
			)
			.response("400", self.create_response("Bad request", None))
			.response("404", self.create_response("Not found", None));

		if self.config.include_descriptions {
			builder = builder.description(Some(format!(
				"Partially update an existing {} item",
				basename
			)));
		}

		builder.build()
	}

	fn create_destroy_operation(&self, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("destroy_{}", basename)))
			.summary(Some(format!("Delete {}", basename)))
			.parameter(self.create_id_parameter())
			.response("204", self.create_response("No content", None))
			.response("404", self.create_response("Not found", None));

		if self.config.include_descriptions {
			builder = builder.description(Some(format!("Delete a {} item", basename)));
		}

		builder.build()
	}

	fn create_custom_operation(&self, action: &ActionMetadata, basename: &str) -> Operation {
		let mut builder = OperationBuilder::new();

		if self.config.include_tags {
			builder = builder.tag(basename);
		}

		builder = builder
			.operation_id(Some(format!("{}_{}", action.name, basename)))
			.summary(Some(action.display_name()))
			.response(
				"200",
				self.create_response(&self.config.default_response_description, None),
			);

		if action.detail {
			builder = builder.parameter(self.create_id_parameter());
		}

		builder.build()
	}

	// Helper methods for creating parameters and schemas

	fn create_id_parameter(&self) -> Parameter {
		let id_schema = ObjectBuilder::new()
			.schema_type(SchemaType::Type(Type::Integer))
			.build();

		ParameterBuilder::new()
			.name("id")
			.parameter_in(ParameterIn::Path)
			.required(utoipa::openapi::Required::True)
			.schema(Some(Schema::Object(id_schema)))
			.description(Some("Object ID"))
			.build()
	}

	fn create_request_body(&self, description: &str) -> RequestBody {
		let content = ContentBuilder::new()
			.schema(Some(Self::create_object_schema()))
			.build();

		RequestBodyBuilder::new()
			.description(Some(description))
			.content("application/json", content)
			.required(Some(utoipa::openapi::Required::True))
			.build()
	}

	fn create_response(&self, description: &str, schema: Option<Schema>) -> RefOr<Response> {
		let mut builder = ResponseBuilder::new().description(description);

		if let Some(s) = schema {
			let content = ContentBuilder::new().schema(Some(s)).build();
			builder = builder.content("application/json", content);
		}

		RefOr::T(builder.build())
	}

	fn create_object_schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Object))
				.build(),
		)
	}

	fn create_array_schema() -> Schema {
		use utoipa::openapi::schema::Array;
		Schema::Array(Array::new(Self::create_object_schema()))
	}

	fn hyper_method_to_utoipa(&self, method: &Method) -> HttpMethod {
		match *method {
			Method::GET => HttpMethod::Get,
			Method::POST => HttpMethod::Post,
			Method::PUT => HttpMethod::Put,
			Method::PATCH => HttpMethod::Patch,
			Method::DELETE => HttpMethod::Delete,
			Method::HEAD => HttpMethod::Head,
			Method::OPTIONS => HttpMethod::Options,
			Method::TRACE => HttpMethod::Trace,
			_ => HttpMethod::Get, // Default fallback
		}
	}
}

impl Default for ViewSetInspector {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::viewsets::ModelViewSet;
	use reinhardt_db::orm::{FieldSelector, Model};
	use serde::{Deserialize, Serialize};

	// Allow dead_code: test model used as type parameter for ModelViewSet, fields not read directly
	#[allow(dead_code)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestModel {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct TestModelFields;

	impl FieldSelector for TestModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestModel {
		type PrimaryKey = i64;
		type Fields = TestModelFields;
		fn table_name() -> &'static str {
			"test_models"
		}
		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}
		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
		fn new_fields() -> Self::Fields {
			TestModelFields
		}
	}

	#[derive(Debug, Clone)]
	struct TestSerializer;

	#[test]
	fn test_viewset_inspector_new() {
		let inspector = ViewSetInspector::new();
		assert!(inspector.config.include_descriptions);
		assert!(inspector.config.include_tags);
	}

	#[test]
	fn test_extract_paths_returns_collection_and_detail() {
		let viewset = ModelViewSet::<TestModel, TestSerializer>::new("users");
		let inspector = ViewSetInspector::new();
		let paths = inspector.extract_paths(&viewset, "/api/users");

		assert!(paths.contains_key("/api/users/"));
		assert!(paths.contains_key("/api/users{id}/"));
	}

	#[test]
	fn test_extract_operations_includes_crud() {
		let viewset = ModelViewSet::<TestModel, TestSerializer>::new("users");
		let inspector = ViewSetInspector::new();
		let operations = inspector.extract_operations(&viewset);

		assert!(operations.len() >= 6); // At least the 6 CRUD operations
	}

	#[test]
	fn test_extract_model_schema_creates_object() {
		let inspector = ViewSetInspector::new();
		let schema = inspector.extract_model_schema("User");

		match schema {
			Schema::Object(_) => {}
			_ => panic!("Expected Object schema"),
		}
	}

	#[test]
	fn test_custom_config() {
		let config = InspectorConfig {
			include_descriptions: false,
			include_tags: false,
			default_response_description: "OK".to_string(),
		};
		let inspector = ViewSetInspector::with_config(config);

		assert!(!inspector.config.include_descriptions);
		assert!(!inspector.config.include_tags);
		assert_eq!(inspector.config.default_response_description, "OK");
	}

	#[test]
	fn test_extract_schema_from_type() {
		use reinhardt_rest::ToSchema;
		use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

		// Define a test model with ToSchema implementation
		#[derive(Debug, Clone)]
		struct User {
			// Allow dead_code: fields define schema structure for ToSchema impl, not accessed directly
			#[allow(dead_code)]
			id: i64,
			#[allow(dead_code)]
			username: String,
			#[allow(dead_code)]
			email: String,
			#[allow(dead_code)]
			is_active: bool,
		}

		impl ToSchema for User {
			fn schema() -> Schema {
				let id_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::Integer))
					.build();
				let username_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::String))
					.build();
				let email_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::String))
					.build();
				let is_active_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::Boolean))
					.build();

				Schema::Object(
					ObjectBuilder::new()
						.schema_type(SchemaType::Type(Type::Object))
						.property("id", Schema::Object(id_schema))
						.property("username", Schema::Object(username_schema))
						.property("email", Schema::Object(email_schema))
						.property("is_active", Schema::Object(is_active_schema))
						.required("id")
						.required("username")
						.required("email")
						.build(),
				)
			}

			fn schema_name() -> Option<String> {
				Some("User".to_string())
			}
		}

		let inspector = ViewSetInspector::new();
		let schema = inspector.extract_schema::<User>();

		// Verify schema is an Object
		match schema {
			Schema::Object(obj) => {
				// Verify properties exist
				assert!(obj.properties.contains_key("id"));
				assert!(obj.properties.contains_key("username"));
				assert!(obj.properties.contains_key("email"));
				assert!(obj.properties.contains_key("is_active"));

				// Verify required fields
				assert_eq!(obj.required.len(), 3);
				assert!(obj.required.contains(&"id".to_string()));
				assert!(obj.required.contains(&"username".to_string()));
				assert!(obj.required.contains(&"email".to_string()));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[test]
	fn test_extract_schema_with_name() {
		use reinhardt_rest::ToSchema;
		use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

		#[derive(Debug, Clone)]
		struct Product {
			// Allow dead_code: fields define schema structure for ToSchema impl, not accessed directly
			#[allow(dead_code)]
			id: i64,
			#[allow(dead_code)]
			name: String,
		}

		impl ToSchema for Product {
			fn schema() -> Schema {
				let id_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::Integer))
					.build();
				let name_schema = ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::String))
					.build();

				Schema::Object(
					ObjectBuilder::new()
						.schema_type(SchemaType::Type(Type::Object))
						.property("id", Schema::Object(id_schema))
						.property("name", Schema::Object(name_schema))
						.build(),
				)
			}

			fn schema_name() -> Option<String> {
				Some("Product".to_string())
			}
		}

		let inspector = ViewSetInspector::new();
		let (schema, name) = inspector.extract_schema_with_name::<Product>();

		// Verify name
		assert_eq!(name, Some("Product".to_string()));

		// Verify schema
		match schema {
			Schema::Object(obj) => {
				assert!(obj.properties.contains_key("id"));
				assert!(obj.properties.contains_key("name"));
			}
			_ => panic!("Expected Object schema"),
		}
	}
}
