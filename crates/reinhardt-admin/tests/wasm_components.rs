//! WASM E2E tests for reinhardt-admin page components.
//!
//! These tests render components into the browser DOM and use
//! reinhardt-test's `Screen` fixture for Testing Library-style queries.
//!
//! Run with: `wasm-pack test --headless --chrome crates/reinhardt-admin`

#![cfg(client)]

use js_sys::{Function, Reflect};
use reinhardt_admin::pages::components::features::{
	Column, FormField, ListViewData, dashboard, detail_view, list_view, list_view_with_actions,
	model_form, model_form_with_fieldsets, model_form_with_inlines,
};
use reinhardt_admin::pages::components::login::login_form;
use reinhardt_admin::pages::components::relation_selector::relation_selector;
use reinhardt_admin::types::{
	AdminAction, AdminActionRequest, FieldInfo, FieldType, Fieldset, FormFieldSpec, InlineFormInfo,
	InlineRowInfo, InlineStyle, ModelInfo, ModelPermission, MutationResponse, RelationOption,
	RelationSelectorLayout, RelationWidget,
};
use reinhardt_pages::component::{PageExt, cleanup_reactive_nodes};
use reinhardt_pages::dom::Element;
use reinhardt_pages::prelude::{defer_yield, use_action};
use reinhardt_pages::reactive::ReactiveScope;
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::{Action, Signal};
use reinhardt_test::wasm::{UserEvent, wait_for};
use rstest::rstest;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct WasmInlineTarget;

impl reinhardt_core::model_form::ModelFormTableName for WasmInlineTarget {
	fn table_name() -> &'static str {
		"line_items"
	}
}

#[wasm_bindgen_test]
fn inline_model_admin_wasm_api_matches_p1_surface() {
	let inline = reinhardt_admin::types::InlineModelAdmin::new::<(), WasmInlineTarget>(
		"Line Item",
		"parent_id",
		&["name"],
	)
	.expect("WASM inline metadata should construct");

	assert_eq!(inline.key(), "line_items-parent_id");
	assert_eq!(inline.child_model(), "Line Item");
	assert_eq!(inline.foreign_key(), "parent_id");
	assert_eq!(inline.fields(), &["name".to_owned()]);
}

struct BodyRoot {
	element: web_sys::Element,
}

impl BodyRoot {
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

impl Drop for BodyRoot {
	fn drop(&mut self) {
		cleanup_reactive_nodes();
		self.element.remove();
	}
}

struct ConfirmStubGuard {
	window: web_sys::Window,
	previous_confirm: JsValue,
	probe: js_sys::Object,
}

impl ConfirmStubGuard {
	fn install(result: bool) -> Self {
		let window = web_sys::window().expect("browser window");
		let previous_confirm = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("confirm"))
			.expect("window.confirm must be readable");
		let probe = js_sys::Object::new();
		js_sys::Reflect::set(&probe, &JsValue::from_str("calls"), &JsValue::from_f64(0.0))
			.expect("probe calls property");
		js_sys::Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtAdminConfirmProbe"),
			&probe,
		)
		.expect("install confirm probe");
		let script = format!(
			"const probe = globalThis.__reinhardtAdminConfirmProbe; \
			 probe.calls += 1; probe.message = message; return {result};"
		);
		let stub = js_sys::Function::new_with_args("message", &script);
		js_sys::Reflect::set(
			window.as_ref(),
			&JsValue::from_str("confirm"),
			stub.as_ref(),
		)
		.expect("install confirm stub");

		Self {
			window,
			previous_confirm,
			probe,
		}
	}

	fn calls(&self) -> u32 {
		js_sys::Reflect::get(&self.probe, &JsValue::from_str("calls"))
			.expect("probe calls must be readable")
			.as_f64()
			.unwrap_or_default() as u32
	}

	fn message(&self) -> String {
		js_sys::Reflect::get(&self.probe, &JsValue::from_str("message"))
			.expect("probe message must be readable")
			.as_string()
			.unwrap_or_default()
	}
}

impl Drop for ConfirmStubGuard {
	fn drop(&mut self) {
		let _ = js_sys::Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("confirm"),
			&self.previous_confirm,
		);
		let _ = js_sys::Reflect::delete_property(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtAdminConfirmProbe"),
		);
	}
}

