//! Target-neutral contracts for model-backed forms.

mod policy;
mod schema;
#[cfg(feature = "validators")]
mod validation;

pub use policy::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	NativeModelFormPayload, normalize_native_model_form_value,
};
pub use schema::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPrimaryKey, ModelFormPrimaryKeyFields,
	ModelFormSchema, ModelFormTableName,
};
#[cfg(feature = "validators")]
pub use validation::{
	ModelFormCleanedPayload, ModelFormUpdatingPayload, ModelFormValidatingPayload,
};
