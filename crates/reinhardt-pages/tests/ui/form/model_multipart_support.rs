use std::marker::PhantomData;

use reinhardt_core::{
	model_form::{
		ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
		ModelFormPolicy, ModelFormSchema,
	},
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
	},
	ModelFormFieldDescriptor {
		name: "document",
		kind: ModelFormFieldKind::File,
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
	},
	ModelFormFieldDescriptor {
		name: "avatar",
		kind: ModelFormFieldKind::Image,
		required: false,
		has_default: false,
		nullable: true,
		editable: true,
		generated_relation_id: false,
	},
];

impl ModelFormSchema for UploadFormSchema {
	type Model = Upload;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&UPLOAD_FIELDS
	}
}

impl UploadFormSchema {
	fn title() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[0]
	}

	fn document() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[1]
	}

	fn avatar() -> &'static ModelFormFieldDescriptor {
		&UPLOAD_FIELDS[2]
	}
}

struct UploadModelFormData<P: ModelFormPolicy>(PhantomData<P>);

impl<P: ModelFormPolicy> Default for UploadModelFormData<P> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for UploadModelFormData<P> {
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

#[server_fn]
async fn upload(
	title: String,
	document: reinhardt_core::parsers::UploadedFile,
	avatar: Option<reinhardt_core::parsers::UploadedFile>,
) -> Result<(), ServerFnError> {
	let _ = (title, document, avatar);
	Ok(())
}