fn action_list_data() -> ListViewData {
	ListViewData {
		model_name: "Article".to_string(),
		columns: vec![Column {
			field: "title".to_string(),
			label: "Title".to_string(),
			sortable: true,
		}],
		records: vec![
			HashMap::from([
				("id".to_string(), "41".to_string()),
				("slug".to_string(), "article-a".to_string()),
				("title".to_string(), "Article A".to_string()),
			]),
			HashMap::from([
				("id".to_string(), "42".to_string()),
				("slug".to_string(), "article-b".to_string()),
				("title".to_string(), "Article B".to_string()),
			]),
		],
		current_page: 1,
		total_pages: 1,
		total_count: 2,
		filters: Vec::new(),
	}
}

fn action_metadata() -> Vec<AdminAction> {
	vec![
		AdminAction::new(
			"publish",
			"Publish selected",
			ModelPermission::Change,
			false,
		),
		AdminAction::new("archive", "Archive selected", ModelPermission::Delete, true),
	]
}

fn mount_action_list(
	root: &BodyRoot,
	data: &ListViewData,
	actions: &[AdminAction],
	selected_ids: Signal<BTreeSet<String>>,
	selected_action: Signal<String>,
	action: Action<MutationResponse, String>,
) {
	list_view_with_actions(
		data,
		"slug",
		actions,
		Signal::new(1),
		Signal::new(HashMap::new()),
		(selected_ids, selected_action, action),
	)
	.mount(&Element::new(root.element.clone()))
	.expect("action list mounts");
}

fn input_by_label(root: &BodyRoot, label: &str) -> web_sys::HtmlInputElement {
	root.element
		.query_selector(&format!("input[aria-label='{label}']"))
		.expect("query input")
		.expect("input exists")
		.dyn_into::<web_sys::HtmlInputElement>()
		.expect("input element")
}

fn action_select(root: &BodyRoot) -> web_sys::HtmlSelectElement {
	root.element
		.query_selector("select[aria-label='Admin action']")
		.expect("query action select")
		.expect("action select exists")
		.dyn_into::<web_sys::HtmlSelectElement>()
		.expect("select element")
}

fn run_action_button(root: &BodyRoot) -> web_sys::HtmlButtonElement {
	root.element
		.query_selector("button.admin-btn-primary")
		.expect("query run button")
		.expect("run button exists")
		.dyn_into::<web_sys::HtmlButtonElement>()
		.expect("button element")
}

fn change_input(input: &web_sys::HtmlInputElement, checked: bool) {
	input.set_checked(checked);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change event"))
		.expect("input change dispatches");
}

fn change_action(select: &web_sys::HtmlSelectElement, value: &str) {
	select.set_value(value);
	select
		.dispatch_event(&web_sys::Event::new("change").expect("change event"))
		.expect("select change dispatches");
}

fn click_run(button: &web_sys::HtmlButtonElement) {
	button
		.dispatch_event(&web_sys::MouseEvent::new("click").expect("click event"))
		.expect("button click dispatches");
}

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

#[rstest]
#[wasm_bindgen_test]
fn model_form_with_fieldsets_expands_collapsed_required_group() {
	// Arrange
	let fields = vec![FormField {
		name: "title".to_string(),
		label: "Title".to_string(),
		spec: FormFieldSpec::Input {
			html_type: "text".to_string(),
		},
		required: true,
		value: String::new(),
	}];
	let fieldsets = vec![Fieldset::new(Some("Required"), &["title"]).collapsed()];

	// Act
	let html = model_form_with_fieldsets("Article", &fields, &fieldsets, None).render_to_string();

	// Assert
	let details_start = html
		.find("<details")
		.expect("fieldset should render details");
	let details_end = html[details_start..]
		.find('>')
		.map(|offset| details_start + offset)
		.expect("details opening tag should close");
	assert!(html[details_start..details_end].contains(" open"));
}

