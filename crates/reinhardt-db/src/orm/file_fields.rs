//! Storage-backed model file field values and descriptors.

mod descriptor;
#[cfg(feature = "image-fields")]
mod image;
mod lifecycle;
mod value;

pub use descriptor::ModelFileField;
#[cfg(feature = "image-fields")]
pub use image::{ImageField, ModelImageField};
pub use lifecycle::{
	FileCleanupOperation, FileCommit, FileFieldPolicy, FileMutationError, FileValidationPolicy,
	FileWriteOperation, PendingFileUpload, coordinate_file_mutations,
};
pub use reinhardt_storages::FileStorageError as FileFieldError;
pub use value::FileField;
