#![cfg(wasm)]

include!("ui/form/model_multipart_support.rs");

use std::cell::Cell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Reflect};
use reinhardt_macros::model;
use reinhardt_pages::component::{PageExt, cleanup_reactive_nodes};
use reinhardt_pages::dom::Element;
use reinhardt_pages::prelude::defer_yield;
use reinhardt_pages::reactive::ReactiveScope;
use reinhardt_pages::{FieldError, MutationDispatchOutcome, form, use_form};
use rstest::rstest;
use serial_test::serial;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[model(
	app_label = "pages",
	table_name = "nullable_default_true_records",
	form(name = NullableDefaultTrueForm, fields(published)),
	info = false
)]
struct NullableDefaultTrueRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(default = true, null = true)]
	published: Option<bool>,
}

#[server_fn(model_form = true)]
async fn save_nullable_default_true(
	payload: NullableDefaultTrueFormData,
) -> Result<(), ServerFnError> {
	let _ = payload;
	Ok(())
}

#[model(
	app_label = "pages",
	table_name = "normalized_mutation_records",
	form(name = NormalizedMutationForm, fields(title, published)),
	info = false
)]
struct NormalizedMutationRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(min_length = 1, max_length = 120)]
	#[form(trim)]
	title: String,
	#[field(default = true)]
	published: bool,
}

#[server_fn(model_form = true)]
async fn save_normalized_mutation(
	payload: NormalizedMutationFormData,
) -> Result<(), ServerFnError> {
	let _ = payload;
	Ok(())
}

struct BodyRoot(web_sys::Element);

impl BodyRoot {
	fn new() -> Self {
		let document = web_sys::window()
			.expect("browser window")
			.document()
			.expect("browser document");
		let root = document.create_element("div").expect("create test root");
		document
			.body()
			.expect("browser body")
			.append_child(&root)
			.expect("mount test root");
		Self(root)
	}
}

impl Drop for BodyRoot {
	fn drop(&mut self) {
		cleanup_reactive_nodes();
		self.0.remove();
	}
}

struct SuccessfulFetchGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
}

impl SuccessfulFetchGuard {
	fn install() -> Self {
		let window = web_sys::window().expect("browser window");
		let previous_fetch =
			Reflect::get(window.as_ref(), &JsValue::from_str("fetch")).expect("read browser fetch");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelFormJsonPayload"),
			&JsValue::NULL,
		)
		.expect("clear captured JSON payload");
		let stub = Function::new_with_args(
			"request",
			r#"
				return request.text().then((body) => {
					globalThis.__reinhardtModelFormJsonPayload = body;
					return new Response('null', { status: 200 });
				});
			"#,
		);
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
			.expect("install successful fetch stub");
		Self {
			window,
			previous_fetch,
		}
	}

	fn payload(&self) -> serde_json::Value {
		let body = Reflect::get(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelFormJsonPayload"),
		)
		.expect("read captured JSON payload")
		.as_string()
		.expect("request body was captured");
		serde_json::from_str(&body).expect("request body is JSON")
	}
}

impl Drop for SuccessfulFetchGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
		let _ = Reflect::delete_property(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelFormJsonPayload"),
		);
	}
}

struct MultipartFetchGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
	form_data_prototype: JsValue,
	previous_form_data_append: JsValue,
}

