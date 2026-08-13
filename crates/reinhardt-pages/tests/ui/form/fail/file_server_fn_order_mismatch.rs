use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::{
	form,
	server_fn::{ServerFnError, server_fn},
};

#[server_fn]
async fn upload(document: UploadedFile, title: String) -> Result<(), ServerFnError> {
	let _ = (document, title);
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: OrderMismatchForm,
			server_fn: upload,
			fields: {
				title: CharField { required }
				document: FileField { required }
			}
		};
	});
}
