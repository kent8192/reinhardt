//! File-only forms accept matching raw identifier server-function parameters.

use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::{
	form,
	server_fn::{ServerFnError, server_fn},
};

#[server_fn]
async fn upload(r#type: UploadedFile) -> Result<(), ServerFnError> {
	let _ = r#type;
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: RawIdentifierFileForm,
			server_fn: upload,
			fields: {
				r#type: FileField { required }
			}
		};
	});
}
