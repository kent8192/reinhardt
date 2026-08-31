// The standalone UI fixture accepts target cfg names emitted by the model macro.
#![allow(unexpected_cfgs)]

use reinhardt_macros::model;

include!("../support.rs");

fn validate_raw<P: model_form::ModelFormPolicy>(
	_payload: &RawProfileModelFormData<P>,
) -> Result<(), validators::ValidationErrors> {
	Ok(())
}

fn validate_return<P: model_form::ModelFormPolicy>(
	_payload: &CleanedReturnProfileModelFormData<P>,
) -> Vec<String> {
	Vec::new()
}

async fn validate_async<P: model_form::ModelFormPolicy>(
	_payload: &CleanedAsyncProfileModelFormData<P>,
) -> Result<(), validators::ValidationErrors> {
	Ok(())
}

fn validate_wrong_form<P: model_form::ModelFormPolicy>(
	_payload: &CleanedOtherProfileModelFormData<P>,
) -> Result<(), validators::ValidationErrors> {
	Ok(())
}

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
#[form(validate = validate_raw)]
struct RawProfile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 120)]
	name: String,
}

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
#[form(validate = validate_return)]
struct ReturnProfile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 120)]
	name: String,
}

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
#[form(validate = validate_async)]
struct AsyncProfile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 120)]
	name: String,
}

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
struct OtherProfile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 120)]
	name: String,
}

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
#[form(validate = validate_wrong_form)]
struct WrongFormProfile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 120)]
	name: String,
}

fn main() {}
