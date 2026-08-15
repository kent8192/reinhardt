use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn save(
	avatars: Vec<reinhardt_core::parsers::UploadedFile>,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let _ = avatars;
	Ok(())
}

fn main() {}
