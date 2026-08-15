use super::{FileField, FileFieldError};
use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::upload::store_uploaded_file_with_adoption;
use reinhardt_storages::{
	StorageError, StorageRegistry, StoredObjectAdoption, active_storage_registry,
};
use std::{
	borrow::Cow,
	future::Future,
	mem,
	sync::{Arc, Mutex},
};

/// Runtime policy for one storage-backed model field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFieldPolicy {
	/// Model type that owns the field.
	pub model: Cow<'static, str>,
	/// Logical model field name.
	pub field: Cow<'static, str>,
	/// Relative UTC-aware upload directory template.
	pub upload_to: Cow<'static, str>,
	/// Registry alias for the target storage backend.
	pub storage_alias: Cow<'static, str>,
	/// Maximum logical-key length in Unicode scalar values.
	pub max_length: usize,
	/// Whether exclusively owned committed files are removed after database success.
	///
	/// This must remain disabled when another database value may reference the
	/// same storage alias and logical path.
	pub cleanup: bool,
	/// Validation applied before the upload reaches storage.
	pub validation: FileValidationPolicy,
}

/// Validation policy for a staged file upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileValidationPolicy {
	/// Accept an ordinary file without content-specific validation.
	File,
	/// Require a decodable raster image within optional dimension limits.
	#[cfg(feature = "image-fields")]
	Image {
		/// Inclusive maximum image width.
		max_width: Option<u32>,
		/// Inclusive maximum image height.
		max_height: Option<u32>,
	},
}

/// Database write that will reference a newly stored file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileWriteOperation {
	/// Create a new file reference.
	Create,
	/// Replace an existing file reference.
	Replace,
}

impl FileWriteOperation {
	fn compensation_name(self) -> &'static str {
		match self {
			Self::Create => "create_compensation",
			Self::Replace => "replace_compensation",
		}
	}
}

/// One upload staged before a shared database closure.
#[derive(Clone, Debug)]
pub struct PendingFileUpload {
	/// Field policy used for validation, storage, and cleanup diagnostics.
	pub policy: FileFieldPolicy,
	/// Write operation used for compensation diagnostics.
	pub operation: FileWriteOperation,
	/// Uploaded bytes and client metadata.
	pub upload: UploadedFile,
}

/// Committed database operation that releases an old file reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCleanupOperation {
	/// Replace an old file reference.
	Replace,
	/// Clear a nullable file reference.
	Clear,
	/// Delete the row that owned a file reference.
	Delete,
}

impl FileCleanupOperation {
	fn cleanup_name(self) -> &'static str {
		match self {
			Self::Replace => "replace_cleanup",
			Self::Clear => "clear_cleanup",
			Self::Delete => "delete_cleanup",
		}
	}
}

/// Successful database value plus old files eligible for cleanup.
#[derive(Debug)]
pub struct FileCommit<T> {
	/// Value returned to the lifecycle caller.
	pub value: T,
	cleanup: Vec<CommittedFileCleanup>,
}

impl<T> FileCommit<T> {
	/// Create a successful database outcome without old-file cleanup.
	#[must_use]
	pub fn new(value: T) -> Self {
		Self {
			value,
			cleanup: Vec::new(),
		}
	}

	/// Add an exclusively owned old file for best-effort cleanup when permitted.
	#[must_use]
	pub fn cleanup(
		mut self,
		policy: FileFieldPolicy,
		file: FileField,
		operation: FileCleanupOperation,
	) -> Self {
		if policy.cleanup {
			self.cleanup.push(CommittedFileCleanup {
				policy,
				file,
				operation,
			});
		}
		self
	}
}

/// Primary failure returned by file lifecycle coordination.
#[derive(Debug, thiserror::Error)]
pub enum FileMutationError<E> {
	/// Upload validation or storage failed.
	#[error("file storage mutation failed: {0}")]
	Storage(#[from] FileFieldError),
	/// Upload validation or storage failed for one coordinated model field.
	#[error("file storage mutation for field '{field}' failed: {source}")]
	StorageForField {
		/// Logical model field whose upload failed.
		field: String,
		/// Underlying validation or storage failure.
		#[source]
		source: FileFieldError,
	},
	/// The caller-owned database operation failed.
	#[error("database mutation failed: {0}")]
	Database(E),
}

#[derive(Debug)]
struct CommittedFileCleanup {
	policy: FileFieldPolicy,
	file: FileField,
	operation: FileCleanupOperation,
}

#[derive(Debug)]
struct StoredFileUpload {
	policy: FileFieldPolicy,
	operation: FileWriteOperation,
	storage_alias: String,
	path: String,
}

impl StoredFileUpload {
	fn matches(&self, file: &FileField) -> bool {
		self.storage_alias == file.storage_alias() && self.path == file.path()
	}
}

struct UnstagedUploadGuard {
	registry: Arc<StorageRegistry>,
	cleanup_runtime: tokio::runtime::Handle,
	policy: FileFieldPolicy,
	operation: FileWriteOperation,
	storage_alias: String,
	upload: Mutex<Option<StoredFileUpload>>,
}

impl UnstagedUploadGuard {
	fn new(
		registry: Arc<StorageRegistry>,
		policy: FileFieldPolicy,
		operation: FileWriteOperation,
	) -> Self {
		Self {
			registry,
			cleanup_runtime: tokio::runtime::Handle::current(),
			storage_alias: policy.storage_alias.to_string(),
			policy,
			operation,
			upload: Mutex::new(None),
		}
	}

