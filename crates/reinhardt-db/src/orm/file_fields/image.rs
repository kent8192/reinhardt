use super::{
	FileCleanupOperation, FileCommit, FileField, FileFieldError, FileFieldPolicy,
	FileMutationError, FileValidationPolicy, FileWriteOperation, PendingFileUpload,
	coordinate_file_mutations,
};
use crate::orm::field_codec::{DatabaseField, FieldCodecContext, FieldCodecError};
use image::{ImageError, ImageFormat, ImageReader, Limits};
use reinhardt_core::parsers::UploadedFile;
use std::borrow::Cow;
use std::convert::Infallible;
use std::future::Future;
use std::io::Cursor;
use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

/// A validated image reference stored as the same logical path as [`FileField`].
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct ImageField(FileField);

impl ImageField {
	/// Construct a typed image reference to an existing logical storage key.
	pub fn from_existing(
		path: impl Into<String>,
		storage_alias: impl Into<String>,
	) -> Result<Self, FileFieldError> {
		FileField::from_existing(path, storage_alias).map(Self)
	}

	/// Return the portable logical storage key.
	#[must_use]
	pub fn path(&self) -> &str {
		self.0.path()
	}

	/// Return the registry alias used to resolve this value.
	#[must_use]
	pub fn storage_alias(&self) -> &str {
		self.0.storage_alias()
	}

	/// Read the referenced image bytes from the active storage registry.
	pub async fn open(&self) -> Result<Vec<u8>, FileFieldError> {
		self.0.open().await
	}

	/// Return the referenced image size in bytes.
	pub async fn size(&self) -> Result<u64, FileFieldError> {
		self.0.size().await
	}

	/// Generate a URL using the configured expiration for this storage alias.
	pub async fn url(&self) -> Result<String, FileFieldError> {
		self.0.url().await
	}

	/// Generate a URL with an explicit expiration.
	pub async fn url_with_expiry(&self, expiry: Duration) -> Result<String, FileFieldError> {
		self.0.url_with_expiry(expiry).await
	}

	fn into_file(self) -> FileField {
		self.0
	}
}

impl DatabaseField for ImageField {
	type Storage = String;

	fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
		self.0.encode_database()
	}

	fn decode_database(
		value: Self::Storage,
		context: &FieldCodecContext,
	) -> Result<Self, FieldCodecError> {
		FileField::decode_database(value, context).map(Self)
	}

	fn validate_database_context(
		&self,
		context: &FieldCodecContext,
	) -> Result<(), FieldCodecError> {
		self.0.validate_database_context(context)
	}
}

/// Static upload and validation policy emitted for one model image field.
#[derive(Clone, Copy, Debug)]
pub struct ModelImageField<M> {
	model: &'static str,
	field: &'static str,
	upload_to: &'static str,
	storage_alias: &'static str,
	max_length: usize,
	cleanup: bool,
	max_width: Option<u32>,
	max_height: Option<u32>,
	marker: PhantomData<fn() -> M>,
}

