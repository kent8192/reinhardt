//! Integration tests for reinhardt-metadata
//!
//! These tests require multiple crates to work together and verify
//! metadata generation in complex scenarios.

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::{Error, Request, Response, Result};
use reinhardt_rest::metadata::{
	BaseMetadata, FieldInfoBuilder, FieldType, MetadataOptions, SimpleMetadata,
};
use reinhardt_rest::versioning::{BaseVersioning, QueryParameterVersioning};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ========================================================================
// Helper structures for testing
// ========================================================================

/// Simple permission trait for testing
trait TestPermission: Send + Sync {
	fn has_permission(&self, is_authenticated: bool) -> bool;
}

/// Allow any access
struct AllowAny;

impl TestPermission for AllowAny {
	fn has_permission(&self, _is_authenticated: bool) -> bool {
		true
	}
}

/// Require authentication
struct IsAuthenticated;

impl TestPermission for IsAuthenticated {
	fn has_permission(&self, is_authenticated: bool) -> bool {
		is_authenticated
	}
}

/// Mock APIView for testing
struct MockAPIView {
	metadata_class: Option<Arc<dyn BaseMetadata>>,
	name: String,
	description: String,
	allowed_methods: Vec<String>,
}

impl MockAPIView {
	fn new(name: &str, description: &str) -> Self {
		Self {
			metadata_class: Some(Arc::new(SimpleMetadata::new())),
			name: name.to_string(),
			description: description.to_string(),
			allowed_methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string()],
		}
	}

	fn with_metadata_class(mut self, metadata: Option<Arc<dyn BaseMetadata>>) -> Self {
		self.metadata_class = metadata;
		self
	}

	#[allow(dead_code)]
	fn with_allowed_methods(mut self, methods: Vec<String>) -> Self {
		self.allowed_methods = methods;
		self
	}

	async fn handle_options(&self, request: &Request) -> Result<Response> {
		if self.metadata_class.is_none() {
			return Err(Error::Http("Method \"OPTIONS\" not allowed.".to_string()));
		}

		let metadata = self.metadata_class.as_ref().unwrap();
		// Use field mutation because MetadataOptions is #[non_exhaustive]
		let mut options = MetadataOptions::default();
		options.name = self.name.clone();
		options.description = self.description.clone();
		options.allowed_methods = self.allowed_methods.clone();
		options.renders = vec!["application/json".to_string(), "text/html".to_string()];
		options.parses = vec![
			"application/json".to_string(),
			"application/x-www-form-urlencoded".to_string(),
			"multipart/form-data".to_string(),
		];

		let metadata_response = metadata.determine_metadata(request, &options).await?;
		let body = serde_json::to_vec(&metadata_response).unwrap();

		Ok(Response::new(StatusCode::OK).with_body(body))
	}
}

/// Mock APIView with permission checking
struct PermissionAPIView {
	metadata_class: Arc<dyn BaseMetadata>,
	name: String,
	description: String,
	allowed_methods: Vec<String>,
	post_permission: Box<dyn TestPermission>,
	put_permission: Box<dyn TestPermission>,
}

impl PermissionAPIView {
	fn new<P1: TestPermission + 'static, P2: TestPermission + 'static>(
		name: &str,
		post_perm: P1,
		put_perm: P2,
	) -> Self {
		Self {
			metadata_class: Arc::new(SimpleMetadata::new()),
			name: name.to_string(),
			description: "Example view.".to_string(),
			allowed_methods: vec!["POST".to_string(), "PUT".to_string()],
			post_permission: Box::new(post_perm),
			put_permission: Box::new(put_perm),
		}
	}

	fn check_permissions(&self, request: &Request, method: &str) -> bool {
		let user_header = request.headers.get("X-User-Authenticated");
		let is_authenticated = user_header
			.and_then(|v| v.to_str().ok())
			.map(|v| v == "true")
			.unwrap_or(false);

		match method {
			"POST" => self.post_permission.has_permission(is_authenticated),
			"PUT" => self.put_permission.has_permission(is_authenticated),
			_ => true,
		}
	}