	fn transfer(
		&self,
		stored_path: &str,
		stored_alias: &str,
	) -> Result<StoredFileUpload, FileFieldError> {
		let mut upload = self
			.upload
			.lock()
			.unwrap_or_else(|error| error.into_inner());
		if upload
			.as_ref()
			.is_none_or(|owned| owned.path != stored_path || owned.storage_alias != stored_alias)
		{
			return Err(FileFieldError::InvalidUpload(
				"storage backend did not adopt the stored object".to_owned(),
			));
		}
		Ok(upload.take().expect("stored upload ownership was checked"))
	}
}

impl StoredObjectAdoption for UnstagedUploadGuard {
	fn adopt(&self, path: &str) {
		let mut upload = self
			.upload
			.lock()
			.unwrap_or_else(|error| error.into_inner());
		if upload.is_none() {
			*upload = Some(StoredFileUpload {
				policy: self.policy.clone(),
				operation: self.operation,
				storage_alias: self.storage_alias.clone(),
				path: path.to_owned(),
			});
		}
	}
}

impl Drop for UnstagedUploadGuard {
	fn drop(&mut self) {
		let upload = self
			.upload
			.get_mut()
			.unwrap_or_else(|error| error.into_inner())
			.take();
		let Some(upload) = upload else {
			return;
		};
		let registry = Arc::clone(&self.registry);

		// The backend transferred ownership before its save future yielded.
		// Async compensation is delegated without blocking this destructor.
		mem::drop(self.cleanup_runtime.spawn(async move {
			cleanup_stored_upload(&registry, &upload).await;
		}));
	}
}

struct StagedUploadGuard {
	registry: Arc<StorageRegistry>,
	cleanup_runtime: tokio::runtime::Handle,
	uploads: Vec<StoredFileUpload>,
}

impl StagedUploadGuard {
	fn new(registry: Arc<StorageRegistry>, capacity: usize) -> Self {
		Self {
			registry,
			cleanup_runtime: tokio::runtime::Handle::current(),
			uploads: Vec::with_capacity(capacity),
		}
	}

	fn stage(&mut self, upload: StoredFileUpload) -> Result<FileField, FileFieldError> {
		self.uploads.push(upload);
		let upload = &self.uploads[self.uploads.len() - 1];
		FileField::from_existing(&upload.path, &upload.storage_alias)
	}

	async fn compensate(&mut self) {
		while let Some(upload) = self.uploads.last() {
			cleanup_stored_upload(&self.registry, upload).await;
			self.uploads.pop();
		}
	}

