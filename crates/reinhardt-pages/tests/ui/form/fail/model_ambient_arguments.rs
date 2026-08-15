include!("../model_multipart_support.rs");

use reinhardt_pages::form;

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
			ambient_arguments: {
				tenant_id: 42,
			},
		};
	});
}
