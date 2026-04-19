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
use reinhardt_admin::types::{FormFieldSpec, ModelInfo};
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
		spec: FormFieldSpec::Input {
			html_type: "text".to_string(),
		},
		required: true,
		value: String::new(),
		values: Vec::new(),
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
		spec: FormFieldSpec::Input {
			html_type: "text".to_string(),
		},
		required: true,
		value: "john_doe".to_string(),
		values: Vec::new(),
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

#[wasm_bindgen_test]
fn test_model_form_renders_textarea_for_text_area_spec() {
	let fields = vec![FormField {
		name: "bio".to_string(),
		label: "Bio".to_string(),
		spec: FormFieldSpec::TextArea,
		required: false,
		value: "Hello world".to_string(),
	}];

	let page = model_form("Profile", &fields, None);
	let html = page.render_to_string();

	assert!(
		html.contains("<textarea"),
		"TextArea spec should render a <textarea> element, got: {html}"
	);
	assert!(
		html.contains("Hello world"),
		"TextArea body should contain the field value"
	);
}

#[wasm_bindgen_test]
fn test_model_form_renders_select_with_inline_options() {
	let fields = vec![FormField {
		name: "status".to_string(),
		label: "Status".to_string(),
		spec: FormFieldSpec::Select {
			choices: vec![
				("active".to_string(), "Active".to_string()),
				("inactive".to_string(), "Inactive".to_string()),
			],
		},
		required: true,
		value: "active".to_string(),
	}];

	let page = model_form("Account", &fields, None);
	let html = page.render_to_string();

	assert!(
		html.contains("<select"),
		"Select spec should render a <select> element"
	);
	// Options must be direct children of <select>, not wrapped in <span>.
	assert!(
		!html.contains("<span><option") && !html.contains("<span> <option"),
		"Options must not be wrapped in <span> (invalid HTML), got: {html}"
	);
	assert!(
		html.contains("<option") && html.contains("Active") && html.contains("Inactive"),
		"All choices should render as <option> elements"
	);
	assert!(
		html.contains("selected"),
		"The current value should be marked selected"
	);
}

