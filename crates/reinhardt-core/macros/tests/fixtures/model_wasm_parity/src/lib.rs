#![deny(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormSchema,
	ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileField {
	path: String,
	#[serde(rename = "storage")]
	storage_alias: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageField {
	path: String,
	#[serde(rename = "storage")]
	storage_alias: String,
}

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

	#[field(email = true, max_length = 200)]
	#[form(trim)]
	pub email: String,

	#[field(min_value = 1, max_value = 10)]
	pub quantity: i64,

	#[field(min_value = 1, max_value = 10)]
	pub ratio: f64,

	#[field(min_value = 1, max_value = 10)]
	pub amount: rust_decimal::Decimal,

	#[field(max_length = 40, blank = true)]
	pub nullable_note: Option<Option<String>>,

	pub config: serde_json::Value,

	pub published: bool,

	pub event_date: chrono::NaiveDate,

	pub event_time: chrono::NaiveTime,

	pub aware_at: chrono::DateTime<chrono::Utc>,

	pub naive_at: chrono::NaiveDateTime,

	pub token: uuid::Uuid,

	#[field(upload_to = "documents", max_length = 255)]
	pub document: FileField,

	#[field(upload_to = "images", max_length = 255)]
	pub avatar: ImageField,
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
		errors.add(
			"title",
			ValidationError::Custom("Blocked title".to_owned()),
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
		payload
			.set_email("  person@example.com  ".to_owned())
			.expect("email should be editable");
		payload.set_quantity(5).expect("integer should be editable");
		payload.set_ratio(5.5).expect("float should be editable");
		payload
			.set_amount(rust_decimal::Decimal::new(55, 1))
			.expect("decimal should be editable");
		payload
			.set_config(serde_json::json!({"nested": [true]}))
			.expect("JSON should be editable");
		payload
			.set_published(false)
			.expect("boolean should be editable");
		payload
			.set_event_date(chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
			.expect("date should be editable");
		payload
			.set_event_time(chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap())
			.expect("time should be editable");
		payload
			.set_token(uuid::Uuid::nil())
			.expect("UUID should be editable");
		let cleaned = payload.clean_and_validate().expect("valid payload");
		assert_eq!(cleaned.title(), Some(&"trimmed".to_owned()));
		assert_eq!(cleaned.api_url(), Some(&"https://example.com".to_owned()));
		assert_eq!(cleaned.email(), Some(&"person@example.com".to_owned()));
		assert_eq!(cleaned.quantity(), Some(&5));
		assert_eq!(cleaned.ratio(), Some(&5.5));
		assert_eq!(
			cleaned.amount(),
			Some(&rust_decimal::Decimal::new(55, 1))
		);
		assert_eq!(cleaned.nullable_note(), None);
		assert_eq!(cleaned.config(), Some(&serde_json::json!({"nested": [true]})));
		assert_eq!(cleaned.published(), Some(&false));
		assert_eq!(cleaned.token(), Some(&uuid::Uuid::nil()));
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

		let mut explicit_null =
			FormProjectModelFormData::<AllEditableModelFields>::empty();
		explicit_null
			.set_json("nullable_note", serde_json::Value::Null)
			.expect("nullable value should accept an explicit clear");
		assert_eq!(
			explicit_null
				.clean_and_validate()
				.expect("nullable clear should validate")
				.nullable_note(),
			Some(&None)
		);

		let mut json_null = FormProjectModelFormData::<AllEditableModelFields>::empty();
		json_null
			.set_config(serde_json::Value::Null)
			.expect("JSON null should be editable");
		assert_eq!(
			json_null
				.clean_and_validate()
				.expect("JSON null matches native model JSON cleaning")
				.config(),
			Some(&serde_json::Value::Null)
		);

		let mut numeric = FormProjectModelFormData::<AllEditableModelFields>::empty();
		numeric.set_quantity(0).expect("editable integer");
		numeric.set_ratio(11.0).expect("editable float");
		numeric
			.set_amount(rust_decimal::Decimal::ZERO)
			.expect("editable decimal");
		let errors = match numeric.clean_and_validate() {
			Ok(_) => panic!("numeric bounds should reject the payload"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["quantity", "ratio", "amount"]
		);

		for (field, value) in [
			("email", "person@localhost"),
			("api_url", "https://example.com?query=value"),
		] {
			let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
			payload
				.set_json(field, serde_json::Value::String(value.to_owned()))
				.expect("text field should be editable");
			let errors = match payload.clean_and_validate() {
				Ok(_) => panic!("canonical format validator should reject the boundary value"),
				Err(errors) => errors,
			};
			assert_eq!(
				errors
					.ordered_field_errors()
					.map(|(field, _)| field)
					.collect::<Vec<_>>(),
				[field]
			);
		}

		let mut deep = serde_json::Value::Null;
		for _ in 0..66 {
			deep = serde_json::Value::Array(vec![deep]);
		}
		let mut json_depth = FormProjectModelFormData::<AllEditableModelFields>::empty();
		json_depth.set_config(deep).expect("JSON should be editable");
		let errors = match json_depth.clean_and_validate() {
			Ok(_) => panic!("deep JSON should match native rejection"),
			Err(errors) => errors,
		};
		assert!(errors.field_errors().contains_key("config"));

		let mut year = FormProjectModelFormData::<AllEditableModelFields>::empty();
		year.set_aware_at(
			chrono::NaiveDate::from_ymd_opt(25, 1, 15)
				.unwrap()
				.and_hms_opt(14, 30, 0)
				.unwrap()
				.and_utc(),
		)
		.expect("datetime should be editable");
		let errors = match year.clean_and_validate() {
			Ok(_) => panic!("out-of-range year should match native rejection"),
			Err(errors) => errors,
		};
		assert!(errors.field_errors().contains_key("aware_at"));

		let mut document = FormProjectModelFormData::<AllEditableModelFields>::empty();
		document
			.set_document(FileField {
				path: "documents/report.pdf".to_owned(),
				storage_alias: "default".to_owned(),
			})
			.expect("stored reference should be editable");
		let errors = match document.clean_and_validate() {
			Ok(_) => panic!("untrusted file reference should match native rejection"),
			Err(errors) => errors,
		};
		assert!(errors.field_errors().contains_key("document"));

		let mut avatar = FormProjectModelFormData::<AllEditableModelFields>::empty();
		avatar
			.set_avatar(ImageField {
				path: "images/avatar.png".to_owned(),
				storage_alias: "default".to_owned(),
			})
			.expect("stored image reference should be editable");
		let errors = match avatar.clean_and_validate() {
			Ok(_) => panic!("untrusted image reference should match native rejection"),
			Err(errors) => errors,
		};
		assert!(errors.field_errors().contains_key("avatar"));

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
			["title", "_all"]
		);

		let mut field_before_cross =
			FormProjectModelFormData::<AllEditableModelFields>::empty();
		field_before_cross
			.set_title("blocked".to_owned())
			.expect("editable title");
		field_before_cross
			.set_quantity(0)
			.expect("editable integer");
		let errors = match field_before_cross.clean_and_validate() {
			Ok(_) => panic!("field validation should reject before the callback"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["quantity"]
		);

		struct TitleOnly;
		impl reinhardt_core::model_form::ModelFormPolicy for TitleOnly {
			fn allows(field: &str) -> bool {
				field == "title"
			}
		}
		let forbidden: FormProjectModelFormData<TitleOnly> = serde_json::from_value(
			serde_json::json!({
				"title": "blocked",
				"email": "person@example.com",
			}),
		)
		.expect("known forbidden field should be recorded");
		let errors = match forbidden.clean_and_validate() {
			Ok(_) => panic!("forbidden field should reject before the callback"),
			Err(errors) => errors,
		};
		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["email"]
		);
	}
}
