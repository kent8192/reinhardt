#![cfg(all(target_family = "wasm", target_os = "unknown"))]

use js_sys::Function;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormSchema,
};
use reinhardt_pages::form::ModelFormState;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct UploadSchema;

impl ModelFormSchema for UploadSchema {
	type Model = ();

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		const FIELDS: [ModelFormFieldDescriptor; 3] = [
			ModelFormFieldDescriptor {
				name: "document",
				kind: ModelFormFieldKind::File,
				required: true,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
				trim: false,
			},
			ModelFormFieldDescriptor {
				name: "avatar",
				kind: ModelFormFieldKind::Image,
				required: false,
				has_default: false,
				nullable: true,
				editable: true,
				generated_relation_id: false,
				trim: false,
			},
			ModelFormFieldDescriptor {
				name: "title",
				kind: ModelFormFieldKind::Text {
					min_length: None,
					max_length: None,
					multiline: false,
				},
				required: false,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
				trim: false,
			},
		];
		&FIELDS
	}
}

#[derive(Default)]
struct EmptyPayload;

impl ModelFormPayload<AllEditableModelFields> for EmptyPayload {
	fn supplied_fields(&self) -> Vec<&'static str> {
		Vec::new()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, _field: &str) -> Option<serde_json::Value> {
		None
	}

	fn set_json(
		&mut self,
		field: &str,
		_value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		Err(ModelFormPayloadError::UnknownField {
			field: field.to_owned(),
		})
	}
}

fn browser_file(name: &str) -> web_sys::File {
	Function::new_with_args("name", "return new File(['content'], name);")
		.call1(&JsValue::NULL, &JsValue::from_str(name))
		.expect("create browser file")
		.dyn_into::<web_sys::File>()
		.expect("created value must be a File")
}

#[wasm_bindgen_test]
fn file_state_tracks_selected_files_without_json_payload_entries() {
	let mut state = ModelFormState::<UploadSchema, AllEditableModelFields>::new();

	assert!(matches!(state.optional_file_argument("avatar"), Ok(None)));
	assert_eq!(
		state
			.optional_file_argument("title")
			.expect_err("scalar fields must be rejected by file helpers"),
		ModelFormPayloadError::InvalidValue {
			field: "title".to_owned(),
			message: "expected a file field".to_owned(),
		}
	);
	assert_eq!(
		state
			.required_file_argument("document")
			.expect_err("missing required file must fail exactly"),
		ModelFormPayloadError::InvalidValue {
			field: "document".to_owned(),
			message: "is required".to_owned(),
		}
	);
	state
		.set_file("document", browser_file("document.pdf"))
		.expect("required file field should accept a browser File");
	state
		.set_file("avatar", browser_file("avatar.png"))
		.expect("optional image field should accept a browser File");

	assert_eq!(
		state.file("document").map(web_sys::File::name).as_deref(),
		Some("document.pdf")
	);
	assert_eq!(
		state
			.required_file_argument("document")
			.expect("selected required file")
			.name(),
		"document.pdf"
	);
	assert_eq!(
		state
			.optional_file_argument("avatar")
			.expect("selected optional file")
			.map(|file| file.name()),
		Some("avatar.png".to_owned())
	);
	assert!(
		state
			.build_payload::<EmptyPayload>()
			.expect("selected files must not be added to JSON payloads")
			.supplied_fields()
			.is_empty()
	);

	state
		.clear_file("avatar")
		.expect("clearing an optional image should succeed");
	assert!(
		state
			.optional_file_argument("avatar")
			.expect("optional image field")
			.is_none()
	);
	assert_eq!(
		state
			.required_file_argument("document")
			.expect("required document remains selected")
			.name(),
		"document.pdf"
	);
	state
		.clear_file("document")
		.expect("clearing a required file should succeed");
	assert_eq!(
		state
			.required_file_argument("document")
			.expect_err("cleared required file must fail validation"),
		ModelFormPayloadError::InvalidValue {
			field: "document".to_owned(),
			message: "is required".to_owned(),
		}
	);

	state.clear_selected_values();
	assert!(state.file("document").is_none());
	assert!(state.file("avatar").is_none());
}

#[wasm_bindgen_test]
fn file_state_clears_only_files_matching_submitted_snapshot() {
	let mut state = ModelFormState::<UploadSchema, AllEditableModelFields>::new();
	state
		.set_file("document", browser_file("document.pdf"))
		.expect("required file field should accept a browser File");
	state
		.set_file("avatar", browser_file("avatar.png"))
		.expect("optional image field should accept a browser File");
	let submitted = state.clone();

	state
		.set_file("avatar", browser_file("new-avatar.png"))
		.expect("a newer file selection should replace the pending file");
	state.clear_selected_files_matching(&submitted);

	assert!(state.file("document").is_none());
	assert_eq!(
		state.file("avatar").map(web_sys::File::name).as_deref(),
		Some("new-avatar.png")
	);
}
