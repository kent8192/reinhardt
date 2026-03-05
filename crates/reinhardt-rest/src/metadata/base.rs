//! Base metadata trait and implementations

use super::fields::FieldInfo;
use super::options::MetadataOptions;
use super::response::MetadataResponse;
use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::Request;
use std::collections::HashMap;

/// Base trait for metadata providers
#[async_trait]
pub trait BaseMetadata: Send + Sync {
	/// Determine metadata for a view based on the request
	async fn determine_metadata(
		&self,
		request: &Request,
		options: &MetadataOptions,
	) -> Result<MetadataResponse>;
}

/// Simple metadata implementation
///
/// This is the default metadata implementation that returns
/// basic information about the view and its fields.
#[derive(Debug, Clone)]
pub struct SimpleMetadata {
	pub include_actions: bool,
}

impl SimpleMetadata {
	/// Creates a new `SimpleMetadata` instance with actions enabled by default
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::SimpleMetadata;
	///
	/// let metadata = SimpleMetadata::new();
	/// assert!(metadata.include_actions);
	/// ```
	pub fn new() -> Self {
		Self {
			include_actions: true,
		}
	}
	/// Configures whether to include actions in metadata responses
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::SimpleMetadata;
	///
	/// let metadata = SimpleMetadata::new().with_actions(false);
	/// assert!(!metadata.include_actions);
	///
	/// let metadata_with_actions = SimpleMetadata::new().with_actions(true);
	/// assert!(metadata_with_actions.include_actions);
	/// ```
	pub fn with_actions(mut self, include: bool) -> Self {
		self.include_actions = include;
		self
	}
	/// Convert serializer field information to metadata field information
	///
	/// This method transforms serializer field metadata into the format expected
	/// by the metadata system, using type inference to determine field types.
	///
	/// # Arguments
	///
	/// * `serializer_fields` - Map of field names to serializer field information
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{SimpleMetadata, SerializerFieldInfo};
	/// use std::collections::HashMap;
	///
	/// let metadata = SimpleMetadata::new();
	/// let mut serializer_fields = HashMap::new();
	/// serializer_fields.insert(
	///     "username".to_string(),
	///     SerializerFieldInfo {
	///         name: "username".to_string(),
	///         type_name: "String".to_string(),
	///         is_optional: false,
	///         is_read_only: false,
	///         is_write_only: false,
	///     }
	/// );
	///
	/// let fields = metadata.convert_serializer_fields(&serializer_fields);
	/// assert_eq!(fields.len(), 1);
	/// ```
	pub fn convert_serializer_fields(
		&self,
		serializer_fields: &HashMap<String, super::options::SerializerFieldInfo>,
	) -> HashMap<String, FieldInfo> {
		use super::inferencer::SchemaInferencer;

		let inferencer = SchemaInferencer::new();
		let mut fields = HashMap::new();

		for (field_name, serializer_field) in serializer_fields {
			// Use type inference to determine field type
			let mut field_info = inferencer.infer_from_type_name(&serializer_field.type_name);

			// Override required status based on is_optional
			field_info.required = !serializer_field.is_optional;

			// Set read_only and write_only flags
			field_info.read_only = Some(serializer_field.is_read_only);
			// Note: metadata FieldInfo doesn't have write_only field yet
			// This could be added in the future if needed

			fields.insert(field_name.clone(), field_info);
		}

		fields
	}

	/// Determine which actions should be available based on allowed methods
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{SimpleMetadata, FieldInfoBuilder, FieldType};
	/// use std::collections::HashMap;
	///
	/// let metadata = SimpleMetadata::new();
	/// let mut fields = HashMap::new();
	/// fields.insert(
	///     "username".to_string(),
	///     FieldInfoBuilder::new(FieldType::String).required(true).build()
	/// );
	///
	/// let allowed_methods = vec!["GET".to_string(), "POST".to_string(), "PUT".to_string()];
	/// let actions = metadata.determine_actions(&allowed_methods, &fields);
	///
	/// // GET is not included in actions, only POST and PUT
	/// assert!(!actions.contains_key("GET"));
	/// assert!(actions.contains_key("POST"));
	/// assert!(actions.contains_key("PUT"));
	/// assert_eq!(actions["POST"].len(), 1);
	/// ```
	pub fn determine_actions(
		&self,
		allowed_methods: &[String],
		fields: &HashMap<String, FieldInfo>,
	) -> HashMap<String, HashMap<String, FieldInfo>> {
		let mut actions = HashMap::new();

		for method in allowed_methods {
			let method_upper = method.to_uppercase();
			if method_upper == "POST" || method_upper == "PUT" || method_upper == "PATCH" {
				actions.insert(method_upper, fields.clone());
			}
		}

		actions
	}
}