	async fn handle_options(&self, request: &Request) -> Result<Response> {
		let mut filtered_methods = Vec::new();

		for method in &self.allowed_methods {
			if self.check_permissions(request, method) {
				filtered_methods.push(method.clone());
			}
		}

		// Use field mutation because MetadataOptions is #[non_exhaustive]
		let mut options = MetadataOptions::default();
		options.name = self.name.clone();
		options.description = self.description.clone();
		options.allowed_methods = filtered_methods.clone();
		// renders and parses already default to ["application/json"]

		let fields = HashMap::new();
		let actions = SimpleMetadata::new().determine_actions(&filtered_methods, &fields);

		let mut metadata_response = self
			.metadata_class
			.determine_metadata(request, &options)
			.await?;

		if !actions.is_empty() {
			metadata_response.actions = Some(actions);
		}

		let body = serde_json::to_vec(&metadata_response).unwrap();
		Ok(Response::new(StatusCode::OK).with_body(body))
	}
}

/// Mock APIView with versioning support
struct VersionedAPIView {
	metadata_class: Arc<dyn BaseMetadata>,
	versioning_scheme: Arc<dyn BaseVersioning>,
	request_version: Arc<RwLock<Option<String>>>,
}

impl VersionedAPIView {
	fn new(versioning: Arc<dyn BaseVersioning>) -> Self {
		Self {
			metadata_class: Arc::new(SimpleMetadata::new()),
			versioning_scheme: versioning,
			request_version: Arc::new(RwLock::new(None)),
		}
	}

	async fn handle_options(&self, request: &Request) -> Result<Response> {
		// Determine version and store it
		let version = self.versioning_scheme.determine_version(request).await?;
		*self.request_version.write().await = Some(version.clone());

		// Verify that version is accessible (simulating get_serializer access)
		let stored_version = self.request_version.read().await;
		assert!(
			stored_version.is_some(),
			"Request should have version attribute"
		);

		// Use field mutation because MetadataOptions is #[non_exhaustive]
		let mut options = MetadataOptions::default();
		options.name = "Example".to_string();
		options.description = "Example view.".to_string();
		options.allowed_methods = vec!["POST".to_string()];
		// renders and parses already default to ["application/json"]

		let metadata_response = self
			.metadata_class
			.determine_metadata(request, &options)
			.await?;
		let body = serde_json::to_vec(&metadata_response).unwrap();

		Ok(Response::new(StatusCode::OK).with_body(body))
	}

	fn get_versioning_scheme(&self) -> &Arc<dyn BaseVersioning> {
		&self.versioning_scheme
	}
}

// ========================================================================
// Integration Tests
// ========================================================================

// Test 1: test_none_metadata
// When metadata_class = None, OPTIONS should return HTTP 405
#[tokio::test]
async fn test_none_metadata() {
	let view = MockAPIView::new("Example", "Example view.").with_metadata_class(None);

	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let result = view.handle_options(&request).await;

	assert!(result.is_err());
	match result {
		Err(Error::Http(msg)) => {
			assert_eq!(msg, "Method \"OPTIONS\" not allowed.");
		}
		_ => panic!("Expected Http error with MethodNotAllowed message"),
	}
}

// Test 2: test_global_permissions
// Metadata should exclude actions without global permissions
#[tokio::test]
async fn test_global_permissions() {
	// POST requires authentication (will be denied)
	// PUT allows anyone (will be allowed)
	let view = PermissionAPIView::new(
		"Example",
		IsAuthenticated, // POST requires auth
		AllowAny,        // PUT allows anyone
	);

	let headers = HeaderMap::new();
	// No authentication header - user is not authenticated

	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = view.handle_options(&request).await.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_bytes = response.body.to_vec();
	let data: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

	// Only PUT should be in actions (POST was denied due to permission)
	let actions = data["actions"].as_object().unwrap();
	assert!(actions.contains_key("PUT"));
	assert!(!actions.contains_key("POST"));
}

