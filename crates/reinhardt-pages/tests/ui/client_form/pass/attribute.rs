use reinhardt_core::validators::{Validate, ValidationErrors};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{UseFormAsyncSubmitOutcome, client_form, use_form};
use serde::{Deserialize, Serialize};

#[client_form(name = ProfileForm, server_fn = submit_profile, validate)]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRequest {
	pub display_name: String,
}

impl Validate for ProfileRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		Ok(())
	}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileResponse {
	display_name: String,
}

#[server_fn]
async fn submit_profile(
	request: crate::ProfileRequest,
) -> Result<ProfileResponse, ServerFnError> {
	Ok(ProfileResponse {
		display_name: request.display_name,
	})
}

fn assert_submit_output(
	value: Result<UseFormAsyncSubmitOutcome<ProfileResponse>, ServerFnError>,
) -> Result<UseFormAsyncSubmitOutcome<ProfileResponse>, ServerFnError> {
	value
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = ProfileForm::new();
		let runtime = use_form(&form).build();
		runtime.set_value(ProfileFormField::DisplayName, "Ada".to_string());
		let request = ProfileForm::to_request(&runtime);
		assert_eq!(request.display_name, "Ada");
		let _submit_future = async { assert_submit_output(form.submit(&runtime).await) };
	});
}
