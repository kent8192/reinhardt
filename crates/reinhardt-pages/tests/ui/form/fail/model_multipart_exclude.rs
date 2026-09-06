include!("../model_multipart_support.rs");

fn main() {
	let _form = reinhardt_pages::form! {
		name: UploadForm,
		model: Upload,
		policy: UploadPolicy,
		exclude: [avatar],
		server_fn: upload,
	};
}
