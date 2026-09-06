//! A model form with explicit fields matches a typed multipart server function.

include!("../model_multipart_support.rs");

use reinhardt_pages::{form, use_form};

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
		let runtime = use_form(&form).build();
		let _mutation = form.server_mutation(&runtime).build();
	});
}
