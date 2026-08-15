use reinhardt_pages_macros::server_fn;

type Avatar = reinhardt_core::parsers::UploadedFile;

#[server_fn]
async fn save(avatar: Avatar) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let _ = avatar;
	Ok(())
}

fn main() {}
