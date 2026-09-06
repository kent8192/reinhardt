include!("../model_multipart_support.rs");

struct OtherPolicy;

impl ModelFormPolicy for OtherPolicy {
	fn allows(field: &str) -> bool {
		UploadPolicy::allows(field)
	}
}

fn main() {
	let _form = reinhardt_pages::form! {
		name: OtherPolicyUploadForm,
		model: Upload,
		policy: OtherPolicy,
		fields: [title, document, avatar],
		server_fn: upload,
	};
}
