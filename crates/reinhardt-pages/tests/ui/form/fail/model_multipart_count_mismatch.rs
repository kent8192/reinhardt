include!("../model_multipart_support.rs");

fn main() {
	let _form = reinhardt_pages::form! {
		name: ShortUploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [title, document],
		server_fn: upload,
	};
}
