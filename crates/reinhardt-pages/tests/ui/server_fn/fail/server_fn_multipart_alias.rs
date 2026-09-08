use reinhardt_pages_macros::server_fn;

type Avatar = reinhardt_core::parsers::UploadedFile;

// Restricted visibility keeps optional MSW metadata out of this alias diagnostic.
#[server_fn]
pub(crate) async fn save(avatar: Avatar) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let _ = avatar;
	Ok(())
}

fn main() {}
