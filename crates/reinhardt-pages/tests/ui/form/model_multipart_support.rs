use std::marker::PhantomData;

use reinhardt_core::{
	model_form::{
		ModelFormCleanedPayload, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
		ModelFormPayloadError, ModelFormPolicy, ModelFormSchema, ModelFormValidatingPayload,
	},
	validators::{ValidationError, ValidationErrors},
};
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

struct Upload;

struct UploadPolicy;

impl ModelFormPolicy for UploadPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "document" | "avatar")
	}
}

struct UploadFormSchema;

const UPLOAD_FIELDS: [ModelFormFieldDescriptor; 3] = [
	ModelFormFieldDescriptor {
		name: "title",
		kind: ModelFormFieldKind::Text {
			min_length: None,
			max_length: None,
			multiline: false,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
	trim: false,
	},
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
];

impl ModelFormSchema for UploadFormSchema {
	type Model = Upload;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&UPLOAD_FIELDS
	}
}

impl UploadFormSchema {
	const fn title() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[0]
	}

	const fn document() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[1]
	}

	const fn avatar() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[2]
	}
}

struct UploadModelFormData<P: ModelFormPolicy> {
	title: Option<String>,
	_policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> Default for UploadModelFormData<P> {
	fn default() -> Self {
		Self {
			title: None,
			_policy: PhantomData,
		}
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for UploadModelFormData<P> {
	fn supplied_fields(&self) -> Vec<&'static str> {
		self.title.as_ref().map_or_else(Vec::new, |_| vec!["title"])
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"title" => self.title.clone().map(serde_json::Value::String),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		match field {
			"title" => {
				self.title = Some(serde_json::from_value(value).map_err(|error| {
					ModelFormPayloadError::InvalidValue {
						field: field.to_owned(),
						message: error.to_string(),
					}
				})?);
				Ok(())
			}
			_ => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

struct CleanedUploadModelFormData<P: ModelFormPolicy>(UploadModelFormData<P>);

impl<P: ModelFormPolicy> ModelFormCleanedPayload for CleanedUploadModelFormData<P> {
	type Raw = UploadModelFormData<P>;

	fn into_raw(self) -> Self::Raw {
		self.0
	}
}

impl<P: ModelFormPolicy> ModelFormValidatingPayload for UploadModelFormData<P> {
	type Cleaned = CleanedUploadModelFormData<P>;

	fn clean_and_validate(self) -> Result<Self::Cleaned, ValidationErrors> {
		if self.title.as_deref() == Some("Rejected by validation") {
			let mut errors = ValidationErrors::new();
			errors.add(
				"title",
				ValidationError::Custom("Title is rejected".to_owned()),
			);
			errors.add(
				"_all",
				ValidationError::Custom("Upload is rejected".to_owned()),
			);
			return Err(errors);
		}
		Ok(CleanedUploadModelFormData(self))
	}
}

#[server_fn]
async fn upload(
	title: String,
	document: reinhardt_core::parsers::UploadedFile,
	avatar: Option<reinhardt_core::parsers::UploadedFile>,
) -> Result<(), ServerFnError> {
	let _ = (title, document, avatar);
	Ok(())
}

#[server_fn]
async fn upload_wrong_types(
	title: reinhardt_core::parsers::UploadedFile,
	document: String,
	avatar: Option<reinhardt_core::parsers::UploadedFile>,
) -> Result<(), ServerFnError> {
	let _ = (title, document, avatar);
	Ok(())
}

#[server_fn]
async fn upload_wrong_requiredness(
	title: String,
	document: Option<reinhardt_core::parsers::UploadedFile>,
	avatar: reinhardt_core::parsers::UploadedFile,
) -> Result<(), ServerFnError> {
	let _ = (title, document, avatar);
	Ok(())
}