impl MultipartFetchGuard {
	fn install(document: &web_sys::File, avatar: &web_sys::File) -> Self {
		let window = web_sys::window().expect("browser window");
		let previous_fetch =
			Reflect::get(window.as_ref(), &JsValue::from_str("fetch")).expect("read browser fetch");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadDocument"),
			document.as_ref(),
		)
		.expect("install expected document");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadAvatar"),
			avatar.as_ref(),
		)
		.expect("install expected avatar");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadStatus"),
			&JsValue::from_f64(500.0),
		)
		.expect("install failing response status");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadRequests"),
			&JsValue::from_f64(0.0),
		)
		.expect("install request counter");
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadPayloadError"),
			&JsValue::NULL,
		)
		.expect("clear payload error");
		let form_data_constructor =
			Reflect::get(js_sys::global().as_ref(), &JsValue::from_str("FormData"))
				.expect("FormData constructor");
		let form_data_prototype =
			Reflect::get(&form_data_constructor, &JsValue::from_str("prototype"))
				.expect("FormData prototype");
		let previous_form_data_append =
			Reflect::get(&form_data_prototype, &JsValue::from_str("append"))
				.expect("FormData.append");
		let append_spy = Function::new_with_args(
			"originalAppend",
			r#"
				return function(...args) {
					const result = originalAppend.apply(this, args);
					const name = args[0];
					const expected = name === 'document'
						? globalThis.__reinhardtModelUploadDocument
						: name === 'avatar'
							? globalThis.__reinhardtModelUploadAvatar
							: null;
					if (expected !== null && this.get(name) !== expected) {
						globalThis.__reinhardtModelUploadPayloadError = `${name} File identity was not preserved`;
					}
					return result;
				};
			"#,
		)
		.call1(&JsValue::NULL, &previous_form_data_append)
		.expect("create FormData append spy");
		Reflect::set(
			&form_data_prototype,
			&JsValue::from_str("append"),
			&append_spy,
		)
		.expect("install FormData append spy");
		let stub = Function::new_with_args(
			"request",
			r#"
				globalThis.__reinhardtModelUploadRequests += 1;
				return request.formData().then((formData) => {
					let payloadError = globalThis.__reinhardtModelUploadPayloadError;
					if (formData.get('title') !== '"Report"') payloadError = 'title was not JSON encoded';
					if (!formData.has('document')) payloadError = 'document File was omitted';
					const expectedAvatar = globalThis.__reinhardtModelUploadAvatar;
					if (expectedAvatar === null) {
						if (formData.has('avatar')) payloadError = 'empty optional image was not omitted';
					} else if (!formData.has('avatar')) payloadError = 'avatar File was omitted';
					globalThis.__reinhardtModelUploadPayloadError = payloadError;
					const status = payloadError === null
						? globalThis.__reinhardtModelUploadStatus
						: 500;
					const body = status < 300
						? 'null'
						: '{"version":1,"kind":"server","status":500,"message":"upload failed","field_errors":[]}';
					return new Response(body, { status });
				});
			"#,
		);
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
			.expect("install multipart fetch stub");
		Self {
			window,
			previous_fetch,
			form_data_prototype,
			previous_form_data_append,
		}
	}

	fn requests(&self) -> u32 {
		Reflect::get(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadRequests"),
		)
		.expect("read request counter")
		.as_f64()
		.expect("request counter is numeric") as u32
	}

	fn payload_error(&self) -> Option<String> {
		Reflect::get(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadPayloadError"),
		)
		.expect("read payload error")
		.as_string()
	}

	fn set_status(&self, status: f64) {
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadStatus"),
			&JsValue::from_f64(status),
		)
		.expect("set fetch response status");
	}

	fn set_expected_avatar(&self, avatar: Option<&web_sys::File>) {
		let value = avatar.map_or(JsValue::NULL, |file| JsValue::from(file.clone()));
		Reflect::set(
			js_sys::global().as_ref(),
			&JsValue::from_str("__reinhardtModelUploadAvatar"),
			&value,
		)
		.expect("set expected avatar");
	}
}

impl Drop for MultipartFetchGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
		let _ = Reflect::set(
			&self.form_data_prototype,
			&JsValue::from_str("append"),
			&self.previous_form_data_append,
		);
		for key in [
			"__reinhardtModelUploadDocument",
			"__reinhardtModelUploadAvatar",
			"__reinhardtModelUploadStatus",
			"__reinhardtModelUploadRequests",
			"__reinhardtModelUploadPayloadError",
		] {
			let _ = Reflect::delete_property(js_sys::global().as_ref(), &JsValue::from_str(key));
		}
	}
}

fn browser_file(name: &str) -> web_sys::File {
	Function::new_with_args(
		"name",
		"return new File(['content'], name, { type: 'text/plain' });",
	)
	.call1(&JsValue::NULL, &JsValue::from_str(name))
	.expect("create browser file")
	.dyn_into::<web_sys::File>()
	.expect("created value is a File")
}

