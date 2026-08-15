use reinhardt_pages_macros::server_fn;

#[server_fn(codec = "json")]
async fn save(
	avatar: reinhardt_core::parsers::UploadedFile,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let _ = avatar;
	Ok(())
}

fn main() {}
