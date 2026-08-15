use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn save(
	callback: &dyn Fn() -> reinhardt_core::parsers::UploadedFile,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let _ = callback;
	Ok(())
}

fn main() {}
