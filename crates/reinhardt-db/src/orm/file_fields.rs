//! Storage-backed model file field values and descriptors.

mod descriptor;
mod value;

pub use descriptor::ModelFileField;
pub use reinhardt_storages::FileStorageError as FileFieldError;
pub use value::FileField;
