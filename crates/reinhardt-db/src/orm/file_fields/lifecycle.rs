use super::{FileField, FileFieldError};
use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::upload::store_uploaded_file_with_borrowed_policy;
use reinhardt_storages::{StorageRegistry, active_storage_registry};
use std::borrow::Cow;
use std::future::Future;

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
	/// Whether previously committed files are removed after database success.
	pub cleanup: bool,
	/// Validation applied before the upload reaches storage.
	pub validation: FileValidationPolicy,
}

/// Validation policy for a staged file upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileValidationPolicy {
	/// Accept an ordinary file without content-specific validation.
	File,
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

	/// Add an old file for best-effort cleanup when its field policy permits it.
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
	file: FileField,
}

/// Store all staged files around one caller-owned database operation.
///
/// The persistence closure must return `Ok` only after its transaction is
/// durably committed. Old-file cleanup begins immediately after that result.
pub async fn coordinate_file_mutations<T, E, F, Fut>(
	writes: Vec<PendingFileUpload>,
	persist: F,
) -> Result<T, FileMutationError<E>>
where
	F: FnOnce(Vec<FileField>) -> Fut,
	Fut: Future<Output = Result<FileCommit<T>, E>>,
{
	let registry = active_storage_registry().map_err(FileMutationError::Storage)?;
	let mut stored_uploads = Vec::with_capacity(writes.len());

	for write in writes {
		if let Err(error) = validate_upload(&write.policy.validation, &write.upload) {
			compensate_uploads(&registry, &stored_uploads).await;
			return Err(FileMutationError::Storage(error));
		}

		let stored = match store_uploaded_file_with_borrowed_policy(
			&registry,
			write.policy.model.as_ref(),
			write.policy.field.as_ref(),
			write.policy.upload_to.as_ref(),
			write.policy.storage_alias.as_ref(),
			write.policy.max_length,
			write.upload,
		)
		.await
		{
			Ok(stored) => stored,
			Err(error) => {
				compensate_uploads(&registry, &stored_uploads).await;
				return Err(FileMutationError::Storage(error));
			}
		};

		let file = match FileField::from_existing(&stored.path, &stored.storage_alias) {
			Ok(file) => file,
			Err(error) => {
				cleanup_file(
					&registry,
					write.operation.compensation_name(),
					&write.policy,
					&stored.storage_alias,
					&stored.path,
				)
				.await;
				compensate_uploads(&registry, &stored_uploads).await;
				return Err(FileMutationError::Storage(error));
			}
		};
		stored_uploads.push(StoredFileUpload {
			policy: write.policy,
			operation: write.operation,
			file,
		});
	}

	let stored_values = stored_uploads
		.iter()
		.map(|stored| stored.file.clone())
		.collect();
	let commit = match persist(stored_values).await {
		Ok(commit) => commit,
		Err(error) => {
			compensate_uploads(&registry, &stored_uploads).await;
			return Err(FileMutationError::Database(error));
		}
	};

	for cleanup in commit.cleanup {
		if stored_uploads
			.iter()
			.any(|stored| stored.file == cleanup.file)
		{
			continue;
		}
		cleanup_file(
			&registry,
			cleanup.operation.cleanup_name(),
			&cleanup.policy,
			cleanup.file.storage_alias(),
			cleanup.file.path(),
		)
		.await;
	}

	Ok(commit.value)
}

fn validate_upload(
	policy: &FileValidationPolicy,
	_upload: &UploadedFile,
) -> Result<(), FileFieldError> {
	match policy {
		FileValidationPolicy::File => Ok(()),
	}
}

async fn compensate_uploads(registry: &StorageRegistry, uploads: &[StoredFileUpload]) {
	for upload in uploads.iter().rev() {
		cleanup_file(
			registry,
			upload.operation.compensation_name(),
			&upload.policy,
			upload.file.storage_alias(),
			upload.file.path(),
		)
		.await;
	}
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
		StorageError, StorageRegistry,
	};
	use rstest::{fixture, rstest};
	use serial_test::serial;
	use std::borrow::Cow;
	use std::collections::{BTreeMap, BTreeSet};
	use std::fmt;
	use std::sync::{Arc, Mutex};
	use std::time::Duration;
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
	}

	impl EventStorage {
		fn new(alias: &'static str, events: Events) -> Self {
			Self {
				alias,
				events,
				files: Mutex::new(BTreeMap::new()),
				store_failures: Mutex::new(BTreeSet::new()),
				delete_failures: Mutex::new(BTreeSet::new()),
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

		fn record(&self, operation: &str, path: &str) {
			self.events
				.lock()
				.unwrap()
				.push(format!("{operation} {} {path}", self.alias));
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

			let mut files = self.files.lock().unwrap();
			if files.contains_key(name) {
				return Err(StorageError::AlreadyExists(name.to_owned()));
			}
			files.insert(name.to_owned(), content.to_vec());
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
			Err(FileMutationError::Storage(error)) => {
				assert_eq!(error.to_string(), "the upload does not include a filename");
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
			Err(FileMutationError::Storage(error)) => {
				assert_eq!(
					error.to_string(),
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