#[rstest]
#[case("")]
#[case(" \t")]
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
	let extra_decimal: web_sys::HtmlInputElement = root
		.element
		.query_selector("#inline-field-comments-post_id-1-decimal_number")
		.expect("query extra decimal")
		.expect("extra decimal exists")
		.unchecked_into();
	extra_decimal.set_value("0.1");
	assert!(extra_decimal.check_validity());
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
		r#""__reinhardt_inlines.comments-post_id.1.decimal_number":"0.1""#,
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
fn relation_raw_id_preserves_the_named_value_and_describes_the_resolved_label() {
	let fields = vec![FormField {
		name: "author_id".to_string(),
		label: "Author".to_string(),
		spec: FormFieldSpec::Relation {
			field_name: "author".to_string(),
			widget: RelationWidget::RawId,
			selected: Some(RelationOption {
				id: "7".to_string(),
				label: "Ada Lovelace".to_string(),
			}),
			readonly: false,
		},
		required: true,
		value: "7".to_string(),
	}];

	let scope = ReactiveScope::new();
	let html = scope.enter(|| model_form("Post", &fields, Some("42")).render_to_string());

	assert_eq!(html.matches("name=\"author_id\"").count(), 1, "got: {html}");
	assert_eq!(
		html.matches("data-relation-id=\"true\"").count(),
		1,
		"got: {html}"
	);
	assert_eq!(html.matches("value=\"7\"").count(), 1, "got: {html}");
	assert_eq!(html.matches("Ada Lovelace").count(), 1, "got: {html}");
	assert_eq!(
		html.matches("aria-describedby=\"field-author_id-status\"")
			.count(),
		1,
		"got: {html}"
	);
	assert_eq!(
		html.matches("id=\"field-author_id-status\"").count(),
		1,
		"got: {html}"
	);
}

#[wasm_bindgen_test]
fn relation_autocomplete_uses_a_search_control_and_a_hidden_submitted_id() {
	let fields = vec![FormField {
		name: "author_id".to_string(),
		label: "Author".to_string(),
		spec: FormFieldSpec::Relation {
			field_name: "author".to_string(),
			widget: RelationWidget::Autocomplete,
			selected: Some(RelationOption {
				id: "7".to_string(),
				label: "Ada Lovelace".to_string(),
			}),
			readonly: false,
		},
		required: true,
		value: "7".to_string(),
	}];

	let scope = ReactiveScope::new();
	let html = scope.enter(|| model_form("Post", &fields, Some("42")).render_to_string());

	assert_eq!(
		html.matches("for=\"field-author_id-search\"").count(),
		1,
		"got: {html}"
	);
	assert_eq!(html.matches("type=\"search\"").count(), 1, "got: {html}");
	assert_eq!(html.matches("role=\"combobox\"").count(), 1, "got: {html}");
	assert_eq!(html.matches("role=\"listbox\"").count(), 1, "got: {html}");
	assert_eq!(html.matches("Loading…").count(), 1, "got: {html}");
	assert_eq!(html.matches("Previous").count(), 1, "got: {html}");
	assert_eq!(html.matches("Next").count(), 1, "got: {html}");
	assert_eq!(html.matches("type=\"hidden\"").count(), 1, "got: {html}");
	assert_eq!(html.matches("name=\"author_id\"").count(), 1, "got: {html}");
	assert_eq!(
		html.matches("data-relation-id=\"true\"").count(),
		1,
		"got: {html}"
	);
	assert_eq!(html.matches("value=\"7\"").count(), 1, "got: {html}");
}

#[wasm_bindgen_test]
fn readonly_relation_renders_without_a_submitted_control() {
	let fields = vec![FormField {
		name: "author_id".to_string(),
		label: "Author".to_string(),
		spec: FormFieldSpec::Relation {
			field_name: "author".to_string(),
			widget: RelationWidget::Autocomplete,
			selected: Some(RelationOption {
				id: "7".to_string(),
				label: "Ada Lovelace".to_string(),
			}),
			readonly: true,
		},
		required: true,
		value: "7".to_string(),
	}];

	let html = model_form("Post", &fields, Some("42")).render_to_string();

	assert_eq!(
		html.matches("class=\"relation-readonly\"").count(),
		1,
		"got: {html}"
	);
	assert_eq!(
		html.matches("data-relation-id=\"true\"").count(),
		0,
		"got: {html}"
	);
	assert_eq!(html.matches("name=\"author_id\"").count(), 0, "got: {html}");
	assert_eq!(html.matches("Ada Lovelace").count(), 1, "got: {html}");
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
	record.insert("id".to_string(), serde_json::json!(1));
	record.insert("name".to_string(), serde_json::json!("Alice"));

	let data = ListViewData {
		model_name: "User".to_string(),
		columns: vec![
			Column {
				field: "id".to_string(),
				label: "ID".to_string(),
				sortable: true,
				editable: false,
				linked: true,
				required: true,
				form_spec: None,
			},
			Column {
				field: "name".to_string(),
				label: "Name".to_string(),
				sortable: true,
				editable: false,
				linked: false,
				required: false,
				form_spec: None,
			},
		],
		pk_field: "id".to_string(),
		records: vec![record],
		current_page: 1,
		total_pages: 3,
		total_count: 25,
		filters: vec![],
	};

	let scope = ReactiveScope::new();
	let html = scope.enter(|| {
		let page_signal = Signal::new(1u64);
		let filters_signal = Signal::new(HashMap::new());
		list_view(&data, page_signal, filters_signal).render_to_string()
	});
	scope.dispose();

	assert!(html.contains("User List"), "Should show list title");
	assert!(html.contains("Alice"), "Should show record data");
	assert!(html.contains("ID"), "Should show column header");
	assert!(html.contains("Name"), "Should show column header");
	assert!(html.contains("Showing 25 User"), "Should show record count");
	assert!(
		!html.contains("Select current page") && !html.contains("Admin action"),
		"Lists without configured actions should preserve the existing controls"
	);
}