impl Default for SimpleMetadata {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseMetadata for SimpleMetadata {
	async fn determine_metadata(
		&self,
		_request: &Request,
		options: &MetadataOptions,
	) -> Result<MetadataResponse> {
		let mut response = MetadataResponse {
			name: options.name.clone(),
			description: options.description.clone(),
			renders: Some(options.renders.clone()),
			parses: Some(options.parses.clone()),
			actions: None,
		};

		if self.include_actions {
			// Inspect serializer fields to get field metadata
			let fields = if let Some(serializer_fields) = &options.serializer_fields {
				self.convert_serializer_fields(serializer_fields)
			} else {
				HashMap::new()
			};

			let actions = self.determine_actions(&options.allowed_methods, &fields);
			if !actions.is_empty() {
				response.actions = Some(actions);
			}
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::metadata::fields::FieldInfoBuilder;
	use crate::metadata::types::{ChoiceInfo, FieldType};
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};

	fn create_test_request() -> Request {
		Request::builder()
			.method(Method::OPTIONS)
			.uri("/users/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	// DRF test: test_determine_metadata_abstract_method_raises_proper_error
	// BaseMetadata is a trait in Rust, so we test implementation requirements instead
	#[tokio::test]
	async fn test_base_metadata_trait_requires_implementation() {
		// This test verifies that BaseMetadata trait requires determine_metadata implementation
		let metadata = SimpleMetadata::new();
		let request = create_test_request();
		let options = MetadataOptions::default();

		// Should successfully call determine_metadata on a concrete implementation
		let result = metadata.determine_metadata(&request, &options).await;
		assert!(result.is_ok());
	}

	// DRF test: test_metadata
	// OPTIONS requests should return valid 200 response with metadata
	#[tokio::test]
	async fn test_metadata_basic_response() {
		let metadata = SimpleMetadata::new();
		let request = create_test_request();
		let options = MetadataOptions {
			name: "Example".to_string(),
			description: "Example view.".to_string(),
			allowed_methods: vec!["GET".to_string()],
			renders: vec!["application/json".to_string(), "text/html".to_string()],
			parses: vec![
				"application/json".to_string(),
				"application/x-www-form-urlencoded".to_string(),
				"multipart/form-data".to_string(),
			],
			serializer_fields: None,
		};

		let response = metadata
			.determine_metadata(&request, &options)
			.await
			.unwrap();

		assert_eq!(response.name, "Example");
		assert_eq!(response.description, "Example view.");
		assert_eq!(
			response.renders,
			Some(vec![
				"application/json".to_string(),
				"text/html".to_string()
			])
		);
		assert_eq!(
			response.parses,
			Some(vec![
				"application/json".to_string(),
				"application/x-www-form-urlencoded".to_string(),
				"multipart/form-data".to_string(),
			])
		);
	}

	// DRF test: test_actions
	// OPTIONS should return 'actions' key with field metadata for POST/PUT
	#[tokio::test]
	async fn test_actions_with_fields() {
		let metadata = SimpleMetadata::new();

		let mut fields = HashMap::new();

		// choice_field
		fields.insert(
			"choice_field".to_string(),
			FieldInfoBuilder::new(FieldType::Choice)
				.required(true)
				.read_only(false)
				.label("Choice field")
				.choices(vec![
					ChoiceInfo {
						value: "red".to_string(),
						display_name: "red".to_string(),
					},
					ChoiceInfo {
						value: "green".to_string(),
						display_name: "green".to_string(),
					},
					ChoiceInfo {
						value: "blue".to_string(),
						display_name: "blue".to_string(),
					},
				])
				.build(),
		);

		// integer_field
		fields.insert(
			"integer_field".to_string(),
			FieldInfoBuilder::new(FieldType::Integer)
				.required(true)
				.read_only(false)
				.label("Integer field")
				.min_value(1.0)
				.max_value(1000.0)
				.build(),
		);

		// char_field
		fields.insert(
			"char_field".to_string(),
			FieldInfoBuilder::new(FieldType::String)
				.required(false)
				.read_only(false)
				.label("Char field")
				.min_length(3)
				.max_length(40)
				.build(),
		);

		// nested_field
		let mut nested_children = HashMap::new();
		nested_children.insert(
			"a".to_string(),
			FieldInfoBuilder::new(FieldType::Integer)
				.required(true)
				.read_only(false)
				.label("A")
				.build(),
		);
		nested_children.insert(
			"b".to_string(),
			FieldInfoBuilder::new(FieldType::Integer)
				.required(true)
				.read_only(false)
				.label("B")
				.build(),
		);

		fields.insert(
			"nested_field".to_string(),
			FieldInfoBuilder::new(FieldType::NestedObject)
				.required(true)
				.read_only(false)
				.label("Nested field")
				.children(nested_children)
				.build(),
		);

		let options = MetadataOptions {
			name: "Example".to_string(),
			description: "Example view.".to_string(),
			allowed_methods: vec!["POST".to_string()],
			renders: vec!["application/json".to_string()],
			parses: vec!["application/json".to_string()],
			serializer_fields: None,
		};

		let actions = metadata.determine_actions(&options.allowed_methods, &fields);

		assert!(actions.contains_key("POST"));
		let post_fields = &actions["POST"];
		assert!(post_fields.contains_key("choice_field"));
		assert!(post_fields.contains_key("integer_field"));
		assert!(post_fields.contains_key("char_field"));
		assert!(post_fields.contains_key("nested_field"));

		// Verify choice field
		let choice_field = &post_fields["choice_field"];
		assert_eq!(choice_field.field_type, FieldType::Choice);
		assert!(choice_field.required);
		assert_eq!(choice_field.read_only, Some(false));
		assert_eq!(choice_field.choices.as_ref().unwrap().len(), 3);

		// Verify integer field
		let integer_field = &post_fields["integer_field"];
		assert_eq!(integer_field.field_type, FieldType::Integer);
		assert_eq!(integer_field.min_value, Some(1.0));
		assert_eq!(integer_field.max_value, Some(1000.0));

		// Verify nested field
		let nested_field = &post_fields["nested_field"];
		assert_eq!(nested_field.field_type, FieldType::NestedObject);
		assert!(nested_field.children.is_some());
		let children = nested_field.children.as_ref().unwrap();
		assert!(children.contains_key("a"));
		assert!(children.contains_key("b"));
	}

	#[tokio::test]
	async fn test_simple_metadata() {
		let metadata = SimpleMetadata::new();
		let request = create_test_request();
		let options = MetadataOptions {
			name: "User List".to_string(),
			description: "List all users".to_string(),
			allowed_methods: vec!["GET".to_string(), "POST".to_string()],
			renders: vec!["application/json".to_string()],
			parses: vec!["application/json".to_string()],
			serializer_fields: None,
		};

		let response = metadata
			.determine_metadata(&request, &options)
			.await
			.unwrap();

		assert_eq!(response.name, "User List");
		assert_eq!(response.description, "List all users");
		assert_eq!(response.renders, Some(vec!["application/json".to_string()]));
		assert_eq!(response.parses, Some(vec!["application/json".to_string()]));
	}

	// DRF test: test_metadata_with_serializer_inspection
	// Test that serializer fields are properly inspected and converted to metadata
	#[tokio::test]
	async fn test_metadata_with_serializer_inspection() {
		use crate::metadata::options::SerializerFieldInfo;

		let metadata = SimpleMetadata::new();
		let request = create_test_request();

		// Simulate serializer field introspection
		let mut serializer_fields = HashMap::new();
		serializer_fields.insert(
			"username".to_string(),
			SerializerFieldInfo {
				name: "username".to_string(),
				type_name: "String".to_string(),
				is_optional: false,
				is_read_only: false,
				is_write_only: false,
			},
		);
		serializer_fields.insert(
			"email".to_string(),
			SerializerFieldInfo {
				name: "email".to_string(),
				type_name: "String".to_string(),
				is_optional: false,
				is_read_only: false,
				is_write_only: false,
			},
		);
		serializer_fields.insert(
			"age".to_string(),
			SerializerFieldInfo {
				name: "age".to_string(),
				type_name: "i32".to_string(),
				is_optional: true,
				is_read_only: false,
				is_write_only: false,
			},
		);

		let options = MetadataOptions {
			name: "User Create".to_string(),
			description: "Create a new user".to_string(),
			allowed_methods: vec!["POST".to_string()],
			renders: vec!["application/json".to_string()],
			parses: vec!["application/json".to_string()],
			serializer_fields: Some(serializer_fields),
		};

		let response = metadata
			.determine_metadata(&request, &options)
			.await
			.unwrap();

		// Verify basic metadata
		assert_eq!(response.name, "User Create");
		assert_eq!(response.description, "Create a new user");

		// Verify actions were generated
		assert!(response.actions.is_some());
		let actions = response.actions.unwrap();
		assert!(actions.contains_key("POST"));

		// Verify POST action fields
		let post_fields = &actions["POST"];
		assert_eq!(post_fields.len(), 3);

		// Verify username field
		assert!(post_fields.contains_key("username"));
		let username_field = &post_fields["username"];
		assert_eq!(username_field.field_type, FieldType::String);
		assert!(username_field.required);
		assert_eq!(username_field.read_only, Some(false));

		// Verify email field
		assert!(post_fields.contains_key("email"));
		let email_field = &post_fields["email"];
		assert_eq!(email_field.field_type, FieldType::String);
		assert!(email_field.required);

		// Verify age field (optional)
		assert!(post_fields.contains_key("age"));
		let age_field = &post_fields["age"];
		assert_eq!(age_field.field_type, FieldType::Integer);
		assert!(!age_field.required); // is_optional: true
		assert_eq!(age_field.read_only, Some(false));
	}
}
