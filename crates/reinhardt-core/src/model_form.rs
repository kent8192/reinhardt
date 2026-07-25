//! Target-neutral contracts for model-backed forms.

mod policy;
mod schema;

pub use policy::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
};
pub use schema::{ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormSchema};
