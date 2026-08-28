use reinhardt_core::validators::{Validate, ValidationErrors};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{
	ClientForm as Form, FormRuntimeSource, UseFormAsyncSubmitOutcome, client_form, use_form,
};
use serde::{Deserialize as De, Serialize as Ser};

#[client_form(name = ProfileForm, server_fn = submit_profile, validate)]
#[derive(Clone, PartialEq, Ser, De)]
pub struct ProfileRequest {
	pub display_name: String,
}

impl Validate for ProfileRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		Ok(())
	}
}

#[client_form(name = RenamedForm)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, PartialEq)]
struct RenamedRequest {
	#[serde(rename = "preferredName")]
	preferred_name: String,
}

#[reinhardt_pages::client_form(name = AliasedForm)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, PartialEq, Ser, De, Form)]
struct AliasedRequest {
	#[serde(rename = "preferredName")]
	preferred_name: String,
}

#[derive(Clone, Debug, PartialEq, Ser, De)]
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
		assert_eq!(
			RenamedForm::new().runtime_field_by_name("preferredName"),
			Some(RenamedFormField::PreferredName),
		);
		assert_eq!(
			AliasedForm::new().runtime_field_by_name("preferredName"),
			Some(AliasedFormField::PreferredName),
		);
		let aliased = AliasedRequest {
			preferred_name: "Ada".to_string(),
		};
		assert_eq!(
			serde_json::to_string(&aliased).unwrap(),
			r#"{"preferredName":"Ada"}"#,
		);
		assert_eq!(
			serde_json::from_str::<AliasedRequest>(r#"{"preferredName":"Ada"}"#).unwrap(),
			aliased,
		);
		let _submit_future = async { assert_submit_output(form.submit(&runtime).await) };
	});
}
