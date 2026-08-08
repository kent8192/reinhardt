//! Collision-safe upload service.

use crate::file_naming::{
	collision_candidate, expand_upload_template, normalize_client_filename, prepare_upload_key,
	validate_logical_key,
};
use crate::{FileStorageError, StorageError, StorageRegistry};
use chrono::{DateTime, Utc};
use rand::RngCore;
use reinhardt_core::parsers::UploadedFile;

const MAX_COLLISION_RETRIES: usize = 10;

/// Static policy used to store one model file field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadPolicy {
	/// Model type that owns the field.
	pub model: &'static str,
	/// Field name that owns the upload policy.
	pub field: &'static str,
	/// Relative UTC-aware upload directory template.
	pub upload_to: &'static str,
	/// Registry alias for the target storage backend.
	pub storage_alias: &'static str,
	/// Maximum logical-key length in Unicode scalar values.
	pub max_length: usize,
}

/// Logical storage reference returned after an upload succeeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredFile {
	/// Accepted portable logical key.
	pub path: String,
	/// Registry alias of the backend that accepted the key.
	pub storage_alias: String,
}

/// Store an uploaded file without overwriting an existing logical key.
///
/// # Errors
///
/// Returns [`FileStorageError`] for invalid filenames or policies, unavailable
/// storage aliases or capabilities, storage failures, and exhausted collisions.
pub async fn store_uploaded_file(
	registry: &StorageRegistry,
	policy: UploadPolicy,
	upload: UploadedFile,
) -> std::result::Result<StoredFile, FileStorageError> {
	let mut random = SystemRandom;
	store_uploaded_file_with_sources(registry, policy, upload, &SystemClock, &mut random).await
}

pub(crate) trait Clock {
	fn now(&self) -> DateTime<Utc>;
}

pub(crate) trait RandomSource {
	fn random_80_bits(&mut self) -> [u8; 10];
}

struct SystemClock;

impl Clock for SystemClock {
	fn now(&self) -> DateTime<Utc> {
		Utc::now()
	}
}

struct SystemRandom;

impl RandomSource for SystemRandom {
	fn random_80_bits(&mut self) -> [u8; 10] {
		let mut value = [0_u8; 10];
		rand::rng().fill_bytes(&mut value);
		value
	}
}

pub(crate) async fn store_uploaded_file_with_sources<C, R>(
	registry: &StorageRegistry,
	policy: UploadPolicy,
	upload: UploadedFile,
	clock: &C,
	random: &mut R,
) -> std::result::Result<StoredFile, FileStorageError>
where
	C: Clock,
	R: RandomSource,
{
	let filename = upload
		.filename
		.as_deref()
		.filter(|filename| !filename.is_empty())
		.ok_or(FileStorageError::MissingFilename)?;
	let normalized = normalize_client_filename(filename)?;
	let captured_time = clock.now();
	let directory = expand_upload_template(policy.upload_to, captured_time)?;
	let original = prepare_upload_key(&directory, &normalized, policy.max_length)?;
	let backend = registry.backend(policy.storage_alias)?;
	if !backend.capabilities().exclusive_create {
		return Err(FileStorageError::UnsupportedExclusiveSave(
			policy.storage_alias.to_string(),
		));
	}

	match backend
		.save_if_absent(&original, upload.data.as_ref())
		.await
	{
		Ok(path) => return stored_file(path, policy.storage_alias),
		Err(StorageError::AlreadyExists(_)) => {}
		Err(error) => return Err(error.into()),
	}

	for _ in 0..MAX_COLLISION_RETRIES {
		let candidate = collision_candidate(&original, random.random_80_bits());
		match backend
			.save_if_absent(&candidate, upload.data.as_ref())
			.await
		{
			Ok(path) => return stored_file(path, policy.storage_alias),
			Err(StorageError::AlreadyExists(_)) => {}
			Err(error) => return Err(error.into()),
		}
	}

	Err(FileStorageError::CollisionExhausted)
}

