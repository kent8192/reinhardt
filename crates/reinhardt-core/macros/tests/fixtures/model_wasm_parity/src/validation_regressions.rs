//! Regression coverage for generated validation and model construction.

use super::{FileField, ImageField};
use reinhardt::model;
use reinhardt_core::model_form::ModelFormPolicy;
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

#[model(app_label = "serde_files", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerdeFiles {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = "documents", max_length = 255)]
	#[serde(rename = "attachment")]
	upload_file: FileField,
	#[field(upload_to = "documents", max_length = 255)]
	preview_file: FileField,
	#[field(upload_to = "images", max_length = 255)]
	#[serde(skip)]
	hidden_image: Option<ImageField>,
}

static DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);

fn generated_default() -> String {
	format!(
		" value-{} ",
		DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst) + 1
	)
}

#[model(app_label = "default_candidates", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[form(validate = validate_default_candidate)]
struct DefaultCandidate {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 16, default = generated_default())]
	#[form(trim)]
	code: String,
	#[field(max_length = 16, default = " note ")]
	#[form(trim)]
	note: Option<String>,
	#[field(min_length = 3, max_length = 16)]
	#[form(trim)]
	optional: Option<String>,
	#[field(max_length = 16)]
	blocked: Option<String>,
}

fn validate_default_candidate<P: ModelFormPolicy>(
	data: &CleanedDefaultCandidateModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if data.code().map(String::as_str) == data.blocked().and_then(Option::as_deref)
		&& data.code().is_some()
	{
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Blocked default".to_owned()),
		);
		return Err(errors);
	}
	Ok(())
}

fn invalid_default() -> String {
	"too long".to_owned()
}

#[model(app_label = "invalid_defaults", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InvalidDefault {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 3, default = invalid_default())]
	code: String,
}

#[model(app_label = "float_candidates", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FloatCandidate {
	#[field(primary_key = true)]
	id: i64,
	#[field(default = 1.5)]
	single: f32,
	#[field(default = 2.5)]
	double: f64,
	nullable: Option<f64>,
}

fn default_file() -> FileField {
	json::from_value(json::json!({"path": "documents/default.pdf", "storage": "default"})).unwrap()
}

fn default_image() -> ImageField {
	json::from_value(json::json!({"path": "images/default.png", "storage": "default"})).unwrap()
}

#[model(app_label = "default_files", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DefaultFiles {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = "documents", max_length = 255, default = default_file())]
	document: FileField,
	#[field(upload_to = "images", max_length = 255, default = default_image())]
	image: ImageField,
}

#[model(app_label = "merged_candidates", form = true, info = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[form(validate = validate_merged_candidate)]
struct MergedCandidate {
	#[field(primary_key = true)]
	id: i64,
	#[field(editable = false)]
	owner_id: i64,
	#[field(max_length = 16)]
	first: String,
	#[field(max_length = 16)]
	second: Option<String>,
}