fn select_file(input: &web_sys::HtmlInputElement, file: &web_sys::File) {
	let transfer = web_sys::DataTransfer::new().expect("create data transfer");
	let items =
		Reflect::get(transfer.as_ref(), &JsValue::from_str("items")).expect("data transfer items");
	Reflect::get(&items, &JsValue::from_str("add"))
		.expect("data transfer add")
		.dyn_into::<Function>()
		.expect("data transfer add is callable")
		.call1(&items, file.as_ref())
		.expect("add selected file");
	input.set_files(transfer.files().as_ref());
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change event"))
		.expect("dispatch file change");
}

fn clear_selected_file(input: &web_sys::HtmlInputElement) {
	input.set_value("");
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change event"))
		.expect("dispatch empty file change");
}

fn query_form(root: &web_sys::Element) -> web_sys::HtmlFormElement {
	root.query_selector("form")
		.expect("query form")
		.expect("model form exists")
		.dyn_into::<web_sys::HtmlFormElement>()
		.expect("form element")
}

fn query_input(root: &web_sys::Element, id: &str) -> web_sys::HtmlInputElement {
	root.query_selector(&format!("#{id}"))
		.expect("query input")
		.expect("input exists")
		.dyn_into::<web_sys::HtmlInputElement>()
		.expect("input element")
}

fn file_count(input: &web_sys::HtmlInputElement) -> u32 {
	input.files().expect("file input list").length()
}

fn selected_file(input: &web_sys::HtmlInputElement) -> web_sys::File {
	input
		.files()
		.expect("file input list")
		.item(0)
		.expect("selected file")
}

fn submit(form: &web_sys::HtmlFormElement) {
	form.dispatch_event(&web_sys::SubmitEvent::new("submit").expect("submit event"))
		.expect("dispatch submit");
}

fn same_file(left: &web_sys::File, right: &web_sys::File) -> bool {
	JsValue::from(left.clone()) == JsValue::from(right.clone())
}

async fn wait_for_requests(fetch: &MultipartFetchGuard, expected: u32) {
	for _ in 0..8 {
		if fetch.requests() == expected {
			return;
		}
		TimeoutFuture::new(10).await;
	}
	assert_eq!(fetch.requests(), expected);
}

async fn wait_for_error(mut has_error: impl FnMut() -> bool) {
	for _ in 0..100 {
		if has_error() {
			return;
		}
		TimeoutFuture::new(10).await;
	}
	assert!(has_error());
}