fn stored_file(
	path: String,
	storage_alias: &str,
) -> std::result::Result<StoredFile, FileStorageError> {
	validate_logical_key(&path)?;
	Ok(StoredFile {
		path,
		storage_alias: storage_alias.to_string(),
	})
}

#[cfg(test)]
mod tests {
	use super::{Clock, RandomSource, UploadPolicy, store_uploaded_file_with_sources};
	use crate::{
		FileStorageError, StorageBackend, StorageCapabilities, StorageEntry, StorageError,
		StorageRegistry,
	};
	use async_trait::async_trait;
	use chrono::{DateTime, TimeZone, Utc};
	use reinhardt_core::parsers::UploadedFile;
	use rstest::rstest;
	use std::collections::VecDeque;
	use std::sync::atomic::{AtomicUsize, Ordering};
	use std::sync::{Arc, Mutex};
	use std::time::Duration;

	#[derive(Clone, Copy)]
	enum Outcome {
		AlreadyExists,
		Success,
		PermissionDenied,
	}

	struct FakeBackend {
		outcomes: Mutex<VecDeque<Outcome>>,
		attempts: Mutex<Vec<String>>,
	}

	impl FakeBackend {
		fn new(outcomes: impl IntoIterator<Item = Outcome>) -> Self {
			Self {
				outcomes: Mutex::new(outcomes.into_iter().collect()),
				attempts: Mutex::new(Vec::new()),
			}
		}
	}

	#[async_trait]
	impl StorageBackend for FakeBackend {
		async fn save(&self, name: &str, _content: &[u8]) -> Result<String, StorageError> {
			Ok(name.to_string())
		}

		async fn save_if_absent(
			&self,
			name: &str,
			_content: &[u8],
		) -> Result<String, StorageError> {
			self.attempts.lock().unwrap().push(name.to_string());
			match self.outcomes.lock().unwrap().pop_front().unwrap() {
				Outcome::AlreadyExists => Err(StorageError::AlreadyExists(name.to_string())),
				Outcome::Success => Ok(name.to_string()),
				Outcome::PermissionDenied => {
					Err(StorageError::PermissionDenied("denied".to_string()))
				}
			}
		}

		fn capabilities(&self) -> StorageCapabilities {
			StorageCapabilities {
				exclusive_create: true,
			}
		}

		async fn open(&self, _name: &str) -> Result<Vec<u8>, StorageError> {
			unreachable!()
		}

		async fn delete(&self, _name: &str) -> Result<(), StorageError> {
			unreachable!()
		}

		async fn exists(&self, _name: &str) -> Result<bool, StorageError> {
			panic!("upload service must not perform exists-then-save")
		}

		async fn url(&self, _name: &str, _expiry_secs: u64) -> Result<String, StorageError> {
			unreachable!()
		}

		async fn size(&self, _name: &str) -> Result<u64, StorageError> {
			unreachable!()
		}

		async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>, StorageError> {
			unreachable!()
		}
	}

	#[derive(Default)]
	struct FakeClock {
		calls: AtomicUsize,
	}

	impl Clock for FakeClock {
		fn now(&self) -> DateTime<Utc> {
			self.calls.fetch_add(1, Ordering::Relaxed);
			Utc.with_ymd_and_hms(2026, 8, 8, 12, 34, 56).unwrap()
		}
	}

	struct FakeRandom {
		values: VecDeque<[u8; 10]>,
		calls: usize,
	}

	impl RandomSource for FakeRandom {
		fn random_80_bits(&mut self) -> [u8; 10] {
			self.calls += 1;
			self.values.pop_front().unwrap()
		}
	}

	fn registry(backend: Arc<FakeBackend>) -> StorageRegistry {
		StorageRegistry::from_entries(
			StorageEntry::new(backend, Duration::from_secs(60)),
			std::iter::empty::<(String, StorageEntry)>(),
		)
		.unwrap()
	}

