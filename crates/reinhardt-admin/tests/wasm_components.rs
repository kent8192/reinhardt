//! WASM E2E tests for reinhardt-admin page components.
//!
//! These tests render components into the browser DOM and use
//! reinhardt-test's `Screen` fixture for Testing Library-style queries.
//!
//! Run with: `wasm-pack test --headless --chrome crates/reinhardt-admin`

#![cfg(client)]

use js_sys::{Function, Reflect};
use reinhardt_admin::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view, model_form,
	model_form_with_fieldsets, model_form_with_inlines,
};
use reinhardt_admin::pages::components::login::login_form;
use reinhardt_admin::types::{
	FieldInfo, FieldType, Fieldset, FormFieldSpec, InlineFormInfo, InlineRowInfo, InlineStyle,
	ModelInfo,
};
use reinhardt_pages::Signal;
use reinhardt_pages::component::PageExt;
use reinhardt_pages::dom::Element;
use reinhardt_pages::reactive::ReactiveScope;
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_test::wasm::{UserEvent, wait_for};
use std::collections::HashMap;
use std::time::Duration;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ============================================================================
// Login Form Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_login_form_renders_username_and_password_fields() {
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
fn model_form_retains_flat_single_card_layout() {
	// Arrange
	let fields = vec![text_field("title", "Title"), text_field("body", "Body")];

	// Act
	let html = model_form("Article", &fields, None).render_to_string();

	// Assert
	assert_eq!(html.matches(r#"class="admin-card p-6""#).count(), 1);
	assert_eq!(html.matches("<details").count(), 0);
	assert!(html.find(r#"id="field-title""#).unwrap() < html.find(r#"id="field-body""#).unwrap());
}

#[wasm_bindgen_test]
fn model_form_with_fieldsets_preserves_order_titles_and_initial_open_state() {
	// Arrange
	let fields = vec![
		text_field("title", "Title"),
		text_field("body", "Body"),
		text_field("published_at", "Published at"),
		text_field("slug", "Slug"),
	];
	let fieldsets = vec![
		Fieldset::new(Some("Main"), &["title", "body"]),
		Fieldset::new(Some("Publishing"), &["published_at"]).collapsed(),
		Fieldset::new(None, &["slug"]).collapsed(),
	];

	// Act
	let html = model_form_with_fieldsets("Article", &fields, &fieldsets, None).render_to_string();

	// Assert
	let main_summary = html.find("<summary>Main</summary>").unwrap();
	let publishing_summary = html.find("<summary>Publishing</summary>").unwrap();
	let fallback_summary = html.find("<summary>Fields</summary>").unwrap();
	assert!(main_summary < publishing_summary && publishing_summary < fallback_summary);
	let title_field = html.find(r#"id="field-title""#).unwrap();
	let body_field = html.find(r#"id="field-body""#).unwrap();
	let published_field = html.find(r#"id="field-published_at""#).unwrap();
	let slug_field = html.find(r#"id="field-slug""#).unwrap();
	assert!(
		title_field < body_field && body_field < published_field && published_field < slug_field
	);

	let details: Vec<&str> = html
		.match_indices("<details")
		.map(|(start, _)| {
			let end = html[start..].find('>').unwrap();
			&html[start..start + end]
		})
		.collect();
	assert_eq!(details.len(), 3);
	assert!(details[0].contains(" open"));
	assert!(!details[1].contains(" open"));
	assert!(!details[2].contains(" open"));
}

#[wasm_bindgen_test]
fn model_form_with_fieldsets_uses_fallback_for_blank_title() {
	for title in ["", " \t"] {
		let fields = vec![text_field("slug", "Slug")];
		let fieldsets = vec![Fieldset::new(Some(title), &["slug"])];

		let html =
			model_form_with_fieldsets("Article", &fields, &fieldsets, None).render_to_string();

		assert!(html.contains("<summary>Fields</summary>"));
	}
}

#[wasm_bindgen_test]
fn model_form_with_inlines_preserves_flat_wrapper_output() {
	// Arrange
	let fields = vec![text_field("title", "Title"), text_field("body", "Body")];

	// Act
	let wrapped = model_form("Article", &fields, None).render_to_string();
	let shared = model_form_with_inlines("Article", &fields, &[], &[], None).render_to_string();

	// Assert
	assert_eq!(shared, wrapped);
}

#[wasm_bindgen_test]
fn model_form_with_inlines_preserves_fieldset_wrapper_output() {
	// Arrange
	let fields = vec![text_field("title", "Title"), text_field("body", "Body")];
	let fieldsets = vec![Fieldset::new(Some("Content"), &["body", "title"])];

	// Act
	let wrapped =
		model_form_with_fieldsets("Article", &fields, &fieldsets, None).render_to_string();
	let shared =
		model_form_with_inlines("Article", &fields, &fieldsets, &[], None).render_to_string();

	// Assert
	assert_eq!(shared, wrapped);
}

#[wasm_bindgen_test]
fn tabular_inline_renders_accessible_schema_and_rows_in_order() {
	// Arrange
	let inline = inline_form(InlineStyle::Tabular, true);

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	assert!(html.contains(r#"<table class="admin-inline-table""#));
	assert!(html.contains("<caption"));
	let code_header = html.find(r#"scope="col">Code"#).unwrap();
	let note_header = html.find(r#"scope="col">Note"#).unwrap();
	assert!(code_header < note_header);
	let existing = html
		.find(r#"name="__reinhardt_inlines.comments-post_id.0.code""#)
		.unwrap();
	let extra = html
		.find(r#"name="__reinhardt_inlines.comments-post_id.1.code""#)
		.unwrap();
	assert!(existing < extra);
	assert!(!html.contains("trusted-fk-should-not-render"));
}

#[wasm_bindgen_test]
fn tabular_inline_uses_exact_controls_without_requiring_blank_extra_rows() {
	// Arrange
	let inline = inline_form(InlineStyle::Tabular, true);

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	assert!(html.contains(r#"name="__reinhardt_inlines.comments-post_id.0.__id" value="007""#));
	assert!(html.contains(r#"name="__reinhardt_inlines.comments-post_id.0.__delete""#));
	assert!(!html.contains(r#"name="__reinhardt_inlines.comments-post_id.1.__id""#));
	assert!(!html.contains(r#"name="__reinhardt_inlines.comments-post_id.1.__delete""#));

	let existing = opening_tag_for_name(&html, "__reinhardt_inlines.comments-post_id.0.code");
	let extra = opening_tag_for_name(&html, "__reinhardt_inlines.comments-post_id.1.code");
	assert!(existing.contains("required"));
	assert!(!extra.contains("required"));
}

#[wasm_bindgen_test]
fn inline_delete_control_requires_an_existing_row_and_delete_capability() {
	// Arrange
	let inline = inline_form(InlineStyle::Tabular, false);

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	assert!(!html.contains(".__delete\""));
}

#[wasm_bindgen_test]
fn inline_readonly_fields_render_values_without_successful_controls() {
	// Arrange
	let mut inline = inline_form(InlineStyle::Tabular, true);
	inline.fields = vec![FieldInfo {
		name: "created_by".to_string(),
		label: "Created by".to_string(),
		field_type: FieldType::Text,
		required: false,
		readonly: true,
		help_text: None,
		placeholder: None,
	}];
	inline.rows[0]
		.values
		.insert("created_by".to_string(), serde_json::json!("auditor"));

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	assert!(html.contains("auditor"));
	assert!(html.contains(r#"class="admin-inline-readonly""#));
	assert!(!html.contains(r#"name="__reinhardt_inlines.comments-post_id.0.created_by""#));
	assert!(!html.contains(r#"name="__reinhardt_inlines.comments-post_id.1.created_by""#));
}

#[wasm_bindgen_test]
fn inline_boolean_existing_value_sets_checked_without_checking_blank_extra() {
	// Arrange
	let mut inline = inline_form(InlineStyle::Tabular, true);
	inline.fields = vec![FieldInfo {
		name: "enabled".to_string(),
		label: "Enabled".to_string(),
		field_type: FieldType::Boolean,
		required: true,
		readonly: false,
		help_text: None,
		placeholder: None,
	}];
	inline.rows[0]
		.values
		.insert("enabled".to_string(), serde_json::json!(true));

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	let existing = opening_tag_for_name(&html, "__reinhardt_inlines.comments-post_id.0.enabled");
	let extra = opening_tag_for_name(&html, "__reinhardt_inlines.comments-post_id.1.enabled");
	assert!(existing.contains("checked"));
	assert!(!existing.contains("required"));
	assert!(!extra.contains("checked"));
	assert!(!extra.contains("required"));
}

#[wasm_bindgen_test]
fn stacked_inline_uses_row_fieldsets_legends_and_deterministic_error_targets() {
	// Arrange
	let inline = inline_form(InlineStyle::Stacked, true);

	// Act
	let html = model_form_with_inlines("Article", &[], &[], &[inline], None).render_to_string();

	// Assert
	assert_eq!(
		html.matches(r#"class="admin-inline-stacked-row""#).count(),
		2
	);
	assert!(html.contains("<legend>Comment 1</legend>"));
	assert!(html.contains("<legend>Comment 2</legend>"));
	for index in 0..2 {
		let error_id = format!("inline-error-comments-post_id-{index}");
		let opening = opening_tag_for_id(&html, &error_id);
		assert!(opening.contains(r#"role="alert""#));
		assert!(opening.contains(r#"aria-live="polite""#));
	}
}

#[wasm_bindgen_test(async)]
async fn structured_inline_errors_update_the_row_without_navigation() {
	// Arrange
	let error = ServerFnError::validation([
		("comments-post_id.0.code", "Code is invalid"),
		("comments-post_id.0._all", "Row is invalid"),
	]);
	let server = MutationErrorFetchGuard::install(&error);
	let root = TestBodyRoot::new("admin-inline-validation-test");
	let scope = ReactiveScope::new();
	let mut inline = inline_form(InlineStyle::Stacked, true);
	inline.fields.extend([
		FieldInfo {
			name: "external_id".to_string(),
			label: "External ID".to_string(),
			field_type: FieldType::Text,
			required: false,
			readonly: false,
			help_text: None,
			placeholder: None,
		},
		FieldInfo {
			name: "large_number".to_string(),
			label: "Large number".to_string(),
			field_type: FieldType::Number,
			required: false,
			readonly: false,
			help_text: None,
			placeholder: None,
		},
		FieldInfo {
			name: "decimal_number".to_string(),
			label: "Decimal number".to_string(),
			field_type: FieldType::Number,
			required: false,
			readonly: false,
			help_text: None,
			placeholder: None,
		},
	]);
	inline.rows[0].values.extend([
		("external_id".to_string(), serde_json::json!("00042")),
		(
			"large_number".to_string(),
			serde_json::json!("9007199254740993"),
		),
		(
			"decimal_number".to_string(),
			serde_json::json!("1234567890.123456789"),
		),
	]);
	let page = model_form_with_inlines("Article", &[], &[], &[inline], None);
	scope.enter(|| {
		page.mount(&Element::new(root.element.clone()))
			.expect("inline form mounts");
	});
	let form: web_sys::HtmlFormElement = root
		.element
		.query_selector("form")
		.expect("query form")
		.expect("form exists")
		.unchecked_into();
	let input = root
		.element
		.query_selector("#inline-field-comments-post_id-0-code")
		.expect("query inline field")
		.expect("inline field exists");
	let row_error = root
		.element
		.query_selector("#inline-error-comments-post_id-0")
		.expect("query row error")
		.expect("row error exists");
	let location_before = web_sys::window()
		.expect("window")
		.location()
		.href()
		.expect("location href");

	// Act
	UserEvent::submit(&form);
	wait_for(move || {
		row_error
			.text_content()
			.is_some_and(|text| text == "Code is invalid Row is invalid")
	})
	.with_timeout(Duration::from_secs(2))
	.await
	.expect("inline validation errors appear");

	// Assert
	assert_eq!(input.get_attribute("aria-invalid").as_deref(), Some("true"));
	assert_eq!(
		input.get_attribute("aria-describedby").as_deref(),
		Some("inline-error-comments-post_id-0")
	);
	assert_eq!(
		web_sys::window()
			.expect("window")
			.location()
			.href()
			.expect("location href"),
		location_before
	);
	let request_body = server.request_body();
	for expected in [
		r#""__reinhardt_inlines.comments-post_id.0.__id":"007""#,
		r#""__reinhardt_inlines.comments-post_id.0.external_id":"00042""#,
		r#""__reinhardt_inlines.comments-post_id.0.large_number":"9007199254740993""#,
		r#""__reinhardt_inlines.comments-post_id.0.decimal_number":"1234567890.123456789""#,
	] {
		assert!(request_body.contains(expected));
	}
}

fn text_field(name: &str, label: &str) -> FormField {
	FormField {
		name: name.to_string(),
		label: label.to_string(),
		spec: FormFieldSpec::Input {
			html_type: "text".to_string(),
		},
		required: false,
		value: String::new(),
	}
}

struct TestBodyRoot {
	element: web_sys::Element,
}

impl TestBodyRoot {
	fn new(id: &str) -> Self {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let element = document.create_element("div").expect("create root");
		element.set_id(id);
		document
			.body()
			.expect("body")
			.append_child(&element)
			.expect("append root");
		Self { element }
	}
}

impl Drop for TestBodyRoot {
	fn drop(&mut self) {
		reinhardt_pages::cleanup_reactive_nodes();
		self.element.remove();
	}
}

struct MutationErrorFetchGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
	previous_alert: JsValue,
	previous_request_body: JsValue,
}

impl MutationErrorFetchGuard {
	fn install(error: &ServerFnError) -> Self {
		let window = web_sys::window().expect("window");
		let previous_fetch =
			Reflect::get(window.as_ref(), &JsValue::from_str("fetch")).expect("read window.fetch");
		let previous_alert =
			Reflect::get(window.as_ref(), &JsValue::from_str("alert")).expect("read window.alert");
		let global = js_sys::global();
		let request_body_key = JsValue::from_str("__reinhardtInlineMutationRequestBody");
		let previous_request_body =
			Reflect::get(global.as_ref(), &request_body_key).expect("read request body probe");
		Reflect::set(global.as_ref(), &request_body_key, &JsValue::NULL)
			.expect("install request body probe");
		let body = serde_json::to_string(error).expect("serialize validation error");
		let body = serde_json::to_string(&body).expect("quote validation error body");
		let fetch = Function::new_with_args(
			"request, init",
			&format!(
				r#"
				const requestBody = request instanceof Request
					? request.clone().text()
					: Promise.resolve(init && init.body ? String(init.body) : "");
				return requestBody.then(value => {{
					globalThis.__reinhardtInlineMutationRequestBody = value;
					return new Response({body}, {{ status: 422, headers: {{ "Content-Type": "application/json" }} }});
				}});
				"#
			),
		);
		let alert = Function::new_no_args("");
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), fetch.as_ref())
			.expect("install fetch stub");
		Reflect::set(window.as_ref(), &JsValue::from_str("alert"), alert.as_ref())
			.expect("install alert stub");

		Self {
			window,
			previous_fetch,
			previous_alert,
			previous_request_body,
		}
	}

	fn request_body(&self) -> String {
		Reflect::get(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtInlineMutationRequestBody"),
		)
		.expect("read captured request body")
		.as_string()
		.expect("request body was captured")
	}
}

impl Drop for MutationErrorFetchGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("alert"),
			&self.previous_alert,
		);
		let _ = Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtInlineMutationRequestBody"),
			&self.previous_request_body,
		);
	}
}

fn inline_form(style: InlineStyle, can_delete: bool) -> InlineFormInfo {
	InlineFormInfo {
		key: "comments-post_id".to_string(),
		model_name: "Comment".to_string(),
		style,
		fields: vec![
			FieldInfo {
				name: "code".to_string(),
				label: "Code".to_string(),
				field_type: FieldType::Text,
				required: true,
				readonly: false,
				help_text: None,
				placeholder: None,
			},
			FieldInfo {
				name: "note".to_string(),
				label: "Note".to_string(),
				field_type: FieldType::TextArea,
				required: false,
				readonly: false,
				help_text: None,
				placeholder: None,
			},
		],
		rows: vec![
			InlineRowInfo {
				id: Some("007".to_string()),
				values: HashMap::from([
					("code".to_string(), serde_json::json!("existing")),
					("note".to_string(), serde_json::json!("first")),
					(
						"post_id".to_string(),
						serde_json::json!("trusted-fk-should-not-render"),
					),
				]),
			},
			InlineRowInfo {
				id: None,
				values: HashMap::new(),
			},
		],
		can_delete,
	}
}

fn opening_tag_for_name<'a>(html: &'a str, name: &str) -> &'a str {
	let marker = format!(r#"name="{name}""#);
	let attribute = html.find(&marker).unwrap();
	let start = html[..attribute].rfind('<').unwrap();
	let end = html[attribute..].find('>').unwrap() + attribute;
	&html[start..=end]
}

fn opening_tag_for_id<'a>(html: &'a str, id: &str) -> &'a str {
	let marker = format!(r#"id="{id}""#);
	let attribute = html.find(&marker).unwrap();
	let start = html[..attribute].rfind('<').unwrap();
	let end = html[attribute..].find('>').unwrap() + attribute;
	&html[start..=end]
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