#[wasm_bindgen_test]
fn test_model_form_renders_multiselect_with_multiple_selections() {
	let fields = vec![FormField {
		name: "perms".to_string(),
		label: "Permissions".to_string(),
		spec: FormFieldSpec::MultiSelect {
			choices: vec![
				("read".to_string(), "Read".to_string()),
				("write".to_string(), "Write".to_string()),
				("delete".to_string(), "Delete".to_string()),
			],
		},
		required: false,
		// Multi-select wire format is comma-separated values; both `read`
		// and `write` should end up marked selected.
		value: "read,write".to_string(),
	}];

	let page = model_form("Role", &fields, None);
	let html = page.render_to_string();

	assert!(
		html.contains("multiple"),
		"MultiSelect spec should produce a <select multiple> element"
	);
	let selected_count = html.matches("selected").count();
	assert!(
		selected_count >= 2,
		"Both `read` and `write` should be marked selected, found {selected_count} `selected` occurrences in: {html}"
	);
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

// ============================================================================
// FormFieldSpec rendering tests (issue #3747)
//
// These tests verify that `model_form` emits the correct HTML element for
// each `FormFieldSpec` variant (TextArea/Select/MultiSelect), including
// per-choice `<option>` rendering, `selected` on the current value, the
// `multiple` attribute for MultiSelect, and the `required` attribute when
// `FormField.required` is true.
// ============================================================================

#[wasm_bindgen_test]
fn textarea_renders_as_textarea_element() {
	// Arrange
	let fields = vec![FormField {
		name: "bio".to_string(),
		label: "Biography".to_string(),
		spec: FormFieldSpec::TextArea,
		required: false,
		value: "hello world".to_string(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("User", &fields, None).render_to_string();

	// Assert: a <textarea> element is emitted with the field's id and name
	assert!(
		html.contains("<textarea"),
		"TextArea spec must render a <textarea> element, got: {html}"
	);
	assert!(
		html.contains(r#"id="field-bio""#),
		"textarea must carry the computed id attribute"
	);
	assert!(
		html.contains(r#"name="bio""#),
		"textarea must carry the field name attribute"
	);
	assert!(
		html.contains("hello world"),
		"textarea body must contain the current field value"
	);
}

#[wasm_bindgen_test]
fn textarea_required_renders_required_attr() {
	// Arrange
	let fields = vec![FormField {
		name: "bio".to_string(),
		label: "Biography".to_string(),
		spec: FormFieldSpec::TextArea,
		required: true,
		value: String::new(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("User", &fields, None).render_to_string();

	// Assert: required attribute must be present on the textarea
	let textarea_start = html
		.find("<textarea")
		.expect("required TextArea must render a <textarea> element");
	let textarea_end = html[textarea_start..]
		.find('>')
		.expect("textarea opening tag must close");
	let opening_tag = &html[textarea_start..textarea_start + textarea_end];
	assert!(
		opening_tag.contains("required"),
		"required textarea opening tag must contain `required`, got: {opening_tag}"
	);
}

#[wasm_bindgen_test]
fn select_renders_options_with_selected_current_value() {
	// Arrange: three choices, the middle one matches FormField.value
	let fields = vec![FormField {
		name: "status".to_string(),
		label: "Status".to_string(),
		spec: FormFieldSpec::Select {
			choices: vec![
				("draft".to_string(), "Draft".to_string()),
				("published".to_string(), "Published".to_string()),
				("archived".to_string(), "Archived".to_string()),
			],
		},
		required: false,
		value: "published".to_string(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("Post", &fields, None).render_to_string();

	// Assert: a <select> element is emitted, one <option> per choice, and
	// the option whose value matches FormField.value carries `selected`.
	assert!(
		html.contains("<select"),
		"Select spec must render a <select> element"
	);
	assert!(
		html.contains(r#"id="field-status""#),
		"select must carry the computed id attribute"
	);
	assert!(
		html.contains(r#"name="status""#),
		"select must carry the field name attribute"
	);
	let draft_start = html
		.find(r#"<option value="draft""#)
		.expect("draft option must be present");
	let draft_end = html[draft_start..]
		.find('>')
		.expect("draft option opening tag must close");
	let draft_tag = &html[draft_start..draft_start + draft_end];
	assert!(
		!draft_tag.contains("selected"),
		"non-selected `draft` option must render without `selected`, got: {draft_tag}"
	);
	let archived_start = html
		.find(r#"<option value="archived""#)
		.expect("archived option must be present");
	let archived_end = html[archived_start..]
		.find('>')
		.expect("archived option opening tag must close");
	let archived_tag = &html[archived_start..archived_start + archived_end];
	assert!(
		!archived_tag.contains("selected"),
		"non-selected `archived` option must render without `selected`, got: {archived_tag}"
	);
	// The currently-selected option's opening tag must carry `selected`.
	let published_start = html
		.find(r#"<option value="published""#)
		.expect("published option must be present");
	let published_end = html[published_start..]
		.find('>')
		.expect("published option opening tag must close");
	let published_tag = &html[published_start..published_start + published_end];
	assert!(
		published_tag.contains("selected"),
		"option matching FormField.value must carry `selected`, got: {published_tag}"
	);
}

#[wasm_bindgen_test]
fn select_required_renders_required_attr() {
	// Arrange
	let fields = vec![FormField {
		name: "status".to_string(),
		label: "Status".to_string(),
		spec: FormFieldSpec::Select {
			choices: vec![("a".to_string(), "A".to_string())],
		},
		required: true,
		value: String::new(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("Post", &fields, None).render_to_string();

	// Assert: required attribute must be present on the <select> opening tag
	let select_start = html
		.find("<select")
		.expect("required Select must render a <select> element");
	let select_end = html[select_start..]
		.find('>')
		.expect("select opening tag must close");
	let opening_tag = &html[select_start..select_start + select_end];
	assert!(
		opening_tag.contains("required"),
		"required select opening tag must contain `required`, got: {opening_tag}"
	);
}

#[wasm_bindgen_test]
fn multiselect_renders_as_select_with_multiple_attr() {
	// Arrange
	let fields = vec![FormField {
		name: "tags".to_string(),
		label: "Tags".to_string(),
		spec: FormFieldSpec::MultiSelect {
			choices: vec![
				("rust".to_string(), "Rust".to_string()),
				("wasm".to_string(), "WASM".to_string()),
			],
		},
		required: false,
		value: String::new(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("Post", &fields, None).render_to_string();

	// Assert: MultiSelect renders as <select> with `multiple` and both options.
	let select_start = html
		.find("<select")
		.expect("MultiSelect spec must render a <select> element");
	let select_end = html[select_start..]
		.find('>')
		.expect("select opening tag must close");
	let opening_tag = &html[select_start..select_start + select_end];
	assert!(
		opening_tag.contains("multiple"),
		"MultiSelect opening tag must contain `multiple`, got: {opening_tag}"
	);
	assert!(
		html.contains(r#"<option value="rust""#),
		"first MultiSelect option must be rendered"
	);
	assert!(
		html.contains(r#"<option value="wasm""#),
		"second MultiSelect option must be rendered"
	);
}

#[wasm_bindgen_test]
fn multiselect_required_renders_required_attr() {
	// Arrange
	let fields = vec![FormField {
		name: "tags".to_string(),
		label: "Tags".to_string(),
		spec: FormFieldSpec::MultiSelect {
			choices: vec![("rust".to_string(), "Rust".to_string())],
		},
		required: true,
		value: String::new(),
		values: Vec::new(),
	}];

	// Act
	let html = model_form("Post", &fields, None).render_to_string();

	// Assert: required attribute must be present on the <select> opening tag
	let select_start = html
		.find("<select")
		.expect("required MultiSelect must render a <select> element");
	let select_end = html[select_start..]
		.find('>')
		.expect("select opening tag must close");
	let opening_tag = &html[select_start..select_start + select_end];
	assert!(
		opening_tag.contains("required"),
		"required MultiSelect opening tag must contain `required`, got: {opening_tag}"
	);
	assert!(
		opening_tag.contains("multiple"),
		"required MultiSelect must still carry `multiple`"
	);
}