	fn policy() -> UploadPolicy {
		UploadPolicy {
			model: "Profile",
			field: "avatar",
			upload_to: "avatars/%Y/%m/%d",
			storage_alias: "default",
			max_length: 255,
		}
	}

	fn upload() -> UploadedFile {
		UploadedFile::new("avatar".to_string(), (&b"content"[..]).into())
			.with_filename("Photo cat.PNG".to_string())
	}

	#[rstest]
	#[tokio::test]
	async fn collision_uses_the_first_exact_eighty_bit_suffix() {
		let backend = Arc::new(FakeBackend::new([Outcome::AlreadyExists, Outcome::Success]));
		let registry = registry(Arc::clone(&backend));
		let mut random = FakeRandom {
			values: [[0; 10]].into(),
			calls: 0,
		};
		let clock = FakeClock::default();

		let stored =
			store_uploaded_file_with_sources(&registry, policy(), upload(), &clock, &mut random)
				.await
				.unwrap();

		assert_eq!(
			stored.path,
			"avatars/2026/08/08/Photo_cat_aaaaaaaaaaaaaaaa.PNG"
		);
		assert_eq!(stored.storage_alias, "default");
		assert_eq!(
			*backend.attempts.lock().unwrap(),
			[
				"avatars/2026/08/08/Photo_cat.PNG",
				"avatars/2026/08/08/Photo_cat_aaaaaaaaaaaaaaaa.PNG",
			]
		);
		assert_eq!(random.calls, 1);
		assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn ten_suffix_collisions_exhaust_without_an_eleventh_retry() {
		let backend = Arc::new(FakeBackend::new([Outcome::AlreadyExists; 11]));
		let registry = registry(Arc::clone(&backend));
		let mut random = FakeRandom {
			values: (0_u8..11).map(|value| [value; 10]).collect(),
			calls: 0,
		};
		let clock = FakeClock::default();

		let error =
			store_uploaded_file_with_sources(&registry, policy(), upload(), &clock, &mut random)
				.await
				.unwrap_err();

		assert_eq!(
			error.to_string(),
			FileStorageError::CollisionExhausted.to_string()
		);
		assert_eq!(backend.attempts.lock().unwrap().len(), 11);
		assert_eq!(random.calls, 10);
		assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn non_collision_storage_error_returns_immediately() {
		let backend = Arc::new(FakeBackend::new([Outcome::PermissionDenied]));
		let registry = registry(Arc::clone(&backend));
		let mut random = FakeRandom {
			values: [[0; 10]].into(),
			calls: 0,
		};
		let clock = FakeClock::default();

		let error =
			store_uploaded_file_with_sources(&registry, policy(), upload(), &clock, &mut random)
				.await
				.unwrap_err();

		assert_eq!(error.to_string(), "Permission denied: denied");
		assert_eq!(backend.attempts.lock().unwrap().len(), 1);
		assert_eq!(random.calls, 0);
		assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
	}

	#[rstest]
	#[case(None)]
	#[case(Some(""))]
	#[tokio::test]
	async fn missing_or_empty_client_filename_fails_before_backend_access(
		#[case] filename: Option<&str>,
	) {
		let backend = Arc::new(FakeBackend::new([Outcome::Success]));
		let registry = registry(Arc::clone(&backend));
		let mut upload = UploadedFile::new("avatar".to_string(), (&b"content"[..]).into());
		upload.filename = filename.map(str::to_string);
		let mut random = FakeRandom {
			values: [[0; 10]].into(),
			calls: 0,
		};
		let clock = FakeClock::default();

		let error =
			store_uploaded_file_with_sources(&registry, policy(), upload, &clock, &mut random)
				.await
				.unwrap_err();

		assert_eq!(error.to_string(), "the upload does not include a filename");
		assert_eq!(backend.attempts.lock().unwrap().len(), 0);
		assert_eq!(random.calls, 0);
		assert_eq!(clock.calls.load(Ordering::Relaxed), 0);
	}
}
