use reinhardt_pages::server_fn::{
	ServerFnError, ServerFnRequestMetadata, ServerFnResponseMetadata, server_fn,
};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ServerFnError>;

#[derive(Serialize)]
pub struct SaveResponse {
	value: String,
}

#[server_fn]
pub async fn aliased_result() -> ApiResult<SaveResponse> {
	Ok(SaveResponse {
		value: "saved".to_string(),
	})
}

fn assert_metadata<T>()
where
	T: ServerFnRequestMetadata<Request = ()>
		+ ServerFnResponseMetadata<Response = SaveResponse, Error = ServerFnError>,
{
}

fn main() {
	assert_metadata::<aliased_result::marker>();
	let _ = aliased_result::mutation();
}