impl<M> ModelImageField<M> {
	/// Construct a descriptor emitted by the model macro.
	///
	/// # Safety
	///
	/// The static policy must describe the same persisted field of `M` as the
	/// generated accessor that returns this descriptor.
	#[doc(hidden)]
	// The generated macro call mirrors the field declaration one-for-one.
	#[allow(clippy::too_many_arguments)]
	pub const unsafe fn from_model_field(
		model: &'static str,
		field: &'static str,
		upload_to: &'static str,
		storage_alias: &'static str,
		max_length: usize,
		cleanup: bool,
		max_width: Option<u32>,
		max_height: Option<u32>,
	) -> Self {
		Self {
			model,
			field,
			upload_to,
			storage_alias,
			max_length,
			cleanup,
			max_width,
			max_height,
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

	/// Return whether old committed images are cleaned after database success.
	#[must_use]
	pub const fn cleanup(&self) -> bool {
		self.cleanup
	}

	/// Return the inclusive maximum image width.
	#[must_use]
	pub const fn max_width(&self) -> Option<u32> {
		self.max_width
	}

	/// Return the inclusive maximum image height.
	#[must_use]
	pub const fn max_height(&self) -> Option<u32> {
		self.max_height
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
			validation: FileValidationPolicy::Image {
				max_width: self.max_width,
				max_height: self.max_height,
			},
		}
	}

	/// Validate and store an upload without transforming its bytes.
	pub async fn store(&self, upload: UploadedFile) -> Result<ImageField, FileFieldError> {
		match self
			.create_with(upload, |stored| async move {
				Ok::<ImageField, Infallible>(stored)
			})
			.await
		{
			Ok(stored) => Ok(stored),
			Err(FileMutationError::Storage(error)) => Err(error),
			Err(FileMutationError::Database(never)) => match never {},
		}
	}

	/// Store an image and persist its model value through one database closure.
	pub async fn create_with<T, E, F, Fut>(
		&self,
		upload: UploadedFile,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce(ImageField) -> Fut + Send + 'static,
		Fut: Future<Output = Result<T, E>> + Send + 'static,
		T: Send + 'static,
		E: Send + 'static,
	{
		coordinate_file_mutations(
			vec![PendingFileUpload {
				policy: self.policy(),
				operation: FileWriteOperation::Create,
				upload,
			}],
			move |mut stored| async move {
				let stored = ImageField(
					stored
						.pop()
						.expect("one staged upload must produce one stored image"),
				);
				persist(stored).await.map(FileCommit::new)
			},
		)
		.await
	}

	/// Replace an image and clean the old object after database success.
	pub async fn replace_with<T, E, F, Fut>(
		&self,
		current: ImageField,
		upload: UploadedFile,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce(ImageField) -> Fut + Send + 'static,
		Fut: Future<Output = Result<T, E>> + Send + 'static,
		T: Send + 'static,
		E: Send + 'static,
	{
		let policy = self.policy();
		coordinate_file_mutations(
			vec![PendingFileUpload {
				policy: policy.clone(),
				operation: FileWriteOperation::Replace,
				upload,
			}],
			move |mut stored| async move {
				let stored = ImageField(
					stored
						.pop()
						.expect("one staged upload must produce one stored image"),
				);
				persist(stored).await.map(|value| {
					FileCommit::new(value).cleanup(
						policy,
						current.into_file(),
						FileCleanupOperation::Replace,
					)
				})
			},
		)
		.await
	}

	/// Clear a nullable image value after database success.
	pub async fn clear_with<T, E, F, Fut>(
		&self,
		current: ImageField,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce() -> Fut + Send + 'static,
		Fut: Future<Output = Result<T, E>> + Send + 'static,
		T: Send + 'static,
		E: Send + 'static,
	{
		let policy = self.policy();
		coordinate_file_mutations(Vec::new(), move |_| async move {
			persist().await.map(|value| {
				FileCommit::new(value).cleanup(
					policy,
					current.into_file(),
					FileCleanupOperation::Clear,
				)
			})
		})
		.await
	}

	/// Delete a model value and clean its image after database success.
	pub async fn delete_with<T, E, F, Fut>(
		&self,
		current: ImageField,
		persist: F,
	) -> Result<T, FileMutationError<E>>
	where
		F: FnOnce() -> Fut + Send + 'static,
		Fut: Future<Output = Result<T, E>> + Send + 'static,
		T: Send + 'static,
		E: Send + 'static,
	{
		let policy = self.policy();
		coordinate_file_mutations(Vec::new(), move |_| async move {
			persist().await.map(|value| {
				FileCommit::new(value).cleanup(
					policy,
					current.into_file(),
					FileCleanupOperation::Delete,
				)
			})
		})
		.await
	}
}

pub(super) fn validate_image_upload(
	upload: &UploadedFile,
	max_width: Option<u32>,
	max_height: Option<u32>,
) -> Result<(), FileFieldError> {
	let filename = upload
		.filename
		.as_deref()
		.filter(|filename| !filename.is_empty())
		.ok_or(FileFieldError::MissingFilename)?;
	let extension = Path::new(filename)
		.extension()
		.and_then(|extension| extension.to_str())
		.ok_or_else(|| {
			FileFieldError::InvalidUpload("unsupported image filename extension".to_owned())
		})?;
	let extension_format = ImageFormat::from_extension(extension).ok_or_else(|| {
		FileFieldError::InvalidUpload("unsupported image filename extension".to_owned())
	})?;
	let mut reader = ImageReader::new(Cursor::new(upload.data.as_ref()))
		.with_guessed_format()
		.map_err(|_| {
			FileFieldError::InvalidUpload("upload bytes could not be inspected".to_owned())
		})?;
	let detected_format = reader.format().ok_or_else(|| {
		FileFieldError::InvalidUpload("upload bytes are not a recognized image format".to_owned())
	})?;
	if extension_format != detected_format {
		return Err(FileFieldError::InvalidUpload(
			"filename extension does not match detected image format".to_owned(),
		));
	}
	let mut limits = Limits::default();
	limits.max_image_width = max_width;
	limits.max_image_height = max_height;
	reader.limits(limits);
	match reader.decode() {
		Ok(_) => Ok(()),
		Err(ImageError::Limits(_)) => Err(FileFieldError::InvalidUpload(
			"image exceeds decoder limits".to_owned(),
		)),
		Err(_) => Err(FileFieldError::InvalidUpload(
			"image bytes are corrupt or incomplete".to_owned(),
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::{ImageField, ModelImageField, validate_image_upload};
	use crate::orm::field_codec::{DatabaseField, FieldCodecContext};
	use async_trait::async_trait;
	use chrono::{DateTime, Utc};
	use image::codecs::png::PngEncoder;
	use image::{ExtendedColorType, ImageEncoder};
	use reinhardt_core::parsers::UploadedFile;
	use reinhardt_storages::{
		FileStorageError, StorageBackend, StorageCapabilities, StorageEntry, StorageError,
		StorageRegistry,
	};
	use serial_test::serial;
	use std::sync::{Arc, Mutex};
	use std::time::Duration;

	#[derive(Default)]
	struct RecordingStorage {
		saved: Mutex<Vec<(String, Vec<u8>)>>,
		deleted: Mutex<Vec<String>>,
	}

	#[async_trait]
	impl StorageBackend for RecordingStorage {
		async fn save(&self, name: &str, content: &[u8]) -> Result<String, StorageError> {
			self.save_if_absent(name, content).await
		}

		async fn save_if_absent(&self, name: &str, content: &[u8]) -> Result<String, StorageError> {
			self.saved
				.lock()
				.unwrap()
				.push((name.to_owned(), content.to_vec()));
			Ok(name.to_owned())
		}

		fn capabilities(&self) -> StorageCapabilities {
			StorageCapabilities {
				exclusive_create: true,
			}
		}

		async fn open(&self, name: &str) -> Result<Vec<u8>, StorageError> {
			self.saved
				.lock()
				.unwrap()
				.iter()
				.find(|(path, _)| path == name)
				.map(|(_, bytes)| bytes.clone())
				.ok_or_else(|| StorageError::NotFound(name.to_owned()))
		}

		async fn delete(&self, name: &str) -> Result<(), StorageError> {
			self.deleted.lock().unwrap().push(name.to_owned());
			Ok(())
		}

		async fn exists(&self, name: &str) -> Result<bool, StorageError> {
			Ok(self
				.saved
				.lock()
				.unwrap()
				.iter()
				.any(|(path, _)| path == name))
		}

		async fn url(&self, name: &str, expiry_secs: u64) -> Result<String, StorageError> {
			Ok(format!("recording://{name}?expiry={expiry_secs}"))
		}

		async fn size(&self, name: &str) -> Result<u64, StorageError> {
			self.open(name).await.map(|bytes| bytes.len() as u64)
		}

		async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>, StorageError> {
			Ok(Utc::now())
		}
	}

	struct Profile;

	fn descriptor(
		cleanup: bool,
		max_width: Option<u32>,
		max_height: Option<u32>,
	) -> ModelImageField<Profile> {
		unsafe {
			ModelImageField::from_model_field(
				"Profile", "image", "images", "media", 255, cleanup, max_width, max_height,
			)
		}
	}

	fn png(width: u32, height: u32) -> Vec<u8> {
		let pixels = vec![0x7f; width as usize * height as usize * 4];
		let mut bytes = Vec::new();
		PngEncoder::new(&mut bytes)
			.write_image(&pixels, width, height, ExtendedColorType::Rgba8)
			.unwrap();
		bytes
	}

	fn upload(filename: &str, bytes: Vec<u8>) -> UploadedFile {
		UploadedFile::new("image".to_owned(), bytes.into()).with_filename(filename.to_owned())
	}

	fn invalid_message(
		filename: &str,
		bytes: Vec<u8>,
		max_width: Option<u32>,
		max_height: Option<u32>,
	) -> String {
		match validate_image_upload(&upload(filename, bytes), max_width, max_height).unwrap_err() {
			FileStorageError::InvalidUpload(message) => message,
			other => panic!("unexpected validation error: {other}"),
		}
	}

	fn activate_storage() -> (
		Arc<RecordingStorage>,
		reinhardt_storages::ActiveStorageRegistryGuard,
	) {
		let backend = Arc::new(RecordingStorage::default());
		let entry = StorageEntry::new(backend.clone(), Duration::from_secs(60));
		let registry = Arc::new(
			StorageRegistry::from_entries(entry.clone(), [("media".to_owned(), entry)]).unwrap(),
		);
		let guard = registry.activate().unwrap();
		(backend, guard)
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn matching_png_is_stored_unchanged_with_typed_path_and_alias() {
		let (backend, _guard) = activate_storage();
		let bytes = png(2, 1);
		let value = descriptor(false, Some(2), Some(1))
			.store(upload("avatar.png", bytes.clone()).with_content_type("image/jpeg".to_owned()))
			.await
			.unwrap();

		assert_eq!(value.path(), "images/avatar.png");
		assert_eq!(value.storage_alias(), "media");
		assert_eq!(value.open().await.unwrap(), bytes);
		assert_eq!(value.size().await.unwrap(), bytes.len() as u64);
		assert_eq!(
			value.url().await.unwrap(),
			"recording://images/avatar.png?expiry=60"
		);
		assert_eq!(
			backend.saved.lock().unwrap().as_slice(),
			[("images/avatar.png".to_owned(), bytes)]
		);
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn image_replace_honors_cleanup_policy() {
		let (backend, _guard) = activate_storage();
		let descriptor = descriptor(false, Some(1), Some(1));
		let current = ImageField::from_existing("images/old.png", "media").unwrap();

		let persisted_path = descriptor
			.replace_with(current, upload("new.png", png(1, 1)), |stored| async move {
				Ok::<_, &'static str>(stored.path().to_owned())
			})
			.await
			.unwrap();

		assert_eq!(persisted_path, "images/new.png");
		assert!(backend.deleted.lock().unwrap().is_empty());
		let policy = descriptor.policy();
		assert!(!policy.cleanup);
		assert_eq!(descriptor.max_width(), Some(1));
		assert_eq!(descriptor.max_height(), Some(1));
	}

	#[test]
	fn image_database_codec_remains_a_path_column() {
		let image = ImageField::from_existing("images/avatar.png", "media").unwrap();
		let context = FieldCodecContext::new("Profile", "image", "image_path")
			.with_metadata("file_storage", "media")
			.with_metadata("file_max_length", "255");

		assert_eq!(image.encode_database().unwrap(), "images/avatar.png");
		assert_eq!(
			ImageField::decode_database("images/avatar.png".to_owned(), &context).unwrap(),
			image
		);
	}

	#[test]
	fn image_validation_accepts_dimensions_at_inclusive_limit() {
		validate_image_upload(&upload("avatar.png", png(2, 1)), Some(2), Some(1)).unwrap();
	}

	#[test]
	fn image_validation_rejects_width_one_pixel_over_limit() {
		assert_eq!(
			invalid_message("avatar.png", png(3, 1), Some(2), Some(1)),
			"image exceeds decoder limits"
		);
	}

	#[test]
	fn image_validation_rejects_decoder_allocation_limit() {
		assert_eq!(
			invalid_message(
				"oversized.ppm",
				b"P6\n50000 50000\n255\n".to_vec(),
				None,
				None,
			),
			"image exceeds decoder limits"
		);
	}

	#[test]
	fn image_validation_rejects_mismatched_extension() {
		assert_eq!(
			invalid_message("avatar.jpg", png(1, 1), None, None),
			"filename extension does not match detected image format"
		);
	}

	#[test]
	fn image_validation_rejects_unknown_corrupt_bytes() {
		assert_eq!(
			invalid_message("avatar.png", b"not an image".to_vec(), None, None),
			"upload bytes are not a recognized image format"
		);
	}

	#[test]
	fn image_validation_rejects_truncated_image_bytes() {
		let mut bytes = png(1, 1);
		bytes.truncate(bytes.len() / 2);
		assert_eq!(
			invalid_message("avatar.png", bytes, None, None),
			"image bytes are corrupt or incomplete"
		);
	}

	#[test]
	fn image_validation_rejects_svg() {
		assert_eq!(
			invalid_message(
				"avatar.svg",
				br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
				None,
				None,
			),
			"unsupported image filename extension"
		);
	}
}