	fn disarm(&mut self) -> Vec<StoredFileUpload> {
		mem::take(&mut self.uploads)
	}
}

impl Drop for StagedUploadGuard {
	fn drop(&mut self) {
		let uploads = mem::take(&mut self.uploads);
		if uploads.is_empty() {
			return;
		}
		let registry = Arc::clone(&self.registry);

		// The runtime handle is captured before the first upload. Drop only
		// transfers ownership to an async task and never blocks on storage I/O.
		mem::drop(self.cleanup_runtime.spawn(async move {
			compensate_uploads(&registry, &uploads).await;
		}));
	}
}

/// Store all staged files around one caller-owned database operation.
///
/// The persistence closure must return `Ok` only after its transaction is
/// durably committed. The persistence task continues to a definitive outcome
/// after caller cancellation, so staged files are not removed while the
/// database commit is still in doubt. Old-file cleanup begins immediately
/// after a successful result.
///
/// # Panics
///
/// Panics when polled outside a Tokio runtime. The runtime handle is acquired
/// before storage begins so cancellation and unwinding can schedule async
/// compensation from the staged-upload guard.
pub async fn coordinate_file_mutations<T, E, F, Fut>(
	writes: Vec<PendingFileUpload>,
	persist: F,
) -> Result<T, FileMutationError<E>>
where
	F: FnOnce(Vec<FileField>) -> Fut + Send + 'static,
	Fut: Future<Output = Result<FileCommit<T>, E>> + Send + 'static,
	T: Send + 'static,
	E: Send + 'static,
{
	let registry = if writes.is_empty() {
		None
	} else {
		Some(active_storage_registry().map_err(FileMutationError::Storage)?)
	};
	let mut staged_uploads = registry
		.as_ref()
		.map(|registry| StagedUploadGuard::new(Arc::clone(registry), writes.len()));
	let mut stored_values = Vec::with_capacity(writes.len());

	for write in writes {
		let registry = registry
			.as_ref()
			.expect("non-empty writes must have an active storage registry");
		let staged_uploads = staged_uploads
			.as_mut()
			.expect("non-empty writes must have a staged-upload guard");
		if let Err(error) = validate_upload(&write.policy.validation, &write.upload).await {
			staged_uploads.compensate().await;
			return Err(FileMutationError::StorageForField {
				field: write.policy.field.to_string(),
				source: error,
			});
		}

		let adoption = Arc::new(UnstagedUploadGuard::new(
			Arc::clone(registry),
			write.policy.clone(),
			write.operation,
		));
		let adoption_protocol: Arc<dyn StoredObjectAdoption> = adoption.clone();
		let stored = match store_uploaded_file_with_adoption(
			registry,
			write.policy.model.as_ref(),
			write.policy.field.as_ref(),
			write.policy.upload_to.as_ref(),
			write.policy.storage_alias.as_ref(),
			write.policy.max_length,
			write.upload,
			adoption_protocol,
		)
		.await
		{
			Ok(stored) => stored,
			Err(error) => {
				staged_uploads.compensate().await;
				return Err(FileMutationError::StorageForField {
					field: write.policy.field.to_string(),
					source: error,
				});
			}
		};

		let owned = match adoption.transfer(&stored.path, &stored.storage_alias) {
			Ok(owned) => owned,
			Err(error) => {
				staged_uploads.compensate().await;
				return Err(FileMutationError::StorageForField {
					field: write.policy.field.to_string(),
					source: error,
				});
			}
		};
		let file = match staged_uploads.stage(owned) {
			Ok(file) => file,
			Err(error) => {
				staged_uploads.compensate().await;
				return Err(FileMutationError::StorageForField {
					field: write.policy.field.to_string(),
					source: error,
				});
			}
		};
		stored_values.push(file);
	}

	let commit_task = tokio::spawn(async move {
		let commit = match persist(stored_values).await {
			Ok(commit) => commit,
			Err(error) => {
				if let Some(mut staged_uploads) = staged_uploads {
					staged_uploads.compensate().await;
				}
				return Err(FileMutationError::Database(error));
			}
		};
		let stored_uploads = staged_uploads
			.map(|mut staged_uploads| staged_uploads.disarm())
			.unwrap_or_default();
		let cleanup_registry = match (registry, commit.cleanup.is_empty()) {
			(Some(registry), _) => Some(registry),
			(None, true) => None,
			(None, false) => match active_storage_registry() {
				Ok(registry) => Some(registry),
				Err(error) => {
					let error_message = error.to_string();
					tracing::error!(
						error = error_message,
						"Committed file cleanup skipped because the active storage registry is unavailable"
					);
					None
				}
			},
		};

		if let Some(registry) = cleanup_registry.as_deref() {
			for cleanup in commit.cleanup {
				if stored_uploads
					.iter()
					.any(|stored| stored.matches(&cleanup.file))
				{
					continue;
				}
				cleanup_file(
					registry,
					cleanup.operation.cleanup_name(),
					&cleanup.policy,
					cleanup.file.storage_alias(),
					cleanup.file.path(),
				)
				.await;
			}
		}

		Ok(commit.value)
	});

	commit_task.await.map_err(|error| {
		FileMutationError::Storage(FileFieldError::Storage(StorageError::Other(format!(
			"file mutation task failed: {error}"
		))))
	})?
}

async fn validate_upload(
	policy: &FileValidationPolicy,
	upload: &UploadedFile,
) -> Result<(), FileFieldError> {
	match policy {
		FileValidationPolicy::File => Ok(()),
		#[cfg(feature = "image-fields")]
		FileValidationPolicy::Image {
			max_width,
			max_height,
		} => {
			let max_width = *max_width;
			let max_height = *max_height;
			let upload = upload.clone();
			tokio::task::spawn_blocking(move || {
				super::image::validate_image_upload(&upload, max_width, max_height)
			})
			.await
			.map_err(|error| {
				FileFieldError::Storage(StorageError::Other(format!(
					"image validation task failed: {error}"
				)))
			})?
		}
	}
}

async fn compensate_uploads(registry: &StorageRegistry, uploads: &[StoredFileUpload]) {
	for upload in uploads.iter().rev() {
		cleanup_stored_upload(registry, upload).await;
	}
}

async fn cleanup_stored_upload(registry: &StorageRegistry, upload: &StoredFileUpload) {
	cleanup_file(
		registry,
		upload.operation.compensation_name(),
		&upload.policy,
		&upload.storage_alias,
		&upload.path,
	)
	.await;
}

async fn cleanup_file(
	registry: &StorageRegistry,
	operation: &'static str,
	policy: &FileFieldPolicy,
	storage_alias: &str,
	path: &str,
) {
	let result = match registry.backend(storage_alias) {
		Ok(backend) => backend.delete(path).await.map_err(FileFieldError::from),
		Err(error) => Err(error),
	};
	if let Err(error) = result {
		tracing::error!(
			operation,
			model = policy.model.as_ref(),
			field = policy.field.as_ref(),
			storage_alias,
			path,
			error = %error,
			"File field cleanup failed"
		);
	}
}

#[cfg(test)]
mod tests {
	use super::{
		FileCleanupOperation, FileCommit, FileFieldPolicy, FileMutationError, FileValidationPolicy,
		FileWriteOperation, PendingFileUpload, coordinate_file_mutations,
	};
	use crate::orm::{FileField, ModelFileField};
	use async_trait::async_trait;
	use chrono::{DateTime, Utc};
	use reinhardt_core::parsers::UploadedFile;
	use reinhardt_storages::{
		ActiveStorageRegistryGuard, StorageBackend, StorageCapabilities, StorageEntry,
		StorageError, StorageRegistry, StoredObjectAdoption, active_storage_registry,
	};
	use rstest::{fixture, rstest};
	use serial_test::serial;
	use std::borrow::Cow;
	use std::collections::{BTreeMap, BTreeSet};
	use std::fmt;
	use std::sync::atomic::{AtomicBool, Ordering};
	use std::sync::{Arc, Mutex};
	use std::time::Duration;
	use tokio::sync::{Notify, oneshot};
	use tracing::field::{Field, Visit};
	use tracing_subscriber::layer::{Context, Layer};
	use tracing_subscriber::prelude::*;
	use tracing_subscriber::registry::LookupSpan;

