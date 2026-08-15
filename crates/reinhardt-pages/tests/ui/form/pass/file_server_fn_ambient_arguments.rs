//! File forms retain typed metadata for ambient server-function arguments.

use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::{
	form,
	server_fn::{ServerFnError, server_fn},
};

#[server_fn]
async fn upload(document: UploadedFile, tenant_id: u64) -> Result<(), ServerFnError> {
	let _ = (document, tenant_id);
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: AmbientFileForm,
			server_fn: upload,
			ambient_arguments: {
				tenant_id: 42_u64,
			},
			fields: {
				document: FileField { required }
			}
		};
	});
}
