//! Integration tests for Browsable API
//!
//! These tests verify that reinhardt-browsable-api works correctly for rendering
//! interactive HTML API documentation.

use reinhardt_rest::browsable_api::{ApiContext, BrowsableApiRenderer, FormContext, FormField};
use serde_json::json;

// ============================================================================
// API Browser Rendering Tests
// ============================================================================

#[test]
fn test_browsable_api_html_response() {
	let renderer = BrowsableApiRenderer::new();

	let context = ApiContext {
		title: "User List API".to_string(),
		description: Some("Get a list of all users".to_string()),
		endpoint: "/api/users/".to_string(),
		method: "GET".to_string(),
		response_data: json!([
			{"id": 1, "name": "Alice", "email": "alice@example.com"},
			{"id": 2, "name": "Bob", "email": "bob@example.com"}
		]),
		response_status: 200,
		allowed_methods: vec!["GET".to_string(), "POST".to_string()],
		request_form: None,
		headers: vec![("Content-Type".to_string(), "application/json".to_string())],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify HTML structure and content
	assert!(
		html.contains("<!DOCTYPE html>"),
		"HTML should start with DOCTYPE, got: {}",
		&html[..100.min(html.len())]
	);
	assert!(
		html.contains("<html>"),
		"HTML should have html element, got: {}",
		&html[..200.min(html.len())]
	);
	assert!(
		html.contains("<title>User List API - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>User List API</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("Get a list of all users"),
		"HTML should contain description text, got: {}",
		&html[..1200.min(html.len())]
	);
	// Check endpoint URL - Tera HTML-encodes forward slashes as &#x2F;
	// so we check for either the raw or encoded version
	assert!(
		html.contains("/api/users/") || html.contains("&#x2F;api&#x2F;users&#x2F;"),
		"HTML should display endpoint URL, got: {}",
		&html[..1200.min(html.len())]
	);
	assert!(
		html.contains("Alice") && html.contains("alice@example.com"),
		"HTML should display user data from JSON response, got partial: {}",
		&html[..2000.min(html.len())]
	);
}

#[test]
fn test_browsable_api_json_data_display() {
	let renderer = BrowsableApiRenderer::new();

	let context = ApiContext {
		title: "Product Detail".to_string(),
		description: None,
		endpoint: "/api/products/42/".to_string(),
		method: "GET".to_string(),
		response_data: json!({
			"id": 42,
			"name": "Laptop",
			"price": 999.99,
			"in_stock": true,
			"specs": {
				"cpu": "Intel i7",
				"ram": "16GB"
			}
		}),
		response_status: 200,
		allowed_methods: vec!["GET".to_string(), "PUT".to_string(), "DELETE".to_string()],
		request_form: None,
		headers: vec![],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify JSON data is displayed with exact values
	assert!(
		html.contains("<title>Product Detail - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Product Detail</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("Laptop"),
		"HTML should display product name, got: {}",
		&html[..2000.min(html.len())]
	);
	assert!(
		html.contains("999.99"),
		"HTML should display product price, got: {}",
		&html[..2000.min(html.len())]
	);
	assert!(
		html.contains("Intel i7") && html.contains("16GB"),
		"HTML should display nested specs data, got: {}",
		&html[..2000.min(html.len())]
	);
}

// ============================================================================
// Interactive Features Tests
// ============================================================================

#[test]
fn test_browsable_api_form_rendering() {
	let renderer = BrowsableApiRenderer::new();

	let form = FormContext {
		fields: vec![
			FormField {
				name: "title".to_string(),
				label: "Title".to_string(),
				field_type: "text".to_string(),
				required: true,
				help_text: Some("Enter the post title".to_string()),
				initial_value: None,
				options: None,
				initial_label: None,
			},
			FormField {
				name: "content".to_string(),
				label: "Content".to_string(),
				field_type: "textarea".to_string(),
				required: true,
				help_text: None,
				initial_value: None,
				options: None,
				initial_label: None,
			},
		],
		submit_url: "/api/posts/".to_string(),
		submit_method: "POST".to_string(),
	};

	let context = ApiContext {
		title: "Create Post".to_string(),
		description: Some("Create a new blog post".to_string()),
		endpoint: "/api/posts/".to_string(),
		method: "POST".to_string(),
		response_data: json!({}),
		response_status: 201,
		allowed_methods: vec!["POST".to_string()],
		request_form: Some(form),
		headers: vec![],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify form elements are present with specific HTML structure
	assert!(
		html.contains("<title>Create Post - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Create Post</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("<form"),
		"HTML should contain form element, got: {}",
		&html[..2000.min(html.len())]
	);
	assert!(
		html.contains("name=\"title\""),
		"Form should have title field, got: {}",
		&html[..3000.min(html.len())]
	);
	assert!(
		html.contains("name=\"content\""),
		"Form should have content field, got: {}",
		&html[..3000.min(html.len())]
	);
	assert!(
		html.contains("Enter the post title"),
		"Form should display help text, got: {}",
		&html[..3000.min(html.len())]
	);
}

#[test]
fn test_browsable_api_method_selection() {
	let renderer = BrowsableApiRenderer::new();

	let context = ApiContext {
		title: "Resource Endpoint".to_string(),
		description: None,
		endpoint: "/api/resource/1/".to_string(),
		method: "GET".to_string(),
		response_data: json!({"id": 1}),
		response_status: 200,
		allowed_methods: vec![
			"GET".to_string(),
			"POST".to_string(),
			"PUT".to_string(),
			"PATCH".to_string(),
			"DELETE".to_string(),
		],
		request_form: None,
		headers: vec![],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify all HTTP methods are displayed in allowed methods section
	assert!(
		html.contains("<title>Resource Endpoint - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Resource Endpoint</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("<strong>Allowed methods:</strong>"),
		"HTML should have allowed methods section with exact label, got: {}",
		&html[..2000.min(html.len())]
	);
	// Verify all methods are present as badge elements
	let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
	for method in &methods {
		assert!(
			html.contains(method),
			"HTML should contain {} method in allowed methods, got: {}",
			method,
			&html[..3000.min(html.len())]
		);
	}
}

#[test]
fn test_browsable_api_authentication_ui() {
	let renderer = BrowsableApiRenderer::new();

	// Simulate authenticated request
	let context = ApiContext {
		title: "Authenticated Endpoint".to_string(),
		description: Some("Requires authentication".to_string()),
		endpoint: "/api/protected/".to_string(),
		method: "GET".to_string(),
		response_data: json!({"message": "Authenticated"}),
		response_status: 200,
		allowed_methods: vec!["GET".to_string()],
		request_form: None,
		headers: vec![("Authorization".to_string(), "Bearer token123".to_string())],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify authentication-related content is displayed
	assert!(
		html.contains("<title>Authenticated Endpoint - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Authenticated Endpoint</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("Requires authentication"),
		"HTML should display authentication requirement in description, got: {}",
		&html[..2000.min(html.len())]
	);
}

// ============================================================================
// Content Negotiation Tests
// ============================================================================

#[test]
fn test_browsable_api_accepts_header() {
	let renderer = BrowsableApiRenderer::new();

	// HTML Accept header scenario
	let context = ApiContext {
		title: "Content Negotiation Test".to_string(),
		description: None,
		endpoint: "/api/data/".to_string(),
		method: "GET".to_string(),
		response_data: json!({"data": "test"}),
		response_status: 200,
		allowed_methods: vec!["GET".to_string()],
		request_form: None,
		headers: vec![("Accept".to_string(), "text/html".to_string())],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Should render HTML for browser with proper structure
	assert!(
		html.contains("<!DOCTYPE html>"),
		"HTML should start with DOCTYPE for browser rendering, got: {}",
		&html[..100.min(html.len())]
	);
	assert!(
		html.contains("<title>Content Negotiation Test - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Content Negotiation Test</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
}

#[test]
fn test_browsable_api_json_response_display() {
	let renderer = BrowsableApiRenderer::new();

	// JSON Accept header - but still display in browsable UI
	let context = ApiContext {
		title: "JSON Response".to_string(),
		description: None,
		endpoint: "/api/json/".to_string(),
		method: "GET".to_string(),
		response_data: json!({
			"status": "success",
			"data": [1, 2, 3, 4, 5]
		}),
		response_status: 200,
		allowed_methods: vec!["GET".to_string()],
		request_form: None,
		headers: vec![("Content-Type".to_string(), "application/json".to_string())],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// JSON data should be visible in HTML with exact values
	assert!(
		html.contains("<title>JSON Response - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>JSON Response</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("success"),
		"HTML should display JSON status field, got: {}",
		&html[..2000.min(html.len())]
	);
	assert!(
		html.contains("data"),
		"HTML should display JSON data field name, got: {}",
		&html[..2000.min(html.len())]
	);
}

// ============================================================================
// Response Status Tests
// ============================================================================

#[test]
fn test_browsable_api_error_response() {
	let renderer = BrowsableApiRenderer::new();

	let context = ApiContext {
		title: "Error Response".to_string(),
		description: None,
		endpoint: "/api/error/".to_string(),
		method: "POST".to_string(),
		response_data: json!({
			"error": "Validation failed",
			"details": {
				"email": ["This field is required"]
			}
		}),
		response_status: 400,
		allowed_methods: vec!["POST".to_string()],
		request_form: None,
		headers: vec![],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Error details should be visible with exact messages
	assert!(
		html.contains("<title>Error Response - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Error Response</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("Validation failed"),
		"HTML should display error message in response data, got: {}",
		&html[..2000.min(html.len())]
	);
	assert!(
		html.contains("This field is required"),
		"HTML should display validation error detail in response data, got: {}",
		&html[..3000.min(html.len())]
	);
}

#[test]
fn test_browsable_api_success_created() {
	let renderer = BrowsableApiRenderer::new();

	let context = ApiContext {
		title: "Resource Created".to_string(),
		description: None,
		endpoint: "/api/items/".to_string(),
		method: "POST".to_string(),
		response_data: json!({
			"id": 123,
			"created_at": "2024-01-01T00:00:00Z"
		}),
		response_status: 201,
		allowed_methods: vec!["GET".to_string(), "POST".to_string()],
		request_form: None,
		headers: vec![("Location".to_string(), "/api/items/123/".to_string())],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify created resource response
	assert!(
		html.contains("<title>Resource Created - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Resource Created</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("123"),
		"HTML should display created resource ID in response data, got: {}",
		&html[..2000.min(html.len())]
	);
}

// ============================================================================
// Form Field Types Tests
// ============================================================================

#[test]
fn test_browsable_api_various_field_types() {
	let renderer = BrowsableApiRenderer::new();

	let form = FormContext {
		fields: vec![
			FormField {
				name: "name".to_string(),
				label: "Name".to_string(),
				field_type: "text".to_string(),
				required: true,
				help_text: None,
				initial_value: Some(json!("John Doe")),
				options: None,
				initial_label: None,
			},
			FormField {
				name: "age".to_string(),
				label: "Age".to_string(),
				field_type: "number".to_string(),
				required: false,
				help_text: Some("Must be 18 or older".to_string()),
				initial_value: None,
				options: None,
				initial_label: None,
			},
			FormField {
				name: "bio".to_string(),
				label: "Biography".to_string(),
				field_type: "textarea".to_string(),
				required: false,
				help_text: None,
				initial_value: None,
				options: None,
				initial_label: None,
			},
		],
		submit_url: "/api/profile/".to_string(),
		submit_method: "PUT".to_string(),
	};

	let context = ApiContext {
		title: "Update Profile".to_string(),
		description: None,
		endpoint: "/api/profile/".to_string(),
		method: "PUT".to_string(),
		response_data: json!({}),
		response_status: 200,
		allowed_methods: vec!["GET".to_string(), "PUT".to_string()],
		request_form: Some(form),
		headers: vec![],
		csrf_token: None,
	};

	let html = renderer.render(&context).unwrap();

	// Verify different field types are rendered correctly
	assert!(
		html.contains("<title>Update Profile - Reinhardt API</title>"),
		"HTML should have exact title with Reinhardt API suffix, got: {}",
		&html[..500.min(html.len())]
	);
	assert!(
		html.contains("<h1>Update Profile</h1>"),
		"HTML should have h1 with title, got: {}",
		&html[..1000.min(html.len())]
	);
	assert!(
		html.contains("name=\"name\""),
		"Form should have name field with exact attribute, got: {}",
		&html[..3000.min(html.len())]
	);
	assert!(
		html.contains("name=\"age\""),
		"Form should have age field with exact attribute, got: {}",
		&html[..3000.min(html.len())]
	);
	assert!(
		html.contains("name=\"bio\""),
		"Form should have bio field with exact attribute, got: {}",
		&html[..3000.min(html.len())]
	);
	assert!(
		html.contains("Must be 18 or older"),
		"Form should display field help text, got: {}",
		&html[..3000.min(html.len())]
	);
}
