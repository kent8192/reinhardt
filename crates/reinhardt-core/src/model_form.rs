//! Target-neutral contracts for model-backed forms.

mod policy;
mod schema;
mod validation;

pub use policy::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	NativeModelFormPayload, normalize_native_model_form_value,
};
pub use schema::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPrimaryKey, ModelFormPrimaryKeyFields,
	ModelFormSchema, ModelFormTableName,
};
pub use validation::{ModelFormCleanedPayload, ModelFormValidatingPayload};
