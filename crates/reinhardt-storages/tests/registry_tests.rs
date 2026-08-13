//! Tests for the process-wide file storage registry.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reinhardt_storages::{
	FileStorageError, StorageBackend, StorageCapabilities, StorageEntry, StorageError,
	StorageRegistry, StorageSettings, active_storage_registry,
};
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn local_entry(directory: &TempDir, expiry_secs: u64) -> StorageEntry {
	let backend = reinhardt_storages::backends::local::LocalStorage::new(
		reinhardt_storages::config::LocalConfig {
			base_path: directory.path().display().to_string(),
		},
	)
	.unwrap();

	StorageEntry::new(Arc::new(backend), Duration::from_secs(expiry_secs))
}

fn assert_registry_unavailable() {
	assert!(matches!(
		active_storage_registry(),
		Err(FileStorageError::RegistryUnavailable)
	));
}

struct UnsupportedExclusiveBackend;

#[async_trait]
impl StorageBackend for UnsupportedExclusiveBackend {
	async fn save(&self, name: &str, _content: &[u8]) -> Result<String, StorageError> {
		Ok(name.to_string())
	}

	fn capabilities(&self) -> StorageCapabilities {
		StorageCapabilities {
			exclusive_create: false,
		}
	}

	async fn open(&self, _name: &str) -> Result<Vec<u8>, StorageError> {
		Ok(Vec::new())
	}

	async fn delete(&self, _name: &str) -> Result<(), StorageError> {
		Ok(())
	}

	async fn exists(&self, _name: &str) -> Result<bool, StorageError> {
		Ok(false)
	}

	async fn url(&self, _name: &str, _expiry_secs: u64) -> Result<String, StorageError> {
		Ok("memory://file".to_string())
	}

	async fn size(&self, _name: &str) -> Result<u64, StorageError> {
		Ok(0)
	}

	async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>, StorageError> {
		Ok(Utc::now())
	}
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn resolves_default_and_named_entries_independently() {
	let default_directory = TempDir::new().unwrap();
	let private_directory = TempDir::new().unwrap();
	let registry = StorageRegistry::from_entries(
		local_entry(&default_directory, 3_600),
		[(
			"private_uploads".to_string(),
			local_entry(&private_directory, 900),
		)],
	)
	.unwrap();

	assert_eq!(
		registry.url_expiry("default").unwrap(),
		Duration::from_secs(3_600)
	);
	assert_eq!(
		registry.url_expiry("private_uploads").unwrap(),
		Duration::from_secs(900)
	);
	registry
		.backend("private_uploads")
		.unwrap()
		.save("independent.txt", b"private")
		.await
		.unwrap();
	assert!(!default_directory.path().join("independent.txt").exists());
	assert!(private_directory.path().join("independent.txt").exists());
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn settings_registry_uses_default_and_named_backend_conversions() {
	let default_directory = TempDir::new().unwrap();
	let private_directory = TempDir::new().unwrap();
	let settings: StorageSettings = toml::from_str(&format!(
		r#"
backend = "local"
url_expiry_secs = 3600

[local]
base_path = "{}"

[named.private_uploads]
backend = "local"
url_expiry_secs = 900

[named.private_uploads.local]
base_path = "{}"
"#,
		default_directory.path().display(),
		private_directory.path().display(),
	))
	.unwrap();

	let registry = StorageRegistry::from_settings(&settings).await.unwrap();

	assert_eq!(
		registry.url_expiry("default").unwrap(),
		Duration::from_secs(3_600)
	);
	assert_eq!(
		registry.url_expiry("private_uploads").unwrap(),
		Duration::from_secs(900)
	);
	registry
		.backend("default")
		.unwrap()
		.save("default.txt", b"default")
		.await
		.unwrap();
	assert!(default_directory.path().join("default.txt").exists());
	assert!(!private_directory.path().join("default.txt").exists());
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn activation_rejects_duplicate_registry_and_unregisters_on_drop() {
	assert_registry_unavailable();
	let directory = TempDir::new().unwrap();
	let registry = Arc::new(
		StorageRegistry::from_entries(
			local_entry(&directory, 3_600),
			std::iter::empty::<(String, StorageEntry)>(),
		)
		.unwrap(),
	);

	let lease = Arc::clone(&registry).activate().unwrap();
	assert!(Arc::ptr_eq(&active_storage_registry().unwrap(), &registry));
	assert!(matches!(
		Arc::clone(&registry).activate(),
		Err(FileStorageError::RegistryUnavailable)
	));

	drop(lease);
	assert_registry_unavailable();
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn backend_clone_remains_usable_after_registry_lease_drops() {
	let directory = TempDir::new().unwrap();
	let registry = Arc::new(
		StorageRegistry::from_entries(
			local_entry(&directory, 3_600),
			std::iter::empty::<(String, StorageEntry)>(),
		)
		.unwrap(),
	);
	let backend = registry.backend("default").unwrap();
	let lease = registry.activate().unwrap();

	drop(lease);
	backend.save("survives.txt", b"content").await.unwrap();

	assert_eq!(backend.open("survives.txt").await.unwrap(), b"content");
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn aliases_require_grammar_and_atomic_exclusive_creation() {
	for alias in [
		"",
		"default",
		"PrivateUploads",
		"private.uploads",
		"-private",
	] {
		let directory = TempDir::new().unwrap();
		let result = StorageRegistry::from_entries(
			local_entry(&directory, 3_600),
			[(alias.to_string(), local_entry(&directory, 900))],
		);

		assert!(
			matches!(result, Err(FileStorageError::UnknownStorageAlias(value)) if value == alias)
		);
	}

	let registry = StorageRegistry::from_entries(
		StorageEntry::new(
			Arc::new(UnsupportedExclusiveBackend),
			Duration::from_secs(3_600),
		),
		std::iter::empty::<(String, StorageEntry)>(),
	)
	.unwrap();

	assert!(matches!(
		registry.validate_file_field_aliases(["default"]),
		Err(FileStorageError::UnsupportedExclusiveSave(alias)) if alias == "default"
	));
	assert!(matches!(
		registry.backend("missing"),
		Err(FileStorageError::UnknownStorageAlias(alias)) if alias == "missing"
	));
}

#[rstest]
#[tokio::test]
#[serial(file_storage_registry)]
async fn registry_lease_cleans_up_after_early_return_and_panic() {
	async fn activate_then_return(registry: Arc<StorageRegistry>) {
		let _lease = registry.activate().unwrap();
	}

	let directory = TempDir::new().unwrap();
	let registry = Arc::new(
		StorageRegistry::from_entries(
			local_entry(&directory, 3_600),
			std::iter::empty::<(String, StorageEntry)>(),
		)
		.unwrap(),
	);

	activate_then_return(Arc::clone(&registry)).await;
	assert_registry_unavailable();

	let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		let _lease = Arc::clone(&registry).activate().unwrap();
		panic!("verify RAII cleanup during unwind");
	}));
	assert!(caught.is_err());
	assert_registry_unavailable();
}