	type Events = Arc<Mutex<Vec<String>>>;

	struct EventStorage {
		alias: &'static str,
		events: Events,
		files: Mutex<BTreeMap<String, Vec<u8>>>,
		store_failures: Mutex<BTreeSet<String>>,
		delete_failures: Mutex<BTreeSet<String>>,
		pause_after_store: AtomicBool,
		store_completed: Notify,
		store_release: Notify,
		delete_completed: Notify,
	}

	impl EventStorage {
		fn new(alias: &'static str, events: Events) -> Self {
			Self {
				alias,
				events,
				files: Mutex::new(BTreeMap::new()),
				store_failures: Mutex::new(BTreeSet::new()),
				delete_failures: Mutex::new(BTreeSet::new()),
				pause_after_store: AtomicBool::new(false),
				store_completed: Notify::new(),
				store_release: Notify::new(),
				delete_completed: Notify::new(),
			}
		}

		fn fail_store(&self, path: &str) {
			self.store_failures.lock().unwrap().insert(path.to_owned());
		}

		fn fail_delete(&self, path: &str) {
			self.delete_failures.lock().unwrap().insert(path.to_owned());
		}

		fn contains(&self, path: &str) -> bool {
			self.files.lock().unwrap().contains_key(path)
		}

		fn pause_store_after_creation(&self) {
			self.pause_after_store.store(true, Ordering::Release);
		}

		fn record(&self, operation: &str, path: &str) {
			self.events
				.lock()
				.unwrap()
				.push(format!("{operation} {} {path}", self.alias));
		}

		async fn wait_for_delete(&self) {
			self.delete_completed.notified().await;
		}

		async fn wait_for_store_completion(&self) {
			self.store_completed.notified().await;
		}

		fn release_store(&self) {
			self.store_release.notify_one();
		}
	}

	#[async_trait]
	impl StorageBackend for EventStorage {
		async fn save(&self, name: &str, content: &[u8]) -> Result<String, StorageError> {
			self.files
				.lock()
				.unwrap()
				.insert(name.to_owned(), content.to_vec());
			Ok(name.to_owned())
		}

		async fn save_if_absent(&self, name: &str, content: &[u8]) -> Result<String, StorageError> {
			self.record("store", name);
			if self.store_failures.lock().unwrap().contains(name) {
				return Err(StorageError::PermissionDenied(format!("store {name}")));
			}

			{
				let mut files = self.files.lock().unwrap();
				if files.contains_key(name) {
					return Err(StorageError::AlreadyExists(name.to_owned()));
				}
				files.insert(name.to_owned(), content.to_vec());
			}
			if self.pause_after_store.load(Ordering::Acquire) {
				self.store_completed.notify_one();
				self.store_release.notified().await;
			}
			Ok(name.to_owned())
		}

		async fn save_if_absent_with_adoption(
			&self,
			name: &str,
			content: &[u8],
			adoption: Arc<dyn StoredObjectAdoption>,
		) -> Result<String, StorageError> {
			self.record("store", name);
			if self.store_failures.lock().unwrap().contains(name) {
				return Err(StorageError::PermissionDenied(format!("store {name}")));
			}

			{
				let mut files = self.files.lock().unwrap();
				if files.contains_key(name) {
					return Err(StorageError::AlreadyExists(name.to_owned()));
				}
				files.insert(name.to_owned(), content.to_vec());
			}
			adoption.adopt(name);
			if self.pause_after_store.load(Ordering::Acquire) {
				self.store_completed.notify_one();
				self.store_release.notified().await;
			}
			Ok(name.to_owned())
		}

		fn capabilities(&self) -> StorageCapabilities {
			StorageCapabilities {
				exclusive_create: true,
			}
		}

		async fn open(&self, name: &str) -> Result<Vec<u8>, StorageError> {
			self.files
				.lock()
				.unwrap()
				.get(name)
				.cloned()
				.ok_or_else(|| StorageError::NotFound(name.to_owned()))
		}

