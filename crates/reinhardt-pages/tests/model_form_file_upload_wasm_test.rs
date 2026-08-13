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
		let stub = Function::new_with_args(
			"request",
			r#"
				globalThis.__reinhardtModelUploadRequests += 1;
				return request.formData().then((formData) => {
					if (formData.get('title') !== '"Report"') throw new Error('title was not JSON encoded');
					if (formData.get('document') !== globalThis.__reinhardtModelUploadDocument) {
						throw new Error('document File identity was not preserved');
					}
					const expectedAvatar = globalThis.__reinhardtModelUploadAvatar;
					if (expectedAvatar === null) {
						if (formData.has('avatar')) throw new Error('empty optional image was not omitted');
					} else if (formData.get('avatar') !== expectedAvatar) {
						throw new Error('avatar File identity was not preserved');
					}
					const status = globalThis.__reinhardtModelUploadStatus;
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

#[wasm_bindgen_test]
async fn model_form_files_render_dispatch_once_and_follow_lifecycle() {
	let root = BodyRoot::new();
	let document_file = browser_file("report.pdf");
	let avatar_file = browser_file("avatar.png");
	let fetch = MultipartFetchGuard::install(&document_file, &avatar_file);
	let scope = ReactiveScope::new();
	let (form_element, title, document, avatar) = scope.enter(|| {
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

		let form_element = root
			.0
			.query_selector("form")
			.expect("query form")
			.expect("model form exists")
			.dyn_into::<web_sys::HtmlFormElement>()
			.expect("form element");
		let title = root
			.0
			.query_selector("#upload-form-title")
			.expect("query title")
			.expect("title input")
			.dyn_into::<web_sys::HtmlInputElement>()
			.expect("title input element");
		let document = root
			.0
			.query_selector("#upload-form-document")
			.expect("query document")
			.expect("document input")
			.dyn_into::<web_sys::HtmlInputElement>()
			.expect("document input element");
		let avatar = root
			.0
			.query_selector("#upload-form-avatar")
			.expect("query avatar")
			.expect("avatar input")
			.dyn_into::<web_sys::HtmlInputElement>()
			.expect("avatar input element");

		assert_eq!(form_element.enctype(), "multipart/form-data");
		assert_eq!(document.type_(), "file");
		assert_eq!(avatar.type_(), "file");
		assert_eq!(avatar.accept(), "image/*");
		title.set_value("Report");
		title
			.dispatch_event(&web_sys::Event::new("input").expect("input event"))
			.expect("dispatch title input");
		select_file(&document, &document_file);
		select_file(&avatar, &avatar_file);
		(form_element, title, document, avatar)
	});

	submit(&form_element);
	defer_yield().await;
	defer_yield().await;
	assert_eq!(fetch.requests(), 1);
	assert!(same_file(&selected_file(&document), &document_file));
	assert!(same_file(&selected_file(&avatar), &avatar_file));
	assert_eq!(title.value(), "Report");
	scope.dispose();
}
