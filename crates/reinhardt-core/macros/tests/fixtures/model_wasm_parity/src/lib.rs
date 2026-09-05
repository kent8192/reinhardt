#![deny(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormSchema,
	ModelFormUpdatingPayload, ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

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

	pub nullable_flag: Option<bool>,

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

#[model(
	app_label = "snapshot_uploads",
	table_name = "snapshot_uploads",
	form = true,
	info = false
)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnapshotUploadRecord {
	#[field(primary_key = true)]
	pub id: Option<i64>,
	#[field(max_length = 200)]
	#[form(trim)]
	pub title: String,
	#[field(upload_to = "documents", max_length = 255)]
	pub document: FileField,
	#[field(upload_to = "images", max_length = 255)]
	pub avatar: ImageField,
	#[field(upload_to = "documents", max_length = 255)]
	pub optional_document: Option<FileField>,
}

#[model(
	app_label = "assigned_key_documents",
	table_name = "assigned_key_documents",
	form = true,
	info = false
)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssignedKeyDocument {
	#[field(primary_key = true, editable = true, max_length = 64)]
	pub id: String,
	#[field(max_length = 200)]
	pub title: String,
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

fn validate_cluster<P: reinhardt_core::model_form::ModelFormPolicy>(
	payload: &CleanedClusterModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload.name().is_none() || payload.api_url().is_none() {
		MISSING_CLUSTER_VALIDATOR_CALLS.fetch_add(1, Ordering::SeqCst);
	}
	let mut errors = ValidationErrors::new();
	if payload.name() == payload.api_url() {
		errors.add(
			"_all",
			ValidationError::Custom("Name and API URL must differ".to_owned()),
		);
	}
	if errors.is_empty() {
		Ok(())
	} else {
		Err(errors)
	}
}

#[model(app_label = "clusters", table_name = "clusters", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[form(validate = validate_cluster)]
struct Cluster {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(editable = false)]
	organization_id: i64,
	#[field(min_length = 1, max_length = 63)]
	#[form(trim)]
	name: String,
	#[field(url = true, max_length = 2048)]
	#[form(trim)]
	api_url: String,
	#[field(blank = true, max_length = 120)]
	notes: String,
}

fn reject_required_scalar_candidate<
	P: reinhardt_core::model_form::ModelFormPolicy,
>(
	_payload: &CleanedRequiredScalarRecordModelFormData<P>,
) -> Result<(), ValidationErrors> {
	let mut errors = ValidationErrors::new();
	errors.add(
		"_all",
		ValidationError::Custom("required scalar callback must not run".to_owned()),
	);
	Err(errors)
}

#[model(
	app_label = "required_scalars",
	table_name = "required_scalar_records",
	form = true,
	info = false
)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[form(validate = reject_required_scalar_candidate)]
struct RequiredScalarRecord {
	#[field(primary_key = true)]
	id: Option<i64>,
	enabled: bool,
	replicas: i64,
}

struct ClusterPolicy;

impl reinhardt_core::model_form::ModelFormPolicy for ClusterPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "name" | "api_url" | "notes")
	}
}

struct ClusterNameOnlyPolicy;

impl reinhardt_core::model_form::ModelFormPolicy for ClusterNameOnlyPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "name" | "notes")
	}
}

static MISSING_CLUSTER_VALIDATOR_CALLS: AtomicUsize = AtomicUsize::new(0);

