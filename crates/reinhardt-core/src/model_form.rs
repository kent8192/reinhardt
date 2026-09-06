//! Target-neutral contracts for model-backed forms.

mod policy;
mod schema;

pub use policy::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	NativeModelFormPayload, normalize_native_model_form_value,
};
pub use schema::{
	ModelFormContract, ModelFormContractField, ModelFormContractSchema, ModelFormFieldDescriptor,
	ModelFormFieldKind, ModelFormPrimaryKey, ModelFormPrimaryKeyFields, ModelFormSchema,
	ModelFormTableName,
};
