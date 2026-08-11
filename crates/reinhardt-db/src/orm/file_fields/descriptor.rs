use super::{FileField, FileFieldError};
use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::{UploadPolicy, active_storage_registry, store_uploaded_file};
use std::marker::PhantomData;

/// Static upload policy emitted for one model file field.
#[derive(Clone, Copy, Debug)]
pub struct ModelFileField<M> {
	model: &'static str,
	field: &'static str,
	upload_to: &'static str,
	storage_alias: &'static str,
	max_length: usize,
	marker: PhantomData<fn() -> M>,
}

impl<M> ModelFileField<M> {
	/// Construct a descriptor emitted by the model macro.
	///
	/// # Safety
	///
	/// The static policy must describe the same persisted field of `M` as the
	/// generated accessor that returns this descriptor.
	#[doc(hidden)]
	pub const unsafe fn from_model_field(
		model: &'static str,
		field: &'static str,
		upload_to: &'static str,
		storage_alias: &'static str,
		max_length: usize,
	) -> Self {
		Self {
			model,
			field,
			upload_to,
			storage_alias,
			max_length,
			marker: PhantomData,
		}
	}

	/// Return the owning model name.
	#[must_use]
	pub const fn model(&self) -> &'static str {
		self.model
	}

	/// Return the logical model field name.
	#[must_use]
	pub const fn field(&self) -> &'static str {
		self.field
	}

	/// Return the upload directory template.
	#[must_use]
	pub const fn upload_to(&self) -> &'static str {
		self.upload_to
	}

	/// Return the configured storage alias.
	#[must_use]
	pub const fn storage_alias(&self) -> &'static str {
		self.storage_alias
	}

	/// Return the maximum logical path length.
	#[must_use]
	pub const fn max_length(&self) -> usize {
		self.max_length
	}

	/// Store an upload and return its typed model value.
	pub async fn store(&self, upload: UploadedFile) -> Result<FileField, FileFieldError> {
		let registry = active_storage_registry()?;
		let stored = store_uploaded_file(
			&registry,
			UploadPolicy {
				model: self.model,
				field: self.field,
				upload_to: self.upload_to,
				storage_alias: self.storage_alias,
				max_length: self.max_length,
			},
			upload,
		)
		.await?;
		FileField::from_existing(stored.path, stored.storage_alias)
	}
}

#[cfg(test)]
mod tests {
	use super::ModelFileField;

	struct Profile;

	#[test]
	fn generated_descriptor_preserves_upload_policy() {
		let descriptor = unsafe {
			ModelFileField::<Profile>::from_model_field(
				"Profile",
				"avatar",
				"avatars/%Y/%m/%d",
				"private_uploads",
				255,
			)
		};

		assert_eq!(descriptor.model(), "Profile");
		assert_eq!(descriptor.field(), "avatar");
		assert_eq!(descriptor.upload_to(), "avatars/%Y/%m/%d");
		assert_eq!(descriptor.storage_alias(), "private_uploads");
		assert_eq!(descriptor.max_length(), 255);
	}
}