#[wasm_bindgen_test]
fn admin_actions_select_current_page_then_toggle_one_configured_primary_key() {
	// Arrange
	let root = BodyRoot::new("admin-action-selection");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let (selected_ids, _selected_action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::new());
		let selected_action = Signal::new(String::new());
		let action = use_action(|_: AdminActionRequest| async {
			Ok::<MutationResponse, String>(MutationResponse {
				success: true,
				message: "done".to_string(),
				affected: Some(0),
				data: None,
			})
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action)
	});

	// Act
	change_input(&input_by_label(&root, "Select current page"), true);
	change_input(&input_by_label(&root, "Select article-a"), false);

	// Assert
	assert_eq!(
		selected_ids.get(),
		BTreeSet::from(["article-b".to_string()])
	);
	assert!(!selected_ids.get().contains("41"));
	scope.dispose();
}

#[wasm_bindgen_test]
async fn admin_action_confirmation_uses_selected_metadata_and_cancel_retains_selection() {
	// Arrange
	let confirm = ConfirmStubGuard::install(false);
	let root = BodyRoot::new("admin-action-confirmation");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let invocations = Rc::new(Cell::new(0));
	let invocations_for_action = Rc::clone(&invocations);
	let (selected_ids, _selected_action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::new());
		let selected_action = Signal::new(String::new());
		let action = use_action(move |_: AdminActionRequest| {
			invocations_for_action.set(invocations_for_action.get() + 1);
			async {
				Ok::<MutationResponse, String>(MutationResponse {
					success: true,
					message: "done".to_string(),
					affected: Some(1),
					data: None,
				})
			}
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action)
	});
	change_input(&input_by_label(&root, "Select article-a"), true);
	let select = action_select(&root);
	let button = run_action_button(&root);

	// Act: an action without confirmation dispatches immediately.
	change_action(&select, "publish");
	click_run(&button);
	defer_yield().await;
	defer_yield().await;

	// Assert
	assert_eq!(invocations.get(), 1);
	assert_eq!(confirm.calls(), 0);

	// Act: cancelling the action that requires confirmation prevents dispatch.
	change_action(&select, "archive");
	click_run(&button);

	// Assert
	assert_eq!(invocations.get(), 1);
	assert_eq!(confirm.calls(), 1);
	assert_eq!(
		confirm.message(),
		"Run \"Archive selected\" on 1 selected records?"
	);
	assert_eq!(
		selected_ids.get(),
		BTreeSet::from(["article-a".to_string()])
	);
	scope.dispose();
}

