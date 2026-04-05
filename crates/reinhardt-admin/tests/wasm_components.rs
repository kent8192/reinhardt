//! WASM E2E tests for reinhardt-admin page components.
//!
//! These tests render components into the browser DOM and use
//! reinhardt-test's `Screen` fixture for Testing Library-style queries.
//!
//! Run with: `wasm-pack test --headless --chrome crates/reinhardt-admin`

#![cfg(client)]

use reinhardt_admin::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view, model_form,
};
use reinhardt_admin::pages::components::login::login_form;
use reinhardt_admin::types::ModelInfo;
use reinhardt_pages::Signal;
use reinhardt_test::fixtures::wasm::*;
use std::collections::HashMap;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ============================================================================
// Login Form Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_login_form_renders_username_and_password_fields() {
	let screen = screen();
	let page = login_form(None);
	let html = page.render_to_string();

	// Mount to DOM
	let document = web_sys::window().unwrap().document().unwrap();
	let container = document.create_element("div").unwrap();
	container.set_inner_html(&html);
	document.body().unwrap().append_child(&container).unwrap();

	// Assert form fields exist
	assert!(html.contains("username"), "Should contain username field");
	assert!(html.contains("password"), "Should contain password field");
	assert!(html.contains("Sign in"), "Should contain submit button");
	assert!(html.contains("Admin Login"), "Should contain login heading");

	// Cleanup
	container.remove();
}

#[wasm_bindgen_test]
fn test_login_form_with_error_displays_alert() {
	let page = login_form(Some("Invalid credentials"));
	let html = page.render_to_string();

	assert!(
		html.contains("Invalid credentials"),
		"Should display error message"
	);
	assert!(
		html.contains("admin-alert-danger"),
		"Should have danger alert class"
	);
}

// ============================================================================
// Dashboard Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_dashboard_renders_model_cards() {
	let models = vec![
		ModelInfo {
			name: "Users".to_string(),
			list_url: "/admin/users/".to_string(),
		},
		ModelInfo {
			name: "Posts".to_string(),
			list_url: "/admin/posts/".to_string(),
		},
	];

	let page = dashboard("Test Admin", &models);
	let html = page.render_to_string();

	assert!(
		html.contains("Test Admin Dashboard"),
		"Should display dashboard title"
	);
	assert!(html.contains("Users"), "Should display Users model card");
	assert!(html.contains("Posts"), "Should display Posts model card");
	assert!(html.contains("/admin/users/"), "Should link to users list");
}

#[wasm_bindgen_test]
fn test_dashboard_empty_models_shows_info_alert() {
	let page = dashboard("Test Admin", &[]);
	let html = page.render_to_string();

	assert!(
		html.contains("No models registered"),
		"Should show info message for empty models"
	);
}

// ============================================================================
// Detail View Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_detail_view_renders_record_data() {
	let mut record = HashMap::new();
	record.insert("id".to_string(), "42".to_string());
	record.insert("name".to_string(), "Test Record".to_string());

	let page = detail_view("User", "42", &record);
	let html = page.render_to_string();

	assert!(html.contains("User Detail"), "Should show model name");
	assert!(html.contains("Test Record"), "Should show record value");
	assert!(html.contains("Edit"), "Should have edit link");
	assert!(html.contains("Back to List"), "Should have back link");
}

// ============================================================================
// Model Form Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_model_form_create_mode() {
	let fields = vec![FormField {
		name: "username".to_string(),
		label: "Username".to_string(),
		field_type: "text".to_string(),
		required: true,
		value: String::new(),
	}];

	let page = model_form("User", &fields, None);
	let html = page.render_to_string();

	assert!(html.contains("Create User"), "Should show create title");
	assert!(
		html.contains("/admin/user/add/"),
		"Should have create action URL"
	);
	assert!(html.contains("Username"), "Should show field label");
	assert!(html.contains("required"), "Should mark required field");
}

#[wasm_bindgen_test]
fn test_model_form_edit_mode() {
	let fields = vec![FormField {
		name: "username".to_string(),
		label: "Username".to_string(),
		field_type: "text".to_string(),
		required: true,
		value: "john_doe".to_string(),
	}];

	let page = model_form("User", &fields, Some("42"));
	let html = page.render_to_string();

	assert!(html.contains("Edit User"), "Should show edit title");
	assert!(
		html.contains("/admin/user/42/change/"),
		"Should have edit action URL"
	);
	assert!(html.contains("john_doe"), "Should pre-fill existing value");
}

// ============================================================================
// List View Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_list_view_renders_table_with_data() {
	let mut record = HashMap::new();
	record.insert("id".to_string(), "1".to_string());
	record.insert("name".to_string(), "Alice".to_string());

	let data = ListViewData {
		model_name: "User".to_string(),
		columns: vec![
			Column {
				field: "id".to_string(),
				label: "ID".to_string(),
				sortable: true,
			},
			Column {
				field: "name".to_string(),
				label: "Name".to_string(),
				sortable: true,
			},
		],
		records: vec![record],
		current_page: 1,
		total_pages: 3,
		total_count: 25,
		filters: vec![],
	};

	let page_signal = Signal::new(1u64);
	let filters_signal = Signal::new(HashMap::new());
	let page = list_view(&data, page_signal, filters_signal);
	let html = page.render_to_string();

	assert!(html.contains("User List"), "Should show list title");
	assert!(html.contains("Alice"), "Should show record data");
	assert!(html.contains("ID"), "Should show column header");
	assert!(html.contains("Name"), "Should show column header");
	assert!(html.contains("Showing 25 User"), "Should show record count");
}
