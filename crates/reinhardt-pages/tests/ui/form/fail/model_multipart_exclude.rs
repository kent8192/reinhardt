include!("../model_multipart_support.rs");

use reinhardt_pages::{
	form,
	form::{ModelFormPayloadSelection, ModelFormServerFn},
};

fn require_payload_selection()
where
	upload::marker: ModelFormServerFn<
		ModelFormPayloadSelection<UploadModelFormData<UploadPolicy>, UploadPolicy>,
		UploadFormSchema,
		UploadPolicy,
	>,
{
}

fn main() {
	let _form = form! {
		name: UploadForm,
		model: Upload,
		policy: UploadPolicy,
		exclude: [avatar],
		server_fn: upload,
	};
}
