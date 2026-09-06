use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{ClientForm, use_form};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize, ClientForm)]
#[client_form(server_fn = submit_settings)]
struct SettingsRequest {
	name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct NonCloneResponse {
	name: String,
}

#[server_fn]
async fn submit_settings(
	request: crate::SettingsRequest,
) -> Result<NonCloneResponse, ServerFnError> {
	Ok(NonCloneResponse { name: request.name })
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = SettingsRequestClientForm::new();
		let runtime = use_form(&form).build();
		let _submit_future = form.submit(&runtime);
	});
}
