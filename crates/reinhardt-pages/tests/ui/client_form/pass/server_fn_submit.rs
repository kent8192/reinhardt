use reinhardt_core::validators::{Validate, ValidationErrors};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{ClientForm, UseFormAsyncSubmitOutcome, use_form};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize, ClientForm)]
#[client_form(server_fn = submit_settings, validate)]
pub struct SettingsRequest {
	pub name: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, ClientForm)]
#[client_form(server_fn = submit_settings_with_inject)]
pub struct InjectedSettingsRequest {
	pub name: String,
}

impl Validate for SettingsRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		Ok(())
	}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsResponse {
	name: String,
}

#[derive(Clone)]
struct Database;

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Database {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self)
	}
}

#[server_fn]
async fn submit_settings(
	request: crate::SettingsRequest,
) -> Result<SettingsResponse, ServerFnError> {
	Ok(SettingsResponse { name: request.name })
}

#[server_fn]
async fn submit_settings_with_inject(
	request: crate::InjectedSettingsRequest,
	#[inject] _db: Database,
) -> Result<SettingsResponse, ServerFnError> {
	Ok(SettingsResponse { name: request.name })
}

fn assert_submit_output(
	value: Result<UseFormAsyncSubmitOutcome<SettingsResponse>, ServerFnError>,
) -> Result<UseFormAsyncSubmitOutcome<SettingsResponse>, ServerFnError> {
	value
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = SettingsRequestClientForm::new();
		let injected_form = InjectedSettingsRequestClientForm::new();
		let runtime = use_form(&form).build();
		let injected_runtime = use_form(&injected_form).build();
		let mutation = form
			.server_mutation(&runtime)
			.reset_form_on_success()
			.on_success(|response| {
				let _: &SettingsResponse = response;
			})
			.build();
		let _: reinhardt_pages::MutationDispatchOutcome = mutation.dispatch();
		let _: Option<SettingsResponse> = mutation.result();
		let injected_mutation = injected_form
			.server_mutation(&injected_runtime)
			.reset_form_on_success()
			.on_success(|response| {
				let _: &SettingsResponse = response;
			})
			.build();
		let _: reinhardt_pages::MutationDispatchOutcome = injected_mutation.dispatch();
		let _: Option<SettingsResponse> = injected_mutation.result();
		let _submit_future = async { assert_submit_output(form.submit(&runtime).await) };
		let _injected_submit_future =
			async { assert_submit_output(injected_form.submit(&injected_runtime).await) };
	});
}