async fn wait_for_files_to_clear(root: &web_sys::Element) {
	for _ in 0..100 {
		if file_count(&query_input(root, "upload-form-document")) == 0
			&& file_count(&query_input(root, "upload-form-avatar")) == 0
		{
			return;
		}
		TimeoutFuture::new(10).await;
	}
	assert_eq!(file_count(&query_input(root, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(root, "upload-form-avatar")), 0);
}

#[rstest]
#[case::automatic_submit(false)]
#[case::server_mutation(true)]
#[serial(model_form_file_upload_globals)]
#[test_attr(wasm_bindgen_test)]
async fn model_form_submit_routes_snapshot_errors_without_fetch(#[case] use_mutation: bool) {
	// Arrange
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	let scope = ReactiveScope::new();
	let (form, runtime) = scope.enter(|| {
		let form = form! {
			name: InvalidUploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		let runtime = use_form(&form).build();
		(form, runtime)
	});
	let title = query_input(&root.0, "invalid-upload-form-title");
	let document = query_input(&root.0, "invalid-upload-form-document");
	title.set_value("Rejected by validation");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(&document, &document_file);
	defer_yield().await;

	// Act
	if use_mutation {
		let mutation = scope.enter(|| form.server_mutation(&runtime).build());
		assert_eq!(
			mutation.dispatch(),
			MutationDispatchOutcome::ValidationFailed
		);
		assert!(!mutation.is_pending());
	} else {
		submit(&query_form(&root.0));
	}
	wait_for_error(|| {
		runtime.get_field_state(form.title_field()).error.is_some()
			&& runtime.form_state().form_error.get().is_some()
	})
	.await;

	// Assert
	assert_eq!(fetch.requests(), 0);
	assert_eq!(
		runtime
			.get_field_state(form.title_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Title is rejected")
	);
	assert_eq!(
		runtime.form_state().form_error.get(),
		Some("Validation failed\n_all: Upload is rejected".to_owned())
	);
	assert_eq!(title.value(), "Rejected by validation");
	assert!(same_file(&selected_file(&document), &document_file));
}

#[rstest]
#[serial(model_form_file_upload_globals)]
#[test_attr(wasm_bindgen_test)]
async fn model_form_mutation_sends_normalized_snapshot_with_defaults() {
	// Arrange
	let fetch = SuccessfulFetchGuard::install();
	let scope = ReactiveScope::new();
	let (form, mutation) = scope.enter(|| {
		let form = form! {
			name: NormalizedMutationPageForm,
			model_form: NormalizedMutationForm,
			server_fn: save_normalized_mutation,
		};
		form.set_value("title", serde_json::json!("  Report  "))
			.expect("raw title is accepted");
		let runtime = use_form(&form).build();
		let mutation = form.server_mutation(&runtime).build();
		(form, mutation)
	});
	assert_eq!(form.value("published"), None);

	// Act
	assert_eq!(mutation.dispatch(), MutationDispatchOutcome::Dispatched);
	for _ in 0..100 {
		if mutation.is_success() {
			break;
		}
		TimeoutFuture::new(10).await;
	}

	// Assert
	assert!(
		mutation.is_success(),
		"mutation failed: {:?}",
		mutation.error()
	);
	assert_eq!(
		fetch.payload(),
		serde_json::json!({"payload": {"title": "Report", "published": true}})
	);
	assert_eq!(form.value("title"), Some(serde_json::json!("  Report  ")));
	assert_eq!(form.value("published"), None);
}

#[wasm_bindgen_test]
#[serial(model_form_file_upload_globals)]
async fn model_form_submit_clears_nullable_default_true_checkbox() {
	let root = BodyRoot::new();
	let _fetch = SuccessfulFetchGuard::install();
	let scope = ReactiveScope::new();
	let form = scope.enter(|| {
		let form = form! {
			name: NullableDefaultTrueCheckboxForm,
			model_form: NullableDefaultTrueForm,
			server_fn: save_nullable_default_true,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		form
	});

	let checkbox = root
		.0
		.query_selector("input[name='published']")
		.expect("query checkbox")
		.expect("published checkbox exists")
		.dyn_into::<web_sys::HtmlInputElement>()
		.expect("published control is an input");
	assert_eq!(checkbox.type_(), "checkbox");
	assert!(checkbox.checked());
	checkbox.set_checked(false);

	submit(&query_form(&root.0));
	let payload = form.data().expect("submitted form data should decode");
	assert_eq!(
		payload.get_json("published"),
		Some(serde_json::Value::Bool(false))
	);
	for _ in 0..100 {
		if form.success().get() {
			break;
		}
		TimeoutFuture::new(10).await;
	}
	assert!(form.success().get());
	scope.dispose();
}

#[wasm_bindgen_test]
#[serial(model_form_file_upload_globals)]
async fn model_form_files_clear_only_after_success_or_reset() {
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	let scope = ReactiveScope::new();
	let (form, runtime) = scope.enter(|| {
		let form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		let runtime = use_form(&form).build();
		(form, runtime)
	});

	let title = query_input(&root.0, "upload-form-title");
	let document = query_input(&root.0, "upload-form-document");
	let avatar = query_input(&root.0, "upload-form-avatar");
	assert_eq!(query_form(&root.0).enctype(), "multipart/form-data");
	assert_eq!(document.type_(), "file");
	assert_eq!(avatar.type_(), "file");
	assert_eq!(avatar.accept(), "image/*");
	title.set_value("Report");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(&document, &document_file);
	select_file(&avatar, &avatar_file);
	defer_yield().await;
	assert!(runtime.get_field_state(form.document_field()).is_dirty);
	assert!(runtime.get_field_state(form.avatar_field()).is_dirty);

	submit(&query_form(&root.0));
	wait_for_requests(&fetch, 1).await;
	wait_for_error(|| form.error().get().is_some()).await;
	assert_eq!(fetch.payload_error(), None);
	assert!(same_file(
		&selected_file(&query_input(&root.0, "upload-form-document")),
		&document_file
	));
	assert!(same_file(
		&selected_file(&query_input(&root.0, "upload-form-avatar")),
		&avatar_file
	));
	assert_eq!(query_input(&root.0, "upload-form-title").value(), "Report");

	fetch.set_status(200.0);
	submit(&query_form(&root.0));
	wait_for_requests(&fetch, 2).await;
	assert_eq!(fetch.payload_error(), None);
	wait_for_files_to_clear(&root.0).await;
	assert!(form.success().get());
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(&root.0, "upload-form-avatar")), 0);
	assert_eq!(query_input(&root.0, "upload-form-title").value(), "Report");

	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	select_file(&query_input(&root.0, "upload-form-avatar"), &avatar_file);
	clear_selected_file(&query_input(&root.0, "upload-form-document"));
	defer_yield().await;
	assert!(!runtime.get_field_state(form.document_field()).is_dirty);
	assert!(runtime.get_field_state(form.avatar_field()).is_dirty);
	submit(&query_form(&root.0));
	defer_yield().await;
	assert_eq!(fetch.requests(), 2);
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	assert!(same_file(
		&selected_file(&query_input(&root.0, "upload-form-avatar")),
		&avatar_file
	));

	query_form(&root.0).reset();
	defer_yield().await;
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(&root.0, "upload-form-avatar")), 0);

	let title = query_input(&root.0, "upload-form-title");
	title.set_value("Report");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	fetch.set_expected_avatar(None);
	submit(&query_form(&root.0));
	wait_for_requests(&fetch, 3).await;
	assert_eq!(fetch.payload_error(), None);
	wait_for_files_to_clear(&root.0).await;
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(&root.0, "upload-form-avatar")), 0);
	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	form.submit().await.expect("direct model-form submission");
	wait_for_requests(&fetch, 4).await;
	assert_eq!(fetch.payload_error(), None);
	wait_for_files_to_clear(&root.0).await;
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	scope.dispose();
}

