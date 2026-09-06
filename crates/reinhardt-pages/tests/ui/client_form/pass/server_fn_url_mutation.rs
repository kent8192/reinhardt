use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{ClientForm, MutationDispatchOutcome, use_form};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize, ClientForm)]
#[client_form(server_fn = submit_settings)]
struct SettingsRequest {
	name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SettingsResponse {
	name: String,
}

#[server_fn(codec = "url")]
async fn submit_settings(
	request: crate::SettingsRequest,
) -> Result<SettingsResponse, ServerFnError> {
	Ok(SettingsResponse { name: request.name })
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = SettingsRequestClientForm::new();
		let runtime = use_form(&form).build();
		let mutation = form.server_mutation(&runtime).build();
		let _: MutationDispatchOutcome = mutation.dispatch();
	});
}