fn validate_merged_candidate<P: ModelFormPolicy>(
	data: &CleanedMergedCandidateModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if data.first().map(String::as_str) == data.second().and_then(Option::as_deref)
		&& data.first().is_some()
	{
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Values must differ".to_owned()),
		);
		return Err(errors);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::model_form::{
		AllEditableModelFields, ModelFormPayload, ModelFormUpdatingPayload,
		ModelFormValidatingPayload,
	};

	fn messages(errors: &ValidationErrors) -> Vec<(String, String)> {
		errors
			.ordered_field_errors()
			.flat_map(|(field, errors)| {
				errors.iter().map(move |error| {
					let message = match error {
						ValidationError::Custom(message) => message.clone(),
						_ => error.to_string(),
					};
					(field.to_owned(), message)
				})
			})
			.collect()
	}

	struct RestoreDefaultCalls(usize);

	impl Drop for RestoreDefaultCalls {
		fn drop(&mut self) {
			DEFAULT_CALLS.store(self.0, Ordering::SeqCst);
		}
	}

	#[cfg_attr(
		not(all(target_family = "wasm", target_os = "unknown")),
		rstest::rstest
	)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn nonfinite_typed_floats_are_rejected_before_null_or_omission_handling() {
		// Arrange
		let existing = FloatCandidate {
			id: 7,
			single: 1.5,
			double: 2.5,
			nullable: None,
		};
		let expected = vec![
			("single".to_owned(), "Expected number or string".to_owned()),
			("double".to_owned(), "Expected number or string".to_owned()),
		];
		for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
			let mut payload = FloatCandidateModelFormData::<AllEditableModelFields>::empty();
			payload.set_single(value as f32).unwrap();
			payload.set_double(value).unwrap();

			// Act
			let create_errors = payload.clone().clean_and_validate().err().unwrap();
			let update_errors = payload
				.clean_and_validate_for_update(&existing)
				.err()
				.unwrap();

			// Assert
			assert_eq!(messages(&create_errors), expected);
			assert_eq!(messages(&update_errors), expected);
		}

		let mut payload = FloatCandidateModelFormData::<AllEditableModelFields>::empty();
		payload.set_nullable(None).unwrap();
		let cleaned = payload.clean_and_validate().unwrap();
		assert_eq!(cleaned.single(), Some(&1.5));
		assert_eq!(cleaned.double(), Some(&2.5));
		assert_eq!(cleaned.nullable(), Some(&None));
		let omitted = FloatCandidateModelFormData::<AllEditableModelFields>::empty()
			.clean_and_validate_for_update(&existing)
			.unwrap();
		assert_eq!(omitted.single(), None);
		assert_eq!(omitted.double(), None);
	}

	#[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn stored_files_use_rust_names_despite_serde_attributes() {
		// Arrange
		let file = json::json!({"path": "documents/original.pdf", "storage": "default"});
		let image = json::json!({"path": "images/original.png", "storage": "default"});
		let existing = SerdeFiles {
			id: 7,
			upload_file: json::from_value(file.clone()).unwrap(),
			preview_file: json::from_value(file.clone()).unwrap(),
			hidden_image: Some(json::from_value(image.clone()).unwrap()),
		};
		let payload_value =
			json::json!({"upload_file": file, "preview_file": file, "hidden_image": image});
		let payload: SerdeFilesModelFormData<AllEditableModelFields> =
			json::from_value(payload_value.clone()).unwrap();

		// Act
		let cleaned = payload
			.clone()
			.clean_and_validate_for_update(&existing)
			.unwrap();

		// Assert
		assert_eq!(json::to_value(cleaned.into_raw()).unwrap(), payload_value);
		assert_eq!(
			json::to_value(&existing).unwrap(),
			json::json!({"id": 7, "attachment": file, "previewFile": file})
		);
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		{
			use reinhardt::forms::ModelForm;
			let mut form = ModelForm::from_payload_and_instance(payload, existing.clone());
			assert_eq!(form.is_valid(), true);
			let mut form = ModelForm::from_payload_and_instance(
				SerdeFilesModelFormData::<AllEditableModelFields>::empty(),
				existing,
			);
			form.set_field_value("hidden_image", image).unwrap();
			assert_eq!(form.is_valid(), true);
		}
	}

	#[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)]
	#[cfg_attr(
		not(all(target_family = "wasm", target_os = "unknown")),
		serial_test::serial(model_form_defaults)
	)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn create_defaults_are_normalized_validated_and_evaluated_once() {
		// Arrange
		let _counter = RestoreDefaultCalls(DEFAULT_CALLS.swap(0, Ordering::SeqCst));
		let payload: DefaultCandidateModelFormData<AllEditableModelFields> =
			json::from_value(json::json!({"code": "   ", "note": "   ", "optional": "   "}))
				.unwrap();

		// Act
		let cleaned = payload.clean_and_validate().unwrap();

		// Assert
		assert_eq!(cleaned.code().map(String::as_str), Some("value-1"));
		assert_eq!(cleaned.note().and_then(Option::as_deref), Some("note"));
		assert_eq!(cleaned.optional(), Some(&None));
		assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
		let raw = cleaned.clone().into_raw();
		assert_eq!(raw.is_defaulted("code"), true);
		let repeated = raw.clean_and_validate().unwrap();
		assert_eq!(repeated.code(), cleaned.code());
		assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		{
			let model = cleaned.clone().into_model().unwrap();
			assert_eq!(model.code, "value-1");
			let mut form =
				reinhardt::forms::ModelForm::<DefaultCandidate>::from_payload(repeated.into_raw());
			assert_eq!(form.build_instance().unwrap().code, "value-1");
			let existing = DefaultCandidate {
				id: 9,
				code: "existing".to_owned(),
				note: Some("old".to_owned()),
				optional: Some("present".to_owned()),
				blocked: None,
			};
			let updated = cleaned.apply_to(existing).unwrap();
			assert_eq!(updated.code, "existing");
			assert_eq!(updated.note.as_deref(), Some("old"));
			assert_eq!(updated.optional, None);
			assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
		}

		DEFAULT_CALLS.store(0, Ordering::SeqCst);
		let blocked: DefaultCandidateModelFormData<AllEditableModelFields> =
			json::from_value(json::json!({"blocked": "value-1"})).unwrap();
		let errors = blocked.clean_and_validate().err().unwrap();
		assert_eq!(
			messages(&errors),
			vec![("_all".to_owned(), "Blocked default".to_owned())]
		);
		assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
	}

	#[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn omitted_defaults_obey_field_bounds() {
		let errors = InvalidDefaultModelFormData::<AllEditableModelFields>::empty()
			.clean_and_validate()
			.err()
			.unwrap();
		assert_eq!(
			messages(&errors),
			vec![(
				"code".to_owned(),
				"Ensure this value has at most 3 characters (it has 8)".to_owned()
			)]
		);
		struct NoClientFields;
		impl ModelFormPolicy for NoClientFields {
			fn allows(_field: &str) -> bool {
				false
			}
		}
		let errors = InvalidDefaultModelFormData::<NoClientFields>::empty()
			.clean_and_validate()
			.err()
			.unwrap();
		assert_eq!(
			messages(&errors),
			vec![(
				"code".to_owned(),
				"Ensure this value has at most 3 characters (it has 8)".to_owned()
			)]
		);
	}

	#[cfg_attr(not(all(target_family = "wasm", target_os = "unknown")), test)]
	#[cfg_attr(
		all(target_family = "wasm", target_os = "unknown"),
		wasm_bindgen_test::wasm_bindgen_test
	)]
	fn only_server_evaluated_file_defaults_are_trusted() {
		// Arrange
		let data = DefaultFilesModelFormData::<AllEditableModelFields>::empty();

		// Act
		let cleaned = data.clean_and_validate().unwrap();

		// Assert
		assert_eq!(cleaned.document(), Some(&default_file()));
		assert_eq!(cleaned.image(), Some(&default_image()));
		let raw = cleaned.into_raw();
		assert_eq!(raw.is_defaulted("document"), true);
		assert_eq!(raw.is_defaulted("image"), true);
		let untrusted: DefaultFilesModelFormData<AllEditableModelFields> =
			json::from_value(json::to_value(&raw).unwrap()).unwrap();
		let errors = untrusted.clean_and_validate().err().unwrap();
		assert_eq!(
			messages(&errors),
			vec![
				(
					"document".to_owned(),
					"Stored file references must come from the existing instance".to_owned()
				),
				(
					"image".to_owned(),
					"Stored file references must come from the existing instance".to_owned()
				),
			]
		);
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		{
			let mut form = reinhardt::forms::ModelForm::<DefaultFiles>::from_payload(raw.clone());
			assert_eq!(form.build_instance().unwrap().document, default_file());
		}
		let mut edited = raw;
		edited.set_document(default_file()).unwrap();
		assert_eq!(edited.is_defaulted("document"), false);
		assert_eq!(
			messages(&edited.clean_and_validate().err().unwrap()),
			vec![(
				"document".to_owned(),
				"Stored file references must come from the existing instance".to_owned(),
			)]
		);
	}

	#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
	#[test]
	fn apply_to_revalidates_the_actual_merged_candidate() {
		// Arrange
		let payload: MergedCandidateModelFormData<AllEditableModelFields> =
			json::from_value(json::json!({"first": "same"})).unwrap();
		let cleaned = payload.clean_and_validate().unwrap();
		let existing = MergedCandidate {
			id: 7,
			owner_id: 3,
			first: "before".to_owned(),
			second: Some("same".to_owned()),
		};

		// Act
		let error = cleaned.apply_to(existing).unwrap_err();

		// Assert
		let reinhardt::forms::ModelFormError::FieldValidation { errors } = error else {
			panic!("expected structured field validation");
		};
		assert_eq!(
			errors,
			std::collections::HashMap::from([(
				"_all".to_owned(),
				vec!["Values must differ".to_owned()]
			)])
		);
	}
}