#[rstest]
#[case::stale_success(200.0)]
#[case::stale_error(500.0)]
#[serial(model_form_file_upload_globals)]
#[test_attr(wasm_bindgen_test)]
async fn invalid_model_form_snapshot_supersedes_previous_submission(#[case] status: f64) {
	// Arrange
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	fetch.set_status(status);
	let scope = ReactiveScope::new();
	let form = scope.enter(|| {
		let form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		form
	});
	let title = query_input(&root.0, "upload-form-title");
	title.set_value("Report");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	select_file(&query_input(&root.0, "upload-form-avatar"), &avatar_file);
	defer_yield().await;
	let snapshot_error = "Validation failed";

	// Act
	submit(&query_form(&root.0));
	title.set_value("");
	submit(&query_form(&root.0));
	defer_yield().await;

	// Assert
	assert_eq!(fetch.requests(), 0);
	assert_eq!(form.error().get().as_deref(), Some(snapshot_error));
	assert!(!form.loading().get());
	assert!(!form.success().get());

	// Act
	form.set_value("title", serde_json::json!("Report"))
		.expect("restore a valid programmatic submission");
	let mut pending = std::pin::pin!(form.submit_response());
	assert!(futures_util::poll!(pending.as_mut()).is_pending());
	assert!(form.loading().get());
	title.set_value("");
	submit(&query_form(&root.0));

	// Assert
	assert_eq!(form.error().get().as_deref(), Some(snapshot_error));
	assert!(!form.loading().get());
	assert!(!form.success().get());
	assert_eq!(
		pending.await.map_err(|error| error.message().to_owned()),
		if status < 300.0 {
			Ok(())
		} else {
			Err("upload failed".to_owned())
		}
	);
	assert_eq!(fetch.requests(), 1);
	assert_eq!(fetch.payload_error(), None);
	assert_eq!(form.error().get().as_deref(), Some(snapshot_error));
	assert!(!form.loading().get());
	assert!(!form.success().get());
	assert!(same_file(
		&selected_file(&query_input(&root.0, "upload-form-document")),
		&document_file
	));
	assert!(same_file(
		&selected_file(&query_input(&root.0, "upload-form-avatar")),
		&avatar_file
	));

	// Act
	fetch.set_status(200.0);
	form.set_value("title", serde_json::json!("Report"))
		.expect("restore a valid programmatic submission");
	form.submit().await.expect("current submission succeeds");
	assert!(form.success().get());
	title.set_value("");
	submit(&query_form(&root.0));

	// Assert
	assert_eq!(form.error().get().as_deref(), Some(snapshot_error));
	assert!(!form.loading().get());
	assert!(!form.success().get());
}

