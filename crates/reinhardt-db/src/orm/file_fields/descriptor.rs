use super::{
	FileCleanupOperation, FileCommit, FileField, FileFieldError, FileFieldPolicy,
	FileMutationError, FileValidationPolicy, FileWriteOperation, PendingFileUpload,
	coordinate_file_mutations,
};
use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::{UploadPolicy, active_storage_registry, store_uploaded_file};
use std::borrow::Cow;
use std::future::Future;
use std::marker::PhantomData;

/// Static upload policy emitted for one model file field.
#[derive(Clone, Copy, Debug)]
pub struct ModelFileField<M> {
	model: &'static str,
	field: &'static str,
	upload_to: &'static str,
	storage_alias: &'static str,
	max_length: usize,
	cleanup: bool,
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
			cleanup: true,
			marker: PhantomData,
		}
	}

	/// Construct a descriptor with an explicit lifecycle cleanup policy.
	///
	/// # Safety
	///
	/// The static policy must describe the same persisted field of `M` as the
	/// generated accessor that returns this descriptor.
	#[doc(hidden)]
	pub const unsafe fn from_model_field_with_cleanup(
		model: &'static str,
		field: &'static str,
		upload_to: &'static str,
		storage_alias: &'static str,
		max_length: usize,
		cleanup: bool,
	) -> Self {
		Self {
			model,
			field,
			upload_to,
			storage_alias,
			max_length,
			cleanup,
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

	/// Return whether old committed files are cleaned after database success.
	#[must_use]
	pub const fn cleanup(&self) -> bool {
		self.cleanup
	}

	/// Return this descriptor's borrowed runtime lifecycle policy.
	#[must_use]
	pub fn policy(&self) -> FileFieldPolicy {
		FileFieldPolicy {
			model: Cow::Borrowed(self.model),
			field: Cow::Borrowed(self.field),
			upload_to: Cow::Borrowed(self.upload_to),
			storage_alias: Cow::Borrowed(self.storage_alias),
			max_length: self.max_length,
			cleanup: self.cleanup,
			validation: FileValidationPolicy::File,
		}
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

	/// Store a new file and persist its model value through one database closure.
	///
	/// The persistence closure must return `Ok` only after its caller-owned
	/// transaction is durably committed.
	pub async fn create_with<T, E, F, Fut>(
		&self,
		upload: UploadedFile,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce(FileField) -> Fut,
		Fut: Future<Output = Result<T, E>>,
	{
		coordinate_file_mutations(
			vec![PendingFileUpload {
				policy: self.policy(),
				operation: FileWriteOperation::Create,
				upload,
			}],
			move |mut stored| async move {
				let stored = stored
					.pop()
					.expect("one staged upload must produce one stored file");
				persist(stored).await.map(FileCommit::new)
			},
		)
		.await
	}

	/// Replace a file and clean the old object after durable database success.
	///
	/// The persistence closure must return `Ok` only after its caller-owned
	/// transaction is durably committed.
	pub async fn replace_with<T, E, F, Fut>(
		&self,
		current: FileField,
		upload: UploadedFile,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce(FileField) -> Fut,
		Fut: Future<Output = Result<T, E>>,
	{
		let policy = self.policy();
		coordinate_file_mutations(
			vec![PendingFileUpload {
				policy: policy.clone(),
				operation: FileWriteOperation::Replace,
				upload,
			}],
			move |mut stored| async move {
				let stored = stored
					.pop()
					.expect("one staged upload must produce one stored file");
				persist(stored).await.map(|value| {
					FileCommit::new(value).cleanup(policy, current, FileCleanupOperation::Replace)
				})
			},
		)
		.await
	}

	/// Clear a nullable file value and clean the old object after database success.
	///
	/// The persistence closure must return `Ok` only after its caller-owned
	/// transaction is durably committed.
	pub async fn clear_with<T, E, F, Fut>(
		&self,
		current: FileField,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce() -> Fut,
		Fut: Future<Output = Result<T, E>>,
	{
		let policy = self.policy();
		coordinate_file_mutations(Vec::new(), move |_| async move {
			persist().await.map(|value| {
				FileCommit::new(value).cleanup(policy, current, FileCleanupOperation::Clear)
			})
		})
		.await
	}

	/// Delete a model value and clean its file after durable database success.
	///
	/// The persistence closure must return `Ok` only after its caller-owned
	/// transaction is durably committed.
	pub async fn delete_with<T, E, F, Fut>(
		&self,
		current: FileField,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce() -> Fut,
		Fut: Future<Output = Result<T, E>>,
	{
		let policy = self.policy();
		coordinate_file_mutations(Vec::new(), move |_| async move {
			persist().await.map(|value| {
				FileCommit::new(value).cleanup(policy, current, FileCleanupOperation::Delete)
			})
		})
		.await
	}
}

#[cfg(test)]
mod tests {
	use super::ModelFileField;
	use std::borrow::Cow;

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
		let policy = descriptor.policy();
		assert_eq!(policy.model, Cow::Borrowed("Profile"));
		assert_eq!(policy.field, Cow::Borrowed("avatar"));
		assert_eq!(policy.upload_to, Cow::Borrowed("avatars/%Y/%m/%d"));
		assert_eq!(policy.storage_alias, Cow::Borrowed("private_uploads"));
		assert!(policy.cleanup);
	}

	#[test]
	fn generated_descriptor_preserves_disabled_cleanup() {
		let descriptor = unsafe {
			ModelFileField::<Profile>::from_model_field_with_cleanup(
				"Profile", "avatar", "avatars", "default", 255, false,
			)
		};

		assert_eq!(descriptor.cleanup(), false);
		assert_eq!(descriptor.policy().cleanup, false);
	}
}
