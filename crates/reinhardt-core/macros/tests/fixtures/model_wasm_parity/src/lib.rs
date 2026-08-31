#![deny(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormSchema,
	ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Serialize};

#[model(app_label = "projects", table_name = "projects", info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Project {
	#[field(primary_key = true, max_length = 64)]
	pub id: String,

	#[field(max_length = 120)]
	pub name: String,
}

#[model(app_label = "jobs", table_name = "jobs", form = true, info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Job {
	#[field(primary_key = true)]
	pub id: i64,

	#[rel(foreign_key)]
	pub project: reinhardt::db::associations::ForeignKeyField<Project>,

	#[field(max_length = 120)]
	pub job_type: String,
}

#[model(app_label = "forms", table_name = "forms", form = true, info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
#[form(validate = validate_form_project)]
pub struct FormProject {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(min_length = 3, max_length = 120)]
	#[form(trim)]
	pub title: String,

	#[field(url = true, max_length = 200)]
	#[form(trim)]
	pub api_url: String,

	pub aware_at: chrono::DateTime<chrono::Utc>,

	pub naive_at: chrono::NaiveDateTime,
}

fn validate_form_project<P: reinhardt_core::model_form::ModelFormPolicy>(
	payload: &CleanedFormProjectModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload
		.title()
		.is_some_and(|title| title == "blocked" || title.is_empty())
	{
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Blocked project".to_owned()),
		);
		Err(errors)
	} else {
		Ok(())
	}
}

pub fn retry_preserves_project(job: &Job, retry: &Job) -> bool {
	job.project_id() == retry.project_id()
}

pub fn accepts_foreign_key_id(job: &Job) -> String {
	job.project_id()
}

pub fn foreign_key_form_kind_is_text() -> bool {
	matches!(
		JobFormSchema::project_id().kind,
		ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(64),
			multiline: false,
		}
	)
}

pub fn model_form_schema_fields() -> usize {
	FormProjectFormSchema::fields().len()
}

pub fn model_form_payload_has_title() -> bool {
	let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
	payload.set_title("draft".to_owned());
	payload.title().is_some_and(|title| title == "draft")
}

pub fn model_form_datetime_payload_round_trips() -> bool {
	if FormProjectFormSchema::aware_at().kind != ModelFormFieldKind::DateTime
		|| FormProjectFormSchema::naive_at().kind != ModelFormFieldKind::NaiveDateTime
	{
		return false;
	}
	let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
	if payload
		.set_json("aware_at", serde_json::json!("2026-07-25T14:30:00Z"))
		.is_err()
		|| payload
			.set_json("naive_at", serde_json::json!("2026-07-25T14:30:00"))
			.is_err()
	{
		return false;
	}
	matches!(
		(payload.aware_at(), payload.naive_at()),
		(Some(aware), Some(naive))
			if aware.to_rfc3339() == "2026-07-25T14:30:00+00:00"
				&& naive.to_string() == "2026-07-25 14:30:00"
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use wasm_bindgen_test::wasm_bindgen_test;

	#[wasm_bindgen_test]
	fn generated_datetime_payload_round_trips_in_wasm_runtime() {
		assert_eq!(
			FormProjectFormSchema::aware_at().kind,
			ModelFormFieldKind::DateTime
		);
		assert_eq!(
			FormProjectFormSchema::naive_at().kind,
			ModelFormFieldKind::NaiveDateTime
		);

		let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
		payload
			.set_json("aware_at", serde_json::json!("2026-07-25T14:30:00Z"))
			.expect("aware datetime should deserialize in WASM");
		payload
			.set_json("naive_at", serde_json::json!("2026-07-25T14:30:00"))
			.expect("naive datetime should deserialize in WASM");

		assert_eq!(
			payload
				.aware_at()
				.expect("aware datetime should be present")
				.to_rfc3339(),
			"2026-07-25T14:30:00+00:00"
		);
		assert_eq!(
			payload
				.naive_at()
				.expect("naive datetime should be present")
				.to_string(),
			"2026-07-25 14:30:00"
		);
	}

	#[wasm_bindgen_test]
	fn generated_payload_cleans_and_validates_in_wasm_runtime() {
		let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
		payload
			.set_title("  trimmed  ".to_owned())
			.expect("title should be editable");
		payload
			.set_api_url("  https://example.com  ".to_owned())
			.expect("URL should be editable");
		let cleaned = payload.clean_and_validate().expect("valid payload");
		assert_eq!(cleaned.title(), Some(&"trimmed".to_owned()));
		assert_eq!(cleaned.api_url(), Some(&"https://example.com".to_owned()));
		let raw = cleaned.clone().into_raw();
		assert_eq!(raw.title(), Some(&"trimmed".to_owned()));
		assert_eq!(raw.api_url(), Some(&"https://example.com".to_owned()));

		for (title, api_url, field) in [
			("   ", "https://example.com", "title"),
			("ab", "https://example.com", "title"),
			(
				"this title is deliberately longer than one hundred and twenty characters so the generated maximum length check rejects it before cross-field validation runs",
				"https://example.com",
				"title",
			),
			("valid", "not a URL", "api_url"),
		] {
			let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
			payload.set_title(title.to_owned()).expect("editable title");
			payload
				.set_api_url(api_url.to_owned())
				.expect("editable URL");
			let errors = match payload.clean_and_validate() {
				Ok(_) => panic!("invalid field should fail generated validation"),
				Err(errors) => errors,
			};
			assert!(errors.field_errors().contains_key(field));
			assert!(!errors.field_errors().contains_key("_all"));
		}

		let mut multiple = FormProjectModelFormData::<AllEditableModelFields>::empty();
		multiple
			.set_title("ab".to_owned())
			.expect("editable title");
		multiple
			.set_api_url("not a URL".to_owned())
			.expect("editable URL");
		let errors = match multiple.clean_and_validate() {
			Ok(_) => panic!("invalid fields should fail generated validation"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["title", "api_url"]
		);

		let mut blocked = FormProjectModelFormData::<AllEditableModelFields>::empty();
		blocked
			.set_title("  blocked  ".to_owned())
			.expect("editable title");
		blocked
			.set_api_url("https://example.com".to_owned())
			.expect("editable URL");
		let errors = match blocked.clean_and_validate() {
			Ok(_) => panic!("cross-field validator should reject blocked project"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["_all"]
		);
	}
}