#[wasm_bindgen_test]
async fn confirmed_admin_action_dispatches_the_exact_action_and_ids() {
	// Arrange
	let confirm = ConfirmStubGuard::install(true);
	let root = BodyRoot::new("admin-action-confirmed");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let request = Rc::new(RefCell::new(None));
	let request_for_action = Rc::clone(&request);
	let (_selected_ids, _selected_action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::from(["article-b".to_string()]));
		let selected_action = Signal::new("archive".to_string());
		let action = use_action(move |action_request: AdminActionRequest| {
			*request_for_action.borrow_mut() = Some(action_request);
			async {
				Ok::<MutationResponse, String>(MutationResponse {
					success: true,
					message: "archived".to_string(),
					affected: Some(1),
					data: None,
				})
			}
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action)
	});

	// Act
	click_run(&run_action_button(&root));
	defer_yield().await;
	defer_yield().await;

	// Assert
	assert_eq!(confirm.calls(), 1);
	assert_eq!(
		confirm.message(),
		"Run \"Archive selected\" on 1 selected records?"
	);
	let request = request
		.borrow()
		.clone()
		.expect("confirmed action must dispatch a request");
	assert_eq!(request.action, "archive");
	assert_eq!(request.ids, vec!["article-b".to_string()]);
	scope.dispose();
}

#[wasm_bindgen_test]
async fn pending_admin_action_disables_and_guards_every_bulk_control() {
	// Arrange
	let root = BodyRoot::new("admin-action-pending");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let invocations = Rc::new(Cell::new(0));
	let invocations_for_action = Rc::clone(&invocations);
	let (selected_ids, selected_action, action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::from(["article-a".to_string()]));
		let selected_action = Signal::new("publish".to_string());
		let action = use_action(move |_: AdminActionRequest| {
			invocations_for_action.set(invocations_for_action.get() + 1);
			async { std::future::pending::<Result<MutationResponse, String>>().await }
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action, action)
	});
	let select = action_select(&root);
	let button = run_action_button(&root);
	let page_checkbox = input_by_label(&root, "Select current page");
	let row_checkbox = input_by_label(&root, "Select article-a");

	// Act
	click_run(&button);
	defer_yield().await;
	reinhardt_pages::reactive::runtime::with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(invocations.get(), 1);
	assert!(action.is_pending());
	assert!(button.disabled());
	assert!(select.disabled());
	assert!(page_checkbox.disabled());
	assert!(row_checkbox.disabled());

	// Act: programmatic events still reach handlers, which must guard pending work.
	change_action(&select, "archive");
	change_input(&page_checkbox, true);
	change_input(&row_checkbox, false);
	click_run(&button);
	defer_yield().await;

	// Assert
	assert_eq!(invocations.get(), 1);
	assert_eq!(selected_action.get(), "publish");
	assert_eq!(
		selected_ids.get(),
		BTreeSet::from(["article-a".to_string()])
	);
	scope.dispose();
}

#[wasm_bindgen_test]
async fn successful_admin_action_clears_selection_and_runs_refresh_callback() {
	// Arrange
	let root = BodyRoot::new("admin-action-success");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let refreshes = Rc::new(Cell::new(0));
	let refreshes_for_success = Rc::clone(&refreshes);
	let (selected_ids, _selected_action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::from(["article-a".to_string()]));
		let selected_action = Signal::new("publish".to_string());
		let action = use_action(|_: AdminActionRequest| async {
			Ok::<MutationResponse, String>(MutationResponse {
				success: true,
				message: "published".to_string(),
				affected: Some(1),
				data: None,
			})
		})
		.on_success(move |_| {
			selected_ids.set(BTreeSet::new());
			refreshes_for_success.set(refreshes_for_success.get() + 1);
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action)
	});

	// Act
	click_run(&run_action_button(&root));
	defer_yield().await;
	defer_yield().await;

	// Assert
	assert!(selected_ids.get().is_empty());
	assert_eq!(refreshes.get(), 1);
	scope.dispose();
}