// Test 3: test_object_permissions
// Metadata should exclude actions without object permissions
#[tokio::test]
async fn test_object_permissions() {
	// POST allows anyone (will be allowed)
	// PUT requires authentication (will be denied)
	let view = PermissionAPIView::new(
		"Example",
		AllowAny,        // POST allows anyone
		IsAuthenticated, // PUT requires auth
	);

	let headers = HeaderMap::new();
	// No authentication - PUT will be denied

	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = view.handle_options(&request).await.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_bytes = response.body.to_vec();
	let data: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

	// Only POST should be in actions (PUT was denied)
	let actions = data["actions"].as_object().unwrap();
	assert!(actions.contains_key("POST"));
	assert!(!actions.contains_key("PUT"));
}

// Test 4: test_bug_2455_clone_request
// Cloned request should have 'version' attribute
#[tokio::test]
async fn test_bug_2455_clone_request() {
	let versioning = Arc::new(
		QueryParameterVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1.0", "2.0"]),
	);

	let view = VersionedAPIView::new(versioning);

	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/?version=2.0")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// This should not panic - version attribute should be accessible
	let response = view.handle_options(&request).await.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	// Verify version was stored and accessible
	let version = view.request_version.read().await;
	assert_eq!(version.as_ref().unwrap(), "2.0");
}

// Test 5: test_bug_2477_clone_request
// Cloned request should have 'versioning_scheme' attribute
#[tokio::test]
async fn test_bug_2477_clone_request() {
	let versioning = Arc::new(
		QueryParameterVersioning::new()
			.with_version_param("v")
			.with_default_version("1.0"),
	);

	let view = VersionedAPIView::new(versioning);

	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/?v=1.0")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// This should not panic - versioning_scheme should be accessible
	let response = view.handle_options(&request).await.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	// Verify versioning scheme is accessible
	let scheme = view.get_versioning_scheme();
	assert_eq!(scheme.version_param(), "v");
}

// Test 6: test_read_only_primary_key_related_field
// Metadata generation should work with read-only PrimaryKeyRelatedField
#[tokio::test]
async fn test_read_only_primary_key_related_field() {
	// Simulate ModelSerializer with read-only PrimaryKeyRelatedField
	let metadata = SimpleMetadata::new();
	let request = Request::builder()
		.method(Method::OPTIONS)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let mut fields = HashMap::new();

	// Simulate model fields
	fields.insert(
		"id".to_string(),
		FieldInfoBuilder::new(FieldType::Integer)
			.required(false)
			.read_only(true)
			.label("ID")
			.build(),
	);

	// Read-only PrimaryKeyRelatedField (children many-to-many)
	fields.insert(
		"children".to_string(),
		FieldInfoBuilder::new(FieldType::Field)
			.required(false)
			.read_only(true)
			.label("Children")
			.build(),
	);

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

	fields.insert(
		"name".to_string(),
		FieldInfoBuilder::new(FieldType::String)
			.required(false)
			.read_only(false)
			.label("Name")
			.max_length(100)
			.build(),
	);

	// Use field mutation because MetadataOptions is #[non_exhaustive]
	let mut options = MetadataOptions::default();
	options.name = "Example".to_string();
	options.description = "Example view.".to_string();
	options.allowed_methods = vec!["POST".to_string()];
	options.renders = vec!["application/json".to_string(), "text/html".to_string()];
	options.parses = vec![
		"application/json".to_string(),
		"application/x-www-form-urlencoded".to_string(),
		"multipart/form-data".to_string(),
	];

	let metadata_response = metadata
		.determine_metadata(&request, &options)
		.await
		.unwrap();
	let actions = metadata.determine_actions(&options.allowed_methods, &fields);

	assert_eq!(metadata_response.name, "Example");
	assert_eq!(metadata_response.description, "Example view.");
	assert!(actions.contains_key("POST"));

	let post_fields = &actions["POST"];
	assert!(post_fields.contains_key("id"));
	assert!(post_fields.contains_key("children"));
	assert!(post_fields.contains_key("integer_field"));
	assert!(post_fields.contains_key("name"));

	// Verify children field is read-only
	assert_eq!(post_fields["children"].read_only, Some(true));
	assert!(!post_fields["children"].required);
}