		async fn delete(&self, name: &str) -> Result<(), StorageError> {
			self.record("delete", name);
			if self.delete_failures.lock().unwrap().contains(name) {
				return Err(StorageError::PermissionDenied(format!("delete {name}")));
			}
			self.files.lock().unwrap().remove(name);
			self.delete_completed.notify_one();
			Ok(())
		}

		async fn exists(&self, name: &str) -> Result<bool, StorageError> {
			Ok(self.contains(name))
		}

		async fn url(&self, name: &str, _expiry_secs: u64) -> Result<String, StorageError> {
			Ok(format!("memory://{}/{name}", self.alias))
		}

		async fn size(&self, name: &str) -> Result<u64, StorageError> {
			self.files
				.lock()
				.unwrap()
				.get(name)
				.map(|content| content.len() as u64)
				.ok_or_else(|| StorageError::NotFound(name.to_owned()))
		}

		async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>, StorageError> {
			Ok(Utc::now())
		}
	}

	struct LifecycleContext {
		events: Events,
		backend: Arc<EventStorage>,
		_registry_guard: ActiveStorageRegistryGuard,
	}

	#[fixture]
	fn context() -> LifecycleContext {
		let events = Arc::new(Mutex::new(Vec::new()));
		let backend = Arc::new(EventStorage::new("default", Arc::clone(&events)));
		let registry = Arc::new(
			StorageRegistry::from_entries(
				StorageEntry::new(backend.clone(), Duration::from_secs(60)),
				std::iter::empty::<(String, StorageEntry)>(),
			)
			.unwrap(),
		);
		let registry_guard = registry.activate().unwrap();
		LifecycleContext {
			events,
			backend,
			_registry_guard: registry_guard,
		}
	}

	struct Profile;

	fn descriptor() -> ModelFileField<Profile> {
		unsafe { ModelFileField::from_model_field("Profile", "avatar", "uploads", "default", 255) }
	}

	fn policy(field: &'static str, cleanup: bool) -> FileFieldPolicy {
		FileFieldPolicy {
			model: Cow::Borrowed("Profile"),
			field: Cow::Borrowed(field),
			upload_to: Cow::Borrowed("uploads"),
			storage_alias: Cow::Borrowed("default"),
			max_length: 255,
			cleanup,
			validation: FileValidationPolicy::File,
		}
	}

	fn owned_policy(field: &str, cleanup: bool) -> FileFieldPolicy {
		FileFieldPolicy {
			model: Cow::Owned("Profile".to_owned()),
			field: Cow::Owned(field.to_owned()),
			upload_to: Cow::Owned("uploads".to_owned()),
			storage_alias: Cow::Owned("default".to_owned()),
			max_length: 255,
			cleanup,
			validation: FileValidationPolicy::File,
		}
	}

	fn upload(filename: &str) -> UploadedFile {
		UploadedFile::new(filename.to_owned(), filename.as_bytes().to_vec().into())
			.with_filename(filename.to_owned())
	}

	fn upload_without_filename() -> UploadedFile {
		UploadedFile::new("missing".to_owned(), b"missing".to_vec().into())
	}

	fn file(path: &str) -> FileField {
		FileField::from_existing(path, "default").unwrap()
	}

	fn record_database(events: &Events) {
		events.lock().unwrap().push("database".to_owned());
	}

	fn actual_events(events: &Events) -> Vec<String> {
		events.lock().unwrap().clone()
	}

	#[derive(Debug, Default, PartialEq, Eq)]
	struct CleanupLog {
		operation: Option<String>,
		model: Option<String>,
		field: Option<String>,
		storage_alias: Option<String>,
		path: Option<String>,
		error: Option<String>,
	}

	#[derive(Clone)]
	struct CleanupLogLayer {
		logs: Arc<Mutex<Vec<CleanupLog>>>,
	}

	#[derive(Default)]
	struct CleanupLogVisitor {
		log: CleanupLog,
	}

	impl CleanupLogVisitor {
		fn record(&mut self, field: &Field, value: String) {
			match field.name() {
				"operation" => self.log.operation = Some(value),
				"model" => self.log.model = Some(value),
				"field" => self.log.field = Some(value),
				"storage_alias" => self.log.storage_alias = Some(value),
				"path" => self.log.path = Some(value),
				"error" => self.log.error = Some(value),
				_ => {}
			}
		}
	}

	impl Visit for CleanupLogVisitor {
		fn record_str(&mut self, field: &Field, value: &str) {
			self.record(field, value.to_owned());
		}

		fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
			self.record(field, value.to_string());
		}

		fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
			self.record(field, format!("{value:?}"));
		}
	}

	impl<S> Layer<S> for CleanupLogLayer
	where
		S: tracing::Subscriber + for<'span> LookupSpan<'span>,
	{
		fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
			let mut visitor = CleanupLogVisitor::default();
			event.record(&mut visitor);
			self.logs.lock().unwrap().push(visitor.log);
		}
	}

	fn expected_log(operation: &str, field: &str, path: &str) -> CleanupLog {
		CleanupLog {
			operation: Some(operation.to_owned()),
			model: Some("Profile".to_owned()),
			field: Some(field.to_owned()),
			storage_alias: Some("default".to_owned()),
			path: Some(path.to_owned()),
			error: Some(format!("Permission denied: delete {path}")),
		}
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn cancellation_after_store_before_stage_compensates_the_new_file(
		context: LifecycleContext,
	) {
		context.backend.pause_store_after_creation();
		let writes = vec![PendingFileUpload {
			policy: policy("avatar", true),
			operation: FileWriteOperation::Create,
			upload: upload("new.txt"),
		}];
		let task = tokio::spawn(coordinate_file_mutations(writes, |_| async move {
			Ok::<_, &'static str>(FileCommit::new(()))
		}));
		context.backend.wait_for_store_completion().await;
		assert!(context.backend.contains("uploads/new.txt"));

		task.abort();
		assert!(task.await.unwrap_err().is_cancelled());
		context.backend.release_store();
		tokio::time::timeout(Duration::from_secs(1), context.backend.wait_for_delete())
			.await
			.expect("post-store cancellation compensation must complete");

		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/new.txt",
				"delete default uploads/new.txt",
			]
		);
		assert!(!context.backend.contains("uploads/new.txt"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn cancellation_during_database_persist_keeps_staged_upload_until_outcome(
		context: LifecycleContext,
	) {
		let events = Arc::clone(&context.events);
		let writes = vec![PendingFileUpload {
			policy: policy("avatar", true),
			operation: FileWriteOperation::Create,
			upload: upload("new.txt"),
		}];
		let (persist_started, persist_started_receiver) = oneshot::channel();
		let (persist_outcome_sender, persist_outcome_receiver) = oneshot::channel();
		let (persist_finished_sender, persist_finished_receiver) = oneshot::channel();
		let task = tokio::spawn(coordinate_file_mutations(writes, move |_| async move {
			record_database(&events);
			assert_eq!(persist_started.send(()), Ok(()));
			persist_outcome_receiver.await.unwrap();
			assert_eq!(persist_finished_sender.send(()), Ok(()));
			Ok::<_, &'static str>(FileCommit::new(()))
		}));
		persist_started_receiver.await.unwrap();

		task.abort();
		assert!(task.await.unwrap_err().is_cancelled());
		assert!(context.backend.contains("uploads/new.txt"));
		assert_eq!(persist_outcome_sender.send(()), Ok(()));
		tokio::time::timeout(Duration::from_secs(1), persist_finished_receiver)
			.await
			.expect("the detached persistence task must reach a definitive outcome")
			.expect("the persistence task must report completion");

		assert_eq!(
			actual_events(&context.events),
			["store default uploads/new.txt", "database"]
		);
		assert!(context.backend.contains("uploads/new.txt"));
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn cleanup_false_mutation_does_not_require_active_registry() {
		assert!(active_storage_registry().is_err());
		let result = coordinate_file_mutations(Vec::new(), |_| async move {
			Ok::<_, &'static str>(FileCommit::new("cleared").cleanup(
				policy("avatar", false),
				file("uploads/old.txt"),
				FileCleanupOperation::Clear,
			))
		})
		.await;

		assert_eq!(result.unwrap(), "cleared");
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn create_database_failure_compensates_the_new_file(context: LifecycleContext) {
		context.backend.fail_delete("uploads/new.txt");
		let events = Arc::clone(&context.events);
		let logs = Arc::new(Mutex::new(Vec::new()));
		let subscriber = tracing_subscriber::registry().with(CleanupLogLayer {
			logs: Arc::clone(&logs),
		});
		let _subscriber_guard = tracing::subscriber::set_default(subscriber);

		let result = descriptor()
			.create_with(upload("new.txt"), move |_stored| {
				let events = Arc::clone(&events);
				async move {
					record_database(&events);
					Err::<(), _>("database failed")
				}
			})
			.await;

		match result {
			Err(FileMutationError::Database(error)) => assert_eq!(error, "database failed"),
			other => panic!("expected database failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/new.txt",
				"database",
				"delete default uploads/new.txt",
			]
		);
		assert_eq!(
			*logs.lock().unwrap(),
			[expected_log(
				"create_compensation",
				"avatar",
				"uploads/new.txt",
			)]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn replace_database_failure_compensates_new_and_keeps_old(context: LifecycleContext) {
		let events = Arc::clone(&context.events);

		let result = descriptor()
			.replace_with(file("uploads/old.txt"), upload("new.txt"), move |_stored| {
				let events = Arc::clone(&events);
				async move {
					record_database(&events);
					Err::<(), _>("database failed")
				}
			})
			.await;

		match result {
			Err(FileMutationError::Database(error)) => assert_eq!(error, "database failed"),
			other => panic!("expected database failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/new.txt",
				"database",
				"delete default uploads/new.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn replace_success_cleans_old_after_database(context: LifecycleContext) {
		let events = Arc::clone(&context.events);

		let result = descriptor()
			.replace_with(file("uploads/old.txt"), upload("new.txt"), move |stored| {
				let events = Arc::clone(&events);
				async move {
					record_database(&events);
					Ok::<_, &'static str>(stored.path().to_owned())
				}
			})
			.await;

		assert_eq!(result.unwrap(), "uploads/new.txt");
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/new.txt",
				"database",
				"delete default uploads/old.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn clear_runs_database_before_old_cleanup(context: LifecycleContext) {
		let events = Arc::clone(&context.events);

		let result = descriptor()
			.clear_with(file("uploads/old.txt"), move || {
				let events = Arc::clone(&events);
				async move {
					record_database(&events);
					Ok::<_, &'static str>("cleared")
				}
			})
			.await;

		assert_eq!(result.unwrap(), "cleared");
		assert_eq!(
			actual_events(&context.events),
			["database", "delete default uploads/old.txt"]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn delete_runs_database_before_old_cleanup(context: LifecycleContext) {
		let events = Arc::clone(&context.events);

		let result = descriptor()
			.delete_with(file("uploads/old.txt"), move || {
				let events = Arc::clone(&events);
				async move {
					record_database(&events);
					Ok::<_, &'static str>("deleted")
				}
			})
			.await;

		assert_eq!(result.unwrap(), "deleted");
		assert_eq!(
			actual_events(&context.events),
			["database", "delete default uploads/old.txt"]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn cleanup_false_suppresses_committed_cleanup(context: LifecycleContext) {
		let cleanup_policy = policy("avatar", false);
		let events = Arc::clone(&context.events);

		let result = coordinate_file_mutations(Vec::new(), move |_| {
			let events = Arc::clone(&events);
			async move {
				record_database(&events);
				Ok::<_, &'static str>(FileCommit::new("saved").cleanup(
					cleanup_policy,
					file("uploads/old.txt"),
					FileCleanupOperation::Replace,
				))
			}
		})
		.await;

		assert_eq!(result.unwrap(), "saved");
		assert_eq!(actual_events(&context.events), ["database"]);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn cleanup_false_never_suppresses_compensation(context: LifecycleContext) {
		let write_policy = policy("avatar", false);
		let cleanup_policy = write_policy.clone();
		let events = Arc::clone(&context.events);
		let writes = vec![PendingFileUpload {
			policy: write_policy,
			operation: FileWriteOperation::Replace,
			upload: upload("new.txt"),
		}];

		let result = coordinate_file_mutations(writes, move |_| {
			let events = Arc::clone(&events);
			async move {
				record_database(&events);
				let _commit = FileCommit::new(()).cleanup(
					cleanup_policy,
					file("uploads/old.txt"),
					FileCleanupOperation::Replace,
				);
				Err::<FileCommit<()>, _>("database failed")
			}
		})
		.await;

		match result {
			Err(FileMutationError::Database(error)) => assert_eq!(error, "database failed"),
			other => panic!("expected database failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/new.txt",
				"database",
				"delete default uploads/new.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn committed_cleanup_failure_is_logged_and_later_cleanup_runs(context: LifecycleContext) {
		context.backend.fail_delete("uploads/first-old.txt");
		context.backend.fail_delete("uploads/second-old.txt");
		context.backend.fail_delete("uploads/third-old.txt");
		let events = Arc::clone(&context.events);
		let logs = Arc::new(Mutex::new(Vec::new()));
		let subscriber = tracing_subscriber::registry().with(CleanupLogLayer {
			logs: Arc::clone(&logs),
		});
		let _subscriber_guard = tracing::subscriber::set_default(subscriber);

		let result = coordinate_file_mutations(Vec::new(), move |_| {
			let events = Arc::clone(&events);
			async move {
				record_database(&events);
				Ok::<_, &'static str>(
					FileCommit::new("saved")
						.cleanup(
							policy("avatar", true),
							file("uploads/first-old.txt"),
							FileCleanupOperation::Replace,
						)
						.cleanup(
							policy("resume", true),
							file("uploads/second-old.txt"),
							FileCleanupOperation::Clear,
						)
						.cleanup(
							policy("contract", true),
							file("uploads/third-old.txt"),
							FileCleanupOperation::Delete,
						),
				)
			}
		})
		.await;

		assert_eq!(result.unwrap(), "saved");
		assert_eq!(
			actual_events(&context.events),
			[
				"database",
				"delete default uploads/first-old.txt",
				"delete default uploads/second-old.txt",
				"delete default uploads/third-old.txt",
			]
		);
		assert_eq!(
			*logs.lock().unwrap(),
			[
				expected_log("replace_cleanup", "avatar", "uploads/first-old.txt",),
				expected_log("clear_cleanup", "resume", "uploads/second-old.txt",),
				expected_log("delete_cleanup", "contract", "uploads/third-old.txt",),
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn replacing_same_alias_and_path_skips_committed_cleanup(context: LifecycleContext) {
		let events = Arc::clone(&context.events);

		let result = descriptor()
			.replace_with(
				file("uploads/same.txt"),
				upload("same.txt"),
				move |stored| {
					let events = Arc::clone(&events);
					async move {
						record_database(&events);
						Ok::<_, &'static str>(stored)
					}
				},
			)
			.await;

		assert_eq!(result.unwrap(), file("uploads/same.txt"));
		assert_eq!(
			actual_events(&context.events),
			["store default uploads/same.txt", "database"]
		);
		assert!(context.backend.contains("uploads/same.txt"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn second_pre_storage_failure_compensates_first_in_reverse(context: LifecycleContext) {
		let events = Arc::clone(&context.events);
		let writes = vec![
			PendingFileUpload {
				policy: policy("avatar", true),
				operation: FileWriteOperation::Create,
				upload: upload("first.txt"),
			},
			PendingFileUpload {
				policy: owned_policy("resume", true),
				operation: FileWriteOperation::Create,
				upload: upload_without_filename(),
			},
		];

		let result = coordinate_file_mutations(writes, move |_| async move {
			record_database(&events);
			Ok::<_, &'static str>(FileCommit::new(()))
		})
		.await;

		match result {
			Err(FileMutationError::StorageForField { field, source }) => {
				assert_eq!(field, "resume");
				assert_eq!(source.to_string(), "the upload does not include a filename");
			}
			other => panic!("expected storage failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/first.txt",
				"delete default uploads/first.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn second_store_failure_compensates_first_and_skips_database(context: LifecycleContext) {
		context.backend.fail_store("uploads/second.txt");
		let events = Arc::clone(&context.events);
		let writes = vec![
			PendingFileUpload {
				policy: policy("avatar", true),
				operation: FileWriteOperation::Create,
				upload: upload("first.txt"),
			},
			PendingFileUpload {
				policy: owned_policy("resume", true),
				operation: FileWriteOperation::Replace,
				upload: upload("second.txt"),
			},
		];

		let result = coordinate_file_mutations(writes, move |_| async move {
			record_database(&events);
			Ok::<_, &'static str>(FileCommit::new(()))
		})
		.await;

		match result {
			Err(FileMutationError::StorageForField { field, source }) => {
				assert_eq!(field, "resume");
				assert_eq!(
					source.to_string(),
					"Permission denied: store uploads/second.txt"
				);
			}
			other => panic!("expected storage failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/first.txt",
				"store default uploads/second.txt",
				"delete default uploads/first.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn two_fields_use_one_database_closure_and_compensate_in_reverse(
		context: LifecycleContext,
	) {
		let events = Arc::clone(&context.events);
		let writes = vec![
			PendingFileUpload {
				policy: policy("avatar", true),
				operation: FileWriteOperation::Create,
				upload: upload("first.txt"),
			},
			PendingFileUpload {
				policy: owned_policy("resume", true),
				operation: FileWriteOperation::Replace,
				upload: upload("second.txt"),
			},
		];

		let result = coordinate_file_mutations(writes, move |stored| {
			let events = Arc::clone(&events);
			async move {
				record_database(&events);
				assert_eq!(
					stored,
					[file("uploads/first.txt"), file("uploads/second.txt")]
				);
				Err::<FileCommit<()>, _>("database failed")
			}
		})
		.await;

		match result {
			Err(FileMutationError::Database(error)) => assert_eq!(error, "database failed"),
			other => panic!("expected database failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/first.txt",
				"store default uploads/second.txt",
				"database",
				"delete default uploads/second.txt",
				"delete default uploads/first.txt",
			]
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn compensation_failure_keeps_database_error_and_continues_reverse_cleanup(
		context: LifecycleContext,
	) {
		context.backend.fail_delete("uploads/second.txt");
		let events = Arc::clone(&context.events);
		let logs = Arc::new(Mutex::new(Vec::new()));
		let subscriber = tracing_subscriber::registry().with(CleanupLogLayer {
			logs: Arc::clone(&logs),
		});
		let _subscriber_guard = tracing::subscriber::set_default(subscriber);
		let writes = vec![
			PendingFileUpload {
				policy: policy("avatar", true),
				operation: FileWriteOperation::Create,
				upload: upload("first.txt"),
			},
			PendingFileUpload {
				policy: owned_policy("resume", true),
				operation: FileWriteOperation::Replace,
				upload: upload("second.txt"),
			},
		];

		let result = coordinate_file_mutations(writes, move |_| {
			let events = Arc::clone(&events);
			async move {
				record_database(&events);
				Err::<FileCommit<()>, _>("database failed")
			}
		})
		.await;

		match result {
			Err(FileMutationError::Database(error)) => assert_eq!(error, "database failed"),
			other => panic!("expected database failure, got {other:?}"),
		}
		assert_eq!(
			actual_events(&context.events),
			[
				"store default uploads/first.txt",
				"store default uploads/second.txt",
				"database",
				"delete default uploads/second.txt",
				"delete default uploads/first.txt",
			]
		);
		assert_eq!(
			*logs.lock().unwrap(),
			[expected_log(
				"replace_compensation",
				"resume",
				"uploads/second.txt",
			)]
		);
	}
}
