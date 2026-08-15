//! A file-bearing form matches a typed multipart server function.

use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::{
	form,
	server_fn::{ServerFnError, server_fn},
};

#[server_fn]
async fn upload(
	title: String,
	document: UploadedFile,
	avatar: Option<UploadedFile>,
) -> Result<(), ServerFnError> {
	let _ = (title, document, avatar);
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: TypedMultipartForm,
			server_fn: upload,
			fields: {
				title: CharField { required }
				document: FileField { required }
				avatar: ImageField {}
			}
		};
	});
}