#[wasm_bindgen_test]
async fn failed_admin_action_retains_selection_and_renders_error() {
	// Arrange
	let root = BodyRoot::new("admin-action-failure");
	let scope = ReactiveScope::new();
	let data = action_list_data();
	let actions = action_metadata();
	let (selected_ids, _selected_action) = scope.enter(|| {
		let selected_ids = Signal::new(BTreeSet::from(["article-a".to_string()]));
		let selected_action = Signal::new("publish".to_string());
		let action = use_action(|_: AdminActionRequest| async {
			Err::<MutationResponse, String>("publish failed".to_string())
		});
		mount_action_list(
			&root,
			&data,
			&actions,
			selected_ids,
			selected_action,
			action,
		);
		(selected_ids, selected_action)
	});

	// Act
	click_run(&run_action_button(&root));
	defer_yield().await;
	defer_yield().await;

	// Assert
	assert_eq!(
		selected_ids.get(),
		BTreeSet::from(["article-a".to_string()])
	);
	let alert = root
		.element
		.query_selector("[role='alert']")
		.expect("query alert")
		.expect("failure alert exists");
	assert_eq!(alert.text_content().as_deref(), Some("publish failed"));
	scope.dispose();
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

fn relation_field(layout: RelationSelectorLayout) -> FormField {
	FormField {
		name: "tags".to_string(),
		label: "Tags".to_string(),
		spec: FormFieldSpec::ManyToManySelector {
			layout,
			available: vec![
				RelationOption::new("1", "Rust"),
				RelationOption::new("2", "WebAssembly"),
			],
			selected: vec![
				RelationOption::new("2", "WebAssembly"),
				RelationOption::new("3", "Serde"),
			],
			page: 1,
			has_more: true,
		},
		required: false,
		value: String::new(),
	}
}

#[wasm_bindgen_test]
fn relation_selector_renders_accessible_horizontal_and_vertical_structure() {
	ReactiveScope::run(|| {
		let horizontal = model_form(
			"Article",
			&[relation_field(RelationSelectorLayout::Horizontal)],
			None,
		)
		.render_to_string();
		let vertical = model_form(
			"Article",
			&[relation_field(RelationSelectorLayout::Vertical)],
			None,
		)
		.render_to_string();

		for html in [&horizontal, &vertical] {
			assert!(html.contains("<fieldset"));
			assert!(html.contains("<legend") && html.contains("Tags"));
			assert!(html.contains(">Search<"));
			assert!(html.contains(">Available<"));
			assert!(html.contains(">Chosen<"));
			assert_eq!(html.matches("<select").count(), 3);
			assert!(html.contains(r#"type="button"#));
			assert!(html.contains(">Add<") && html.contains(">Remove<"));
			assert!(html.contains("data-relation-action=\"load-more\""));
			assert!(html.contains("aria-controls="));
			assert!(html.contains("aria-label=\"Load more relation options after page 1\""));
			assert!(html.contains(r#"aria-live="polite"#));
			assert!(html.contains(r#"hidden="hidden"#));
			assert_eq!(html.matches(r#"name="tags"#).count(), 1);
			assert_eq!(html.matches(r#"value="2"#).count(), 1);
			assert_eq!(html.matches(r#"value="3"#).count(), 1);
		}
		assert!(horizontal.contains("relation-selector--horizontal"));
		assert!(!horizontal.contains("relation-selector--vertical"));
		assert!(vertical.contains("relation-selector--vertical"));
		assert!(!vertical.contains("relation-selector--horizontal"));
	});
}

struct MountedSelector {
	root: Element,
}

impl MountedSelector {
	fn new() -> Self {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("form").expect("root"));
		document
			.body()
			.expect("body")
			.append_child(root.as_web_sys())
			.expect("append root");
		relation_selector(
			"Article",
			"tags",
			"Tags",
			RelationSelectorLayout::Horizontal,
			vec![
				RelationOption::new("1", "Rust"),
				RelationOption::new("2", "WebAssembly"),
			],
			vec![RelationOption::new("3", "Serde")],
			1,
			true,
		)
		.mount(&root)
		.expect("mount selector");
		Self { root }
	}

	fn select(&self, id: &str) -> web_sys::HtmlSelectElement {
		self.root
			.as_web_sys()
			.query_selector(id)
			.expect("selector query")
			.expect("select")
			.unchecked_into()
	}

	fn button(&self, action: &str) -> web_sys::HtmlButtonElement {
		self.root
			.as_web_sys()
			.query_selector(&format!(r#"[data-relation-action="{action}"]"#))
			.expect("button query")
			.expect("button")
			.unchecked_into()
	}
}

impl Drop for MountedSelector {
	fn drop(&mut self) {
		self.root.as_web_sys().remove();
		cleanup_reactive_nodes();
	}
}

fn choose(select: &web_sys::HtmlSelectElement, value: &str) {
	for index in 0..select.options().length() {
		let option: web_sys::HtmlOptionElement = select
			.options()
			.item(index)
			.expect("option")
			.unchecked_into();
		option.set_selected(option.value() == value);
	}
	select
		.dispatch_event(&web_sys::Event::new("change").expect("change event"))
		.expect("dispatch change");
}

fn press(select: &web_sys::HtmlSelectElement, key: &str) {
	let init = web_sys::KeyboardEventInit::new();
	init.set_key(key);
	select
		.dispatch_event(
			&web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init)
				.expect("keydown event"),
		)
		.expect("dispatch keydown");
}

#[wasm_bindgen_test]
fn relation_selector_add_remove_keyboard_and_focus_preserve_named_selection() {
	ReactiveScope::run(|| {
		let fixture = MountedSelector::new();
		let available = fixture.select(r#"select[id$="-available"]"#);

		choose(&available, "1");
		press(&available, "Enter");

		let chosen = fixture.select(r#"select[id$="-chosen"]"#);
		let hidden = fixture.select(r#"select[name="tags"]"#);
		assert_eq!(hidden.options().length(), 2);
		assert_eq!(hidden.selected_options().length(), 2);
		assert_eq!(
			web_sys::window()
				.expect("window")
				.document()
				.expect("document")
				.active_element()
				.expect("active element")
				.id(),
			chosen.id()
		);

		choose(&chosen, "1");
		press(&chosen, "Backspace");
		let hidden = fixture.select(r#"select[name="tags"]"#);
		assert_eq!(hidden.options().length(), 1);
		assert_eq!(hidden.value(), "3");
		assert_eq!(
			web_sys::window()
				.expect("window")
				.document()
				.expect("document")
				.active_element()
				.expect("active element")
				.id(),
			available.id()
		);

		choose(&available, "2");
		fixture.button("add").click();
		let chosen = fixture.select(r#"select[id$="-chosen"]"#);
		choose(&chosen, "2");
		press(&chosen, "Delete");
		let hidden = fixture.select(r#"select[name="tags"]"#);
		assert_eq!(hidden.options().length(), 1);
		assert_eq!(hidden.value(), "3");
	});
}

#[wasm_bindgen_test]
fn relation_selector_instances_have_distinct_ids_and_local_focus() {
	ReactiveScope::run(|| {
		let first = MountedSelector::new();
		let second = MountedSelector::new();
		let first_available = first.select(r#"select[id$="-available"]"#);
		let first_chosen = first.select(r#"select[id$="-chosen"]"#);
		let second_available = second.select(r#"select[id$="-available"]"#);
		let second_chosen = second.select(r#"select[id$="-chosen"]"#);

		assert_ne!(first_available.id(), second_available.id());
		assert_ne!(first_chosen.id(), second_chosen.id());

		choose(&first_available, "1");
		press(&first_available, "Enter");

		assert_eq!(
			web_sys::window()
				.expect("window")
				.document()
				.expect("document")
				.active_element()
				.expect("active element")
				.id(),
			first_chosen.id()
		);
		assert_eq!(first.select(r#"select[name="tags"]"#).options().length(), 2);
		assert_eq!(
			second.select(r#"select[name="tags"]"#).options().length(),
			1
		);
	});
}

#[wasm_bindgen_test]
fn relation_selector_hidden_select_serializes_all_chosen_values() {
	ReactiveScope::run(|| {
		let fixture = MountedSelector::new();
		let available = fixture.select(r#"select[id$="-available"]"#);
		let chosen = fixture.select(r#"select[id$="-chosen"]"#);
		let hidden = fixture.select(r#"select[name="tags"]"#);

		assert_eq!(available.name(), "");
		assert_eq!(chosen.name(), "");
		assert_eq!(hidden.name(), "tags");
		assert!(hidden.has_attribute("hidden"));
		assert!(hidden.multiple());

		choose(&available, "1");
		press(&available, "Enter");
		let hidden = fixture.select(r#"select[name="tags"]"#);
		assert_eq!(hidden.options().length(), 2);
		assert_eq!(hidden.selected_options().length(), 2);
		for index in 0..hidden.options().length() {
			let option: web_sys::HtmlOptionElement = hidden
				.options()
				.item(index)
				.expect("submitted option")
				.unchecked_into();
			assert!(option.selected());
		}

		let form: web_sys::HtmlFormElement = fixture.root.as_web_sys().clone().unchecked_into();
		let serialized = web_sys::FormData::new_with_form(&form)
			.expect("form data")
			.get_all("tags");

		assert_eq!(serialized.length(), 2);
		assert_eq!(serialized.get(0).as_string().as_deref(), Some("3"));
		assert_eq!(serialized.get(1).as_string().as_deref(), Some("1"));
	});
}
