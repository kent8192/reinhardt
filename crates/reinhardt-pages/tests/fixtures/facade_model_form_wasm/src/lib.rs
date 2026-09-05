use reinhardt::model;
use reinhardt::pages::form;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[model(
	app_label = "profiles",
	table_name = "profiles",
	form(name = ProfileCreateForm, fields(name, enabled)),
	info = false
)]
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct Profile {
	#[field(primary_key = true)]
	pub id: i64,
	pub name: String,
	#[field(default = false)]
	pub enabled: bool,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProfileResponse {
	pub token: String,
}

#[server_fn(model_form = true)]
pub async fn save_profile(
	payload: ProfileCreateFormData,
) -> Result<ProfileResponse, ServerFnError> {
	let _ = payload;
	Ok(ProfileResponse {
		token: "saved".to_owned(),
	})
}

pub fn compile_facade_form() {
	let form = form! {
		name: ProfileForm,
		model_form: ProfileCreateForm,
		server_fn: save_profile,
	};
	let _payload: ProfileCreateFormData = form.data().expect("empty payload is valid");
	let _typed_submit = async {
		let response = form
			.submit_response()
			.await
			.expect("the generated response type is concrete");
		let _: ProfileResponse = response;
	};
}
