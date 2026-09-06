include!("../model_multipart_support.rs");

fn main() {
	let _form = reinhardt_pages::form! {
		name: UnvalidatedUploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [title, document, avatar],
		server_fn: upload_unvalidated,
	};
}
