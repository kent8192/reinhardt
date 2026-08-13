#![cfg(wasm)]

include!("ui/form/model_multipart_support.rs");

use js_sys::{Function, Reflect};
use reinhardt_pages::component::{PageExt, cleanup_reactive_nodes};
use reinhardt_pages::dom::Element;
use reinhardt_pages::form;
use reinhardt_pages::prelude::defer_yield;
use reinhardt_pages::reactive::ReactiveScope;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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

struct MultipartFetchGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
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
		let stub = Function::new_with_args(
			"request",
			r#"
				globalThis.__reinhardtModelUploadRequests += 1;
				return request.formData().then((formData) => {
					let payloadError = null;
					if (formData.get('title') !== '"Report"') payloadError = 'title was not JSON encoded';
					if (formData.get('document') !== globalThis.__reinhardtModelUploadDocument) {
						payloadError = 'document File identity was not preserved';
					}
					const expectedAvatar = globalThis.__reinhardtModelUploadAvatar;
					if (expectedAvatar === null) {
						if (formData.has('avatar')) payloadError = 'empty optional image was not omitted';
					} else if (formData.get('avatar') !== expectedAvatar) {
						payloadError = 'avatar File identity was not preserved';
					}
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
	input.set_files(None);
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
		defer_yield().await;
	}
	assert_eq!(fetch.requests(), expected);
}

async fn wait_for_files_to_clear(root: &web_sys::Element) {
	for _ in 0..8 {
		if file_count(&query_input(root, "upload-form-document")) == 0
			&& file_count(&query_input(root, "upload-form-avatar")) == 0
		{
			return;
		}
		defer_yield().await;
	}
	assert_eq!(file_count(&query_input(root, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(root, "upload-form-avatar")), 0);
}

#[wasm_bindgen_test]
async fn model_form_files_clear_only_after_success_or_reset() {
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
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

	submit(&query_form(&root.0));
	wait_for_requests(&fetch, 1).await;
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
	assert!(form.success().get());
	wait_for_files_to_clear(&root.0).await;
	assert_eq!(file_count(&query_input(&root.0, "upload-form-document")), 0);
	assert_eq!(file_count(&query_input(&root.0, "upload-form-avatar")), 0);
	assert_eq!(query_input(&root.0, "upload-form-title").value(), "Report");

	select_file(
		&query_input(&root.0, "upload-form-document"),
		&document_file,
	);
	select_file(&query_input(&root.0, "upload-form-avatar"), &avatar_file);
	clear_selected_file(&query_input(&root.0, "upload-form-document"));
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
	scope.dispose();
}
