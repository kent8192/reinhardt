//! Named contracts share the generated native and WASM validation boundary.

use reinhardt::model;
use reinhardt_core::model_form::ModelFormPolicy;
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Serialize};

#[model(
	app_label = "named_validation",
	form(name = ValidatedCreateForm, fields(quota, title, note)),
	info = false
)]
#[derive(Clone, Serialize, Deserialize)]
#[form(validate = validate_named_document)]
pub struct NamedValidationDocument {
	#[field(primary_key = true)]
	pub id: i64,
	#[field(min_length = 3, max_length = 20)]
	#[form(trim)]
	pub title: String,
	#[field(min_value = 1, max_value = 10, default = 3)]
	pub quota: i64,
	#[field(max_length = 64, default = " default note ")]
	#[form(trim)]
	pub note: Option<String>,
	#[field(max_length = 64, editable = false)]
	pub server_token: String,
}

fn validate_named_document<P: ModelFormPolicy>(
	payload: &CleanedNamedValidationDocumentModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload.title().is_some_and(|title| title == "blocked") {
		let mut errors = ValidationErrors::new();
		errors.add("_all", ValidationError::Custom("Blocked title".to_owned()));
		return Err(errors);
	}
	if payload.quota().is_none() {
		let mut errors = ValidationErrors::new();
		errors.add(
			"quota",
			ValidationError::Custom("Missing default".to_owned()),
		);
		return Err(errors);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::model_form::{ModelFormPayload, ModelFormValidatingPayload};

	#[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn named_contract_normalizes_defaults_and_runs_application_validation() {
		// Arrange
		let data: ValidatedCreateFormData =
			json::from_value(json::json!({ "title": "  Valid title  " })).unwrap();

		// Act
		let cleaned = data.clean_and_validate().unwrap();

		// Assert
		assert_eq!(cleaned.title().map(String::as_str), Some("Valid title"));
		assert_eq!(cleaned.quota(), Some(&3));
		assert_eq!(
			cleaned.note().and_then(Option::as_deref),
			Some("default note")
		);
		let mut raw = cleaned.into_raw();
		assert_eq!(raw.is_defaulted("quota"), true);
		assert_eq!(raw.is_defaulted("note"), true);
		assert_eq!(raw.is_defaulted("title"), false);
		raw.set_quota(4);
		assert_eq!(raw.is_defaulted("quota"), false);
		let repeated = raw.clean_and_validate().unwrap();
		assert_eq!(repeated.quota(), Some(&4));
		assert_eq!(
			repeated.note().and_then(Option::as_deref),
			Some("default note")
		);

		let blocked: ValidatedCreateFormData =
			json::from_value(json::json!({ "title": "  blocked  " })).unwrap();
		let errors = match blocked.clean_and_validate() {
			Ok(_) => panic!("named application validation must reject a normalized blocked title"),
			Err(errors) => errors,
		};
		assert_eq!(errors.field_errors().len(), 1);
		assert_eq!(
			errors.field_errors().get("_all"),
			Some(&vec![ValidationError::Custom("Blocked title".to_owned())]),
		);

		let bounded: ValidatedCreateFormData =
			json::from_value(json::json!({ "title": "  Valid title  ", "quota": 11 })).unwrap();
		let errors = match bounded.clean_and_validate() {
			Ok(_) => panic!("named numeric bounds must be enforced"),
			Err(errors) => errors,
		};
		assert_eq!(errors.field_errors().len(), 1);
		assert_eq!(
			errors.field_errors().get("quota"),
			Some(&vec![ValidationError::Custom(
				"Ensure this value is less than or equal to 10".to_owned(),
			)]),
		);
		let multiple: ValidatedCreateFormData =
			json::from_value(json::json!({ "title": " x ", "quota": 11 })).unwrap();
		let errors = match multiple.clean_and_validate() {
			Ok(_) => panic!("named field validation must reject every invalid field"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			vec!["title", "quota"],
		);
		assert_eq!(
			json::from_value::<ValidatedCreateFormData>(json::json!({
				"title": "Valid title",
				"server_token": "untrusted"
			}))
			.is_err(),
			true,
		);
	}
}
