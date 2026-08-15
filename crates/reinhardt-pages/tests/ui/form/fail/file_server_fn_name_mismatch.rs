use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::{
	form,
	server_fn::{ServerFnError, server_fn},
};

#[server_fn]
async fn upload(uploaded_document: UploadedFile) -> Result<(), ServerFnError> {
	let _ = uploaded_document;
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: NameMismatchForm,
			server_fn: upload,
			fields: {
				document: FileField { required }
			}
		};
	});
}