#[rstest]
#[serial(model_form_file_upload_globals)]
#[test_attr(wasm_bindgen_test)]
async fn multipart_mutation_callbacks_continue_after_form_success_disposes_scope() {
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	fetch.set_status(200.0);
	let scope = Rc::new(ReactiveScope::new());
	let form_success_calls = Rc::new(Cell::new(0));
	let mutation_success_calls = Rc::new(Cell::new(0));
	let mutation = scope.enter(|| {
		let form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		let scope_for_success = Rc::clone(&scope);
		let form_success_calls_for_callback = Rc::clone(&form_success_calls);
		let runtime = use_form(&form)
			.on_submit_success(move |_| {
				form_success_calls_for_callback.set(form_success_calls_for_callback.get() + 1);
				scope_for_success.dispose();
			})
			.build();
		let mutation_success_calls_for_callback = Rc::clone(&mutation_success_calls);
		form.server_mutation(&runtime)
			.on_success(move |_| {
				mutation_success_calls_for_callback
					.set(mutation_success_calls_for_callback.get() + 1);
			})
			.build()
	});

	let title = query_input(&root.0, "upload-form-title");
	title.set_value("Report");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	select_file(&query_input(&root.0, "upload-form-avatar"), &avatar_file);
	defer_yield().await;

	assert_eq!(mutation.dispatch(), MutationDispatchOutcome::Dispatched);
	wait_for_requests(&fetch, 1).await;
	for _ in 0..100 {
		if mutation_success_calls.get() == 1 {
			break;
		}
		TimeoutFuture::new(10).await;
	}

	assert_eq!(form_success_calls.get(), 1);
	assert_eq!(mutation_success_calls.get(), 1);
}

#[rstest]
#[serial(model_form_file_upload_globals)]
#[test_attr(wasm_bindgen_test)]
async fn multipart_mutation_callbacks_continue_after_form_owner_disposal() {
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	fetch.set_status(200.0);
	let form_scope = ReactiveScope::new();
	let (form, runtime) = form_scope.enter(|| {
		let form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		form.clone()
			.into_page()
			.mount(&Element::new(root.0.clone()))
			.expect("model form mounts");
		let runtime = use_form(&form).build();
		(form, runtime)
	});
	let mutation_scope = ReactiveScope::new();
	let mutation_success_calls = Rc::new(Cell::new(0));
	let mutation_success_calls_for_callback = Rc::clone(&mutation_success_calls);
	let mutation = mutation_scope.enter(|| {
		form.server_mutation(&runtime)
			.on_success(move |_| {
				mutation_success_calls_for_callback
					.set(mutation_success_calls_for_callback.get() + 1);
			})
			.build()
	});

	let title = query_input(&root.0, "upload-form-title");
	title.set_value("Report");
	title
		.dispatch_event(&web_sys::Event::new("input").expect("input event"))
		.expect("dispatch title input");
	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	select_file(&query_input(&root.0, "upload-form-avatar"), &avatar_file);
	defer_yield().await;

	assert_eq!(mutation.dispatch(), MutationDispatchOutcome::Dispatched);
	form_scope.dispose();
	wait_for_requests(&fetch, 1).await;
	for _ in 0..100 {
		if mutation_success_calls.get() == 1 {
			break;
		}
		TimeoutFuture::new(10).await;
	}

	assert_eq!(mutation_success_calls.get(), 1);
	mutation_scope.dispose();
}
