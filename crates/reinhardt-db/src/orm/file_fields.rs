//! Storage-backed model file field values and descriptors.

mod descriptor;
mod lifecycle;
mod value;

pub use descriptor::ModelFileField;
pub use lifecycle::{
	FileCleanupOperation, FileCommit, FileFieldPolicy, FileMutationError, FileValidationPolicy,
	FileWriteOperation, PendingFileUpload, coordinate_file_mutations,
};
pub use reinhardt_storages::FileStorageError as FileFieldError;
pub use value::FileField;