#[model(
	app_label = "email_records",
	table_name = "email_records",
	form = true,
	info = false
)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct EmailRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(email = true, max_length = 200)]
	#[form(trim)]
	email: String,
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
	use rstest::rstest;
	use wasm_bindgen_test::wasm_bindgen_test;

	const PARITY_NUMERIC_ERRORS: &[(&str, &str)] = &[
		(
			"quantity",
			"Ensure this value is greater than or equal to 1",
		),
		("ratio", "Ensure this value is less than or equal to 10"),
		(
			"amount",
			"Ensure this value is greater than or equal to 1",
		),
	];
	const PARITY_EMAIL_ERRORS: &[(&str, &str)] =
		&[("email", "Enter a valid email address")];
	const PARITY_URL_ERRORS: &[(&str, &str)] = &[("api_url", "Enter a valid URL")];
	const PARITY_JSON_DEPTH_ERRORS: &[(&str, &str)] =
		&[("config", "JSON structure is too deeply nested.")];
	const PARITY_DATE_ERRORS: &[(&str, &str)] =
		&[("event_date", "Enter a valid date with a 4-digit year")];
	const PARITY_DATETIME_ERRORS: &[(&str, &str)] =
		&[("aware_at", "Enter a year between 1000 and 9999")];
	const PARITY_FILE_ERRORS: &[(&str, &str)] = &[(
		"document",
		"Stored file references must come from the existing instance",
	)];
	const PARITY_IMAGE_ERRORS: &[(&str, &str)] = &[(
		"avatar",
		"Stored file references must come from the existing instance",
	)];
	const PARITY_FORBIDDEN_ERRORS: &[(&str, &str)] =
		&[("email", "This field is not allowed.")];
	const PARITY_CROSS_FIELD_ERRORS: &[(&str, &str)] = &[
		("title", "Blocked title"),
		("_all", "Blocked project"),
	];

	fn expected_errors(expected: &[(&str, &str)]) -> Vec<(String, String)> {
		expected
			.iter()
			.map(|(field, message)| ((*field).to_owned(), (*message).to_owned()))
			.collect()
	}

	fn error_tuples(errors: &ValidationErrors) -> Vec<(String, String)> {
		errors
			.ordered_field_errors()
			.flat_map(|(field, errors)| {
				errors
					.iter()
					.map(move |error| {
						let message = match error {
							ValidationError::Custom(message) => message.clone(),
							_ => error.to_string(),
						};
						(field.to_owned(), message)
					})
			})
			.collect()
	}

	fn cluster_payload(name: &str, api_url: &str) -> ClusterModelFormData<ClusterPolicy> {
		let mut payload = ClusterModelFormData::<ClusterPolicy>::empty();
		payload
			.set_name(name.to_owned())
			.expect("cluster name should be editable");
		payload
			.set_api_url(api_url.to_owned())
			.expect("cluster API URL should be editable");
		payload
			.set_notes("  preserve whitespace  ".to_owned())
			.expect("cluster notes should be editable");
		payload
	}

	fn cluster_validation_errors<P: reinhardt_core::model_form::ModelFormPolicy>(
		payload: ClusterModelFormData<P>,
	) -> ValidationErrors {
		match payload.clean_and_validate() {
			Ok(_) => panic!("cluster payload should fail validation"),
			Err(errors) => errors,
		}
	}

	fn valid_form_project() -> FormProject {
		FormProject {
			id: 1,
			title: "existing".to_owned(),
			api_url: "https://example.com".to_owned(),
			email: "person@example.com".to_owned(),
			quantity: 5,
			ratio: 5.5,
			amount: rust_decimal::Decimal::new(55, 1),
			nullable_note: None,
			nullable_flag: None,
			config: serde_json::json!({"nested": [true]}),
			published: false,
			event_date: chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
			event_time: chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap(),
			aware_at: chrono::NaiveDate::from_ymd_opt(2026, 9, 1)
				.unwrap()
				.and_hms_opt(12, 30, 0)
				.unwrap()
				.and_utc(),
			naive_at: chrono::NaiveDate::from_ymd_opt(2026, 9, 1)
				.unwrap()
				.and_hms_opt(12, 30, 0)
				.unwrap(),
			token: uuid::Uuid::nil(),
			document: FileField::default(),
			avatar: ImageField::default(),
		}
	}

	#[wasm_bindgen_test]
	fn generated_snapshot_deferral_only_accepts_required_uploads() {
		// Arrange
		let mut data = SnapshotUploadRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("  Upload  ".to_owned()).unwrap();

		// Act
		let strict_errors = data.clone().clean_and_validate().err().unwrap();
		let cleaned = data
			.clean_and_validate_with_deferred_required_fields(&["document", "avatar"])
			.expect("required uploads may be deferred to the multipart boundary");

		// Assert
		assert_eq!(
			error_tuples(&strict_errors),
			expected_errors(&[
				("document", "This field is required."),
				("avatar", "This field is required."),
			])
		);
		assert_eq!(cleaned.title().map(String::as_str), Some("Upload"));
		assert_eq!(cleaned.document(), None);
		assert_eq!(cleaned.avatar(), None);

		for (deferred_fields, invalid_fields) in [
			(&["document", "avatar", "title"][..], &["title"][..]),
			(
				&["document", "avatar", "optional_document"][..],
				&["optional_document"][..],
			),
			(&["document", "avatar", "unknown"][..], &["unknown"][..]),
			(&["title", "unknown"][..], &["title", "unknown"][..]),
		] {
			// Arrange
			let data = SnapshotUploadRecordModelFormData::<AllEditableModelFields>::empty();

			// Act
			let errors = data
				.clean_and_validate_with_deferred_required_fields(deferred_fields)
				.err()
				.expect("non-required upload names must be rejected on WASM");

			// Assert
			assert_eq!(
				error_tuples(&errors),
				invalid_fields
					.iter()
					.map(|field| (
						(*field).to_owned(),
						"only required file or image fields may be deferred".to_owned(),
					))
					.collect::<Vec<_>>()
			);
		}
	}

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

	#[rstest]
	#[wasm_bindgen_test]
	fn generated_payload_cleans_and_validates_in_wasm_runtime() {
		let existing = valid_form_project();
		let assigned_existing = AssignedKeyDocument {
			id: "existing-key".to_owned(),
			title: "existing".to_owned(),
		};
		let mut assigned = AssignedKeyDocumentModelFormData::<AllEditableModelFields>::empty();
		assigned
			.set_id("attacker-key".to_owned())
			.expect("assigned primary key should be editable");
		let errors = match assigned.clean_and_validate_for_update(&assigned_existing) {
			Ok(_) => panic!("direct generated updates must reject supplied primary keys"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			vec![(
				"id".to_owned(),
				"model form primary keys cannot be updated".to_owned()
			)]
		);

		let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
		payload
			.set_title("  trimmed  ".to_owned())
			.expect("title should be editable");
		payload
			.set_api_url("  https://example.com/path?query=value  ".to_owned())
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
			.set_aware_at(
				chrono::NaiveDate::from_ymd_opt(2026, 9, 1)
					.unwrap()
					.and_hms_opt(12, 30, 0)
					.unwrap()
					.and_utc(),
			)
			.expect("aware datetime should be editable");
		payload
			.set_naive_at(
				chrono::NaiveDate::from_ymd_opt(2026, 9, 1)
					.unwrap()
					.and_hms_opt(12, 30, 0)
					.unwrap(),
			)
			.expect("naive datetime should be editable");
		payload
			.set_token(uuid::Uuid::nil())
			.expect("UUID should be editable");
		let cleaned = payload
			.clean_and_validate_for_update(&existing)
			.expect("valid payload");
		assert_eq!(cleaned.title(), Some(&"trimmed".to_owned()));
		assert_eq!(
			cleaned.api_url(),
			Some(&"https://example.com/path?query=value".to_owned())
		);
		assert_eq!(cleaned.email(), Some(&"person@example.com".to_owned()));
		assert_eq!(cleaned.quantity(), Some(&5));
		assert_eq!(cleaned.ratio(), Some(&5.5));
		assert_eq!(
			cleaned.amount(),
			Some(&rust_decimal::Decimal::new(55, 1))
		);
		assert_eq!(cleaned.nullable_note(), None);
		assert_eq!(cleaned.nullable_flag(), None);
		assert_eq!(cleaned.config(), Some(&serde_json::json!({"nested": [true]})));
		assert_eq!(cleaned.published(), Some(&false));
		assert_eq!(
			cleaned.event_date(),
			Some(&chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
		);
		assert_eq!(
			cleaned.event_time(),
			Some(&chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap())
		);
		assert_eq!(
			cleaned.aware_at().map(|value| value.to_rfc3339()),
			Some("2026-09-01T12:30:00+00:00".to_owned())
		);
		assert_eq!(
			cleaned.naive_at().map(|value| value.to_string()),
			Some("2026-09-01 12:30:00".to_owned())
		);
		assert_eq!(cleaned.token(), Some(&uuid::Uuid::nil()));
		let raw = cleaned.clone().into_raw();
		assert_eq!(raw.title(), Some(&"trimmed".to_owned()));
		assert_eq!(
			raw.api_url(),
			Some(&"https://example.com/path?query=value".to_owned())
		);

		for (title, api_url, expected) in [
			(
				"   ",
				"https://example.com",
				vec![("title".to_owned(), "This field is required.".to_owned())],
			),
			(
				"ab",
				"https://example.com",
				vec![(
					"title".to_owned(),
					"Ensure this value has at least 3 characters (it has 2)".to_owned(),
				)],
			),
			(
				"this title is deliberately longer than one hundred and twenty characters so the generated maximum length check rejects it before cross-field validation runs",
				"https://example.com",
				vec![(
					"title".to_owned(),
					"Ensure this value has at most 120 characters (it has 156)".to_owned(),
				)],
			),
			(
				"valid",
				"not a URL",
				vec![("api_url".to_owned(), "Enter a valid URL".to_owned())],
			),
		] {
			let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
			payload.set_title(title.to_owned()).expect("editable title");
			payload
				.set_api_url(api_url.to_owned())
				.expect("editable URL");
			let errors = match payload.clean_and_validate_for_update(&existing) {
				Ok(_) => panic!("invalid field should fail generated validation"),
				Err(errors) => errors,
			};
			assert_eq!(error_tuples(&errors), expected);
		}

		let mut multiple = FormProjectModelFormData::<AllEditableModelFields>::empty();
		multiple
			.set_title("ab".to_owned())
			.expect("editable title");
		multiple
			.set_api_url("not a URL".to_owned())
			.expect("editable URL");
		let errors = match multiple.clean_and_validate_for_update(&existing) {
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
				.clean_and_validate_for_update(&existing)
				.expect("nullable clear should validate")
				.nullable_note(),
			Some(&None)
		);

		let mut nullable_bool = FormProjectModelFormData::<AllEditableModelFields>::empty();
		nullable_bool
			.set_json("nullable_flag", serde_json::Value::Null)
			.expect("nullable boolean should accept an explicit clear");
		assert_eq!(
			nullable_bool
				.clean_and_validate_for_update(&existing)
				.expect("nullable boolean clear should validate")
				.nullable_flag(),
			Some(&None)
		);

		let mut json_null = FormProjectModelFormData::<AllEditableModelFields>::empty();
		json_null
			.set_config(serde_json::Value::Null)
			.expect("JSON null should be editable");
		assert_eq!(
			json_null
				.clean_and_validate_for_update(&existing)
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
		let errors = match numeric.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("numeric bounds should reject the payload"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			expected_errors(PARITY_NUMERIC_ERRORS)
		);

		for (field, value, expected) in [
			("email", "person@localhost", PARITY_EMAIL_ERRORS),
			(
				"api_url",
				"https://example.com:123456/",
				PARITY_URL_ERRORS,
			),
		] {
			let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
			payload
				.set_json(field, serde_json::Value::String(value.to_owned()))
				.expect("text field should be editable");
			let errors = match payload.clean_and_validate_for_update(&existing) {
				Ok(_) => panic!("canonical format validator should reject the boundary value"),
				Err(errors) => errors,
			};
			assert_eq!(error_tuples(&errors), expected_errors(expected));
		}

		let mut deep = serde_json::Value::Null;
		for _ in 0..66 {
			deep = serde_json::Value::Array(vec![deep]);
		}
		let mut json_depth = FormProjectModelFormData::<AllEditableModelFields>::empty();
		json_depth.set_config(deep).expect("JSON should be editable");
		let errors = match json_depth.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("deep JSON should match native rejection"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			expected_errors(PARITY_JSON_DEPTH_ERRORS)
		);

		let mut date = FormProjectModelFormData::<AllEditableModelFields>::empty();
		date.set_event_date(chrono::NaiveDate::from_ymd_opt(25, 1, 15).unwrap())
			.expect("date should be editable");
		let errors = match date.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("out-of-range date year should match native rejection"),
			Err(errors) => errors,
		};
		assert_eq!(error_tuples(&errors), expected_errors(PARITY_DATE_ERRORS));

		let mut year = FormProjectModelFormData::<AllEditableModelFields>::empty();
		year.set_aware_at(
			chrono::NaiveDate::from_ymd_opt(25, 1, 15)
				.unwrap()
				.and_hms_opt(14, 30, 0)
				.unwrap()
				.and_utc(),
		)
		.expect("datetime should be editable");
		let errors = match year.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("out-of-range year should match native rejection"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			expected_errors(PARITY_DATETIME_ERRORS)
		);

		let mut existing_with_document = existing.clone();
		existing_with_document.document = FileField {
			path: "documents/existing.pdf".to_owned(),
			storage_alias: "default".to_owned(),
		};
		let mut existing_document = FormProjectModelFormData::<AllEditableModelFields>::empty();
		existing_document
			.set_document(existing_with_document.document.clone())
			.expect("existing stored reference should be editable");
		let cleaned = existing_document
			.clean_and_validate_for_update(&existing_with_document)
			.expect("the existing stored file reference should be trusted");
		assert_eq!(
			cleaned.document(),
			Some(&existing_with_document.document)
		);

		let mut document = FormProjectModelFormData::<AllEditableModelFields>::empty();
		document
			.set_document(FileField {
				path: "documents/report.pdf".to_owned(),
				storage_alias: "default".to_owned(),
			})
			.expect("stored reference should be editable");
		let errors = match document.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("untrusted file reference should match native rejection"),
			Err(errors) => errors,
		};
		assert_eq!(error_tuples(&errors), expected_errors(PARITY_FILE_ERRORS));

		let mut avatar = FormProjectModelFormData::<AllEditableModelFields>::empty();
		avatar
			.set_avatar(ImageField {
				path: "images/avatar.png".to_owned(),
				storage_alias: "default".to_owned(),
			})
			.expect("stored image reference should be editable");
		let errors = match avatar.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("untrusted image reference should match native rejection"),
			Err(errors) => errors,
		};
		assert_eq!(error_tuples(&errors), expected_errors(PARITY_IMAGE_ERRORS));

		let mut blocked = FormProjectModelFormData::<AllEditableModelFields>::empty();
		blocked
			.set_title("  blocked  ".to_owned())
			.expect("editable title");
		blocked
			.set_api_url("https://example.com".to_owned())
			.expect("editable URL");
		let errors = match blocked.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("cross-field validator should reject blocked project"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			expected_errors(PARITY_CROSS_FIELD_ERRORS)
		);

		let mut field_before_cross =
			FormProjectModelFormData::<AllEditableModelFields>::empty();
		field_before_cross
			.set_title("blocked".to_owned())
			.expect("editable title");
		field_before_cross
			.set_quantity(0)
			.expect("editable integer");
		let errors = match field_before_cross.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("field validation should reject before the callback"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&errors),
			expected_errors(&[(
				"quantity",
				"Ensure this value is greater than or equal to 1",
			)])
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
			error_tuples(&errors),
			expected_errors(PARITY_FORBIDDEN_ERRORS)
		);
	}

	#[rstest]
	#[wasm_bindgen_test]
	fn generated_required_email_uses_the_canonical_message_in_wasm_runtime() {
		let missing = EmailRecordModelFormData::<AllEditableModelFields>::empty();
		let missing_errors = match missing.clean_and_validate() {
			Ok(_) => panic!("missing required email should fail"),
			Err(errors) => errors,
		};

		assert_eq!(
			error_tuples(&missing_errors),
			vec![("email".to_owned(), "This field is required.".to_owned())]
		);

		let mut whitespace = EmailRecordModelFormData::<AllEditableModelFields>::empty();
		whitespace
			.set_email("   ".to_owned())
			.expect("email should be editable");
		let whitespace_errors = match whitespace.clean_and_validate() {
			Ok(_) => panic!("trimmed empty required email should fail"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&whitespace_errors),
			vec![("email".to_owned(), "This field is required.".to_owned())]
		);
	}

	#[rstest]
	#[wasm_bindgen_test]
	fn generated_create_and_update_semantics_match_the_server_boundary_in_wasm_runtime() {
		MISSING_CLUSTER_VALIDATOR_CALLS.store(0, Ordering::SeqCst);
		let mut create = ClusterModelFormData::<ClusterPolicy>::empty();
		create
			.set_api_url("https://example.com".to_owned())
			.expect("cluster API URL should be editable");
		create
			.set_notes("missing name".to_owned())
			.expect("cluster notes should be editable");
		let create_errors = match create.clean_and_validate() {
			Ok(_) => panic!("missing required create input should fail"),
			Err(errors) => errors,
		};

		assert_eq!(
			error_tuples(&create_errors),
			vec![("name".to_owned(), "This field is required.".to_owned())]
		);
		assert_eq!(MISSING_CLUSTER_VALIDATOR_CALLS.load(Ordering::SeqCst), 0);

		let existing = Cluster {
			id: Some(42),
			organization_id: 19,
			name: "original".to_owned(),
			api_url: "https://same.example.com".to_owned(),
			notes: "preserve me".to_owned(),
		};
		let mut update = ClusterModelFormData::<ClusterPolicy>::empty();
		update
			.set_name("https://same.example.com".to_owned())
			.expect("cluster name should be editable");
		let update_errors = match update.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("post-merge cross-field validation should reject the update"),
			Err(errors) => errors,
		};
		assert_eq!(
			error_tuples(&update_errors),
			vec![(
				"_all".to_owned(),
				"Name and API URL must differ".to_owned(),
			)]
		);
	}

	#[rstest]
	#[wasm_bindgen_test]
	fn generated_required_scalars_use_canonical_create_errors_in_wasm_runtime() {
		let payload =
			RequiredScalarRecordModelFormData::<AllEditableModelFields>::empty();
		let errors = match payload.clean_and_validate() {
			Ok(_) => panic!("omitted required scalars must fail before the callback"),
			Err(errors) => errors,
		};

		assert_eq!(
			error_tuples(&errors),
			vec![
				("enabled".to_owned(), "This field is required.".to_owned()),
				("replicas".to_owned(), "This field is required.".to_owned()),
			]
		);
	}

	#[rstest]
	#[wasm_bindgen_test]
	fn generated_cluster_validation_matches_the_server_boundary_in_wasm_runtime() {
		let cleaned = cluster_payload(
			&format!(" {} ", "n".repeat(63)),
			"  https://example.com/api  ",
		)
		.clean_and_validate()
		.expect("normalized cluster boundary values should validate");
		assert_eq!(cleaned.name(), Some(&"n".repeat(63)));
		assert_eq!(cleaned.api_url(), Some(&"https://example.com/api".to_owned()));
		assert_eq!(cleaned.notes(), Some(&"  preserve whitespace  ".to_owned()));

		for (name, api_url, expected) in [
			(
				"   ",
				"https://example.com",
				vec![("name".to_owned(), "This field is required.".to_owned())],
			),
			(
				"nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn",
				"not a URL",
				vec![
					(
						"name".to_owned(),
						"Ensure this value has at most 63 characters (it has 64)".to_owned(),
					),
					("api_url".to_owned(), "Enter a valid URL".to_owned()),
				],
			),
		] {
			let errors = cluster_validation_errors(cluster_payload(name, api_url));
			assert_eq!(error_tuples(&errors), expected);
		}

		let errors = cluster_validation_errors(cluster_payload(
			"  https://example.com  ",
			"https://example.com  ",
		));
		assert_eq!(
			error_tuples(&errors),
			vec![(
				"_all".to_owned(),
				"Name and API URL must differ".to_owned(),
			)]
		);

		let forbidden: ClusterModelFormData<ClusterNameOnlyPolicy> = serde_json::from_value(
			serde_json::json!({
				"name": "https://example.com",
				"api_url": "https://example.com",
			}),
		)
		.expect("known forbidden field should retain rejection evidence");
		let errors = cluster_validation_errors(forbidden);
		assert_eq!(
			error_tuples(&errors),
			vec![(
				"api_url".to_owned(),
				"This field is not allowed.".to_owned(),
			)]
		);

		for rejected_field in ["unknown", "organization_id", "id"] {
			let mut value =
				serde_json::json!({"name": "cluster", "api_url": "https://example.com"});
			value
				.as_object_mut()
				.expect("cluster payload should be an object")
				.insert(rejected_field.to_owned(), serde_json::json!(true));

			let error = match serde_json::from_value::<ClusterModelFormData<ClusterPolicy>>(value) {
				Ok(_) => panic!("untrusted field should be rejected"),
				Err(error) => error,
			};
			assert_eq!(
				error.to_string(),
				format!(
					"unknown field `{rejected_field}`, expected one of `name`, `api_url`, `notes`"
				)
			);
		}
	}
}
