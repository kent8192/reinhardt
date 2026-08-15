use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn save(
	_: reinhardt_core::parsers::UploadedFile,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	Ok(())
}

fn main() {}
