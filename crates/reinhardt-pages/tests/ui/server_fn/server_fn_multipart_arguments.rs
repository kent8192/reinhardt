use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::server_fn::{
	ServerFnArgument, ServerFnArgumentCount, ServerFnArgumentKind, ServerFnError, ServerFnMetadata,
};
use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn save(
	name: String,
	avatar: Option<reinhardt_core::parsers::UploadedFile>,
) -> Result<(), ServerFnError> {
	Ok(())
}

#[server_fn]
async fn replace(avatar: UploadedFile) -> Result<(), ServerFnError> {
	Ok(())
}

#[derive(serde::Serialize)]
struct CustomError;

impl From<CustomError> for ServerFnError {
	fn from(_: CustomError) -> Self {
		ServerFnError::server(500, "custom error")
	}
}

impl From<ServerFnError> for CustomError {
	fn from(_: ServerFnError) -> Self {
		Self
	}
}

#[server_fn]
async fn custom_error(avatar: UploadedFile) -> Result<(), CustomError> {
	let _ = avatar;
	Ok(())
}

fn assert_count<T: ServerFnArgumentCount<2>>() {}

fn assert_name<T: 'static>() {}

fn assert_metadata() {
	assert_eq!(<save::marker as ServerFnMetadata>::ARGUMENTS[0].name, "name");
	assert_eq!(
		<save::marker as ServerFnMetadata>::ARGUMENTS[0].kind,
		ServerFnArgumentKind::Json
	);
	assert_eq!(<save::marker as ServerFnMetadata>::ARGUMENTS[1].name, "avatar");
	assert_eq!(
		<save::marker as ServerFnMetadata>::ARGUMENTS[1].kind,
		ServerFnArgumentKind::OptionalFile
	);
	assert!(<save::marker as ServerFnMetadata>::USES_MULTIPART);
	assert_eq!(
		<save::marker as ServerFnArgument<0>>::METADATA.name,
		<save::marker as ServerFnMetadata>::ARGUMENTS[0].name
	);
	assert_eq!(
		<save::marker as ServerFnArgument<1>>::METADATA.kind,
		ServerFnArgumentKind::OptionalFile
	);
	assert_eq!(<replace::marker as ServerFnMetadata>::ARGUMENTS[0].name, "avatar");
	assert_eq!(
		<replace::marker as ServerFnMetadata>::ARGUMENTS[0].kind,
		ServerFnArgumentKind::File
	);
	assert!(<replace::marker as ServerFnMetadata>::USES_MULTIPART);
	assert_eq!(
		<replace::marker as ServerFnArgument<0>>::METADATA.kind,
		ServerFnArgumentKind::File
	);
	assert_count::<save::marker>();
	assert_name::<<save::marker as ServerFnArgument<0>>::Name>();
	assert_name::<<save::marker as ServerFnArgument<1>>::Name>();
}

fn main() {
	assert_metadata();
}
