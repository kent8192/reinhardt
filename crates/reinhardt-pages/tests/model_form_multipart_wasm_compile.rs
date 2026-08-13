#![cfg(wasm)]

include!("ui/form/model_multipart_support.rs");

use reinhardt_pages::form;

#[test]
fn model_form_multipart_dispatch_compiles() {
	let _payload = UploadModelFormData::<UploadPolicy>::default();
	let _form = form! {
		name: UploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [title, document, avatar],
		server_fn: upload,
	};
}
