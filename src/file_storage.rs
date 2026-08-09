//! Facade bootstrap for storage-backed file fields.

use reinhardt_db::migrations::model_registry::{ModelRegistry, global_registry};
use std::sync::Arc;

pub use reinhardt_storages::{
	ActiveStorageRegistryGuard, FileStorageError, StorageBackend, StorageCapabilities,
	StorageEntry, StorageError, StorageRegistry, StorageSettings, active_storage_registry,
};

/// Initialize and activate the process-wide storage registry for registered models.
pub async fn initialize(
	settings: &StorageSettings,
) -> Result<ActiveStorageRegistryGuard, FileStorageError> {
	initialize_with_model_registry(settings, global_registry()).await
}

/// Initialize storage using an explicit model registry.
///
/// This variant is useful for applications that control model registration
/// explicitly and for deterministic startup validation in tests.
pub async fn initialize_with_model_registry(
	settings: &StorageSettings,
	model_registry: &ModelRegistry,
) -> Result<ActiveStorageRegistryGuard, FileStorageError> {
	let registry = Arc::new(StorageRegistry::from_settings(settings).await?);
	activate_registry(registry, model_registry)
}

fn activate_registry(
	registry: Arc<StorageRegistry>,
	model_registry: &ModelRegistry,
) -> Result<ActiveStorageRegistryGuard, FileStorageError> {
	let aliases = model_registry.file_storage_aliases();
	registry.validate_file_field_aliases(aliases.iter().map(String::as_str))?;
	registry.activate()
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use chrono::{DateTime, Utc};
	use reinhardt_db::migrations::{FieldMetadata, FieldType, ModelMetadata};
	use reinhardt_storages::{
		BackendType, LocalStorageSettings, NamedStorageSettings, StorageCapabilities, StorageError,
	};
	use serial_test::serial;
	use std::path::PathBuf;
	use std::sync::atomic::{AtomicU64, Ordering};
	use std::time::Duration;

	static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

	struct TempDirectory(PathBuf);

	impl TempDirectory {
		fn new() -> Self {
			let suffix = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
			let path = std::env::temp_dir().join(format!(
				"reinhardt-file-storage-facade-{}-{suffix}",
				std::process::id()
			));
			std::fs::create_dir_all(&path).expect("temporary storage directory must be created");
			Self(path)
		}
	}

	impl Drop for TempDirectory {
		fn drop(&mut self) {
			let _ = std::fs::remove_dir_all(&self.0);
		}
	}

	fn local_settings(default_directory: &TempDirectory) -> StorageSettings {
		let mut settings = StorageSettings::default();
		settings.backend = BackendType::Local;
		settings.local = Some(LocalStorageSettings {
			base_path: default_directory.0.display().to_string(),
		});
		settings
	}

	fn model_registry_with_alias(alias: Option<&str>) -> ModelRegistry {
		let registry = ModelRegistry::new();
		let mut model = ModelMetadata::new("media", "Asset", "media_asset");
		let mut field =
			FieldMetadata::new(FieldType::VarChar(255)).with_param("model_field_type", "file");
		if let Some(alias) = alias {
			field = field.with_param("file_storage", alias);
		}
		model.add_field("content".to_string(), field);
		registry.register_model(model);
		registry
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn initialize_registers_default_and_named_backends_and_drop_clears_activation() {
		// Arrange
		let default_directory = TempDirectory::new();
		let named_directory = TempDirectory::new();
		let mut settings = local_settings(&default_directory);
		settings.named.insert(
			"private_uploads".to_string(),
			NamedStorageSettings {
				backend: BackendType::Local,
				url_expiry_secs: 900,
				#[cfg(feature = "file-storage-s3")]
				s3: None,
				#[cfg(feature = "file-storage-gcs")]
				gcs: None,
				#[cfg(feature = "file-storage-azure")]
				azure: None,
				local: Some(LocalStorageSettings {
					base_path: named_directory.0.display().to_string(),
				}),
			},
		);
		let models = model_registry_with_alias(Some("private_uploads"));

		// Act
		let guard = initialize_with_model_registry(&settings, &models)
			.await
			.expect("valid default and named storage settings must initialize");
		let active = active_storage_registry().expect("initialization must activate the registry");

		// Assert
		assert!(active.backend("default").is_ok());
		assert!(active.backend("private_uploads").is_ok());
		drop(guard);
		assert!(matches!(
			active_storage_registry(),
			Err(FileStorageError::RegistryUnavailable)
		));
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn initialize_validates_aliases_before_activation() {
		// Arrange
		let directory = TempDirectory::new();
		let settings = local_settings(&directory);
		let models = model_registry_with_alias(Some("missing_alias"));

		// Act
		let result = initialize_with_model_registry(&settings, &models).await;

		// Assert
		assert!(matches!(
			result,
			Err(FileStorageError::UnknownStorageAlias(alias)) if alias == "missing_alias"
		));
		assert!(matches!(
			active_storage_registry(),
			Err(FileStorageError::RegistryUnavailable)
		));
	}

	#[tokio::test]
	#[serial(file_storage_registry)]
	async fn unsupported_exclusive_create_fails_before_activation() {
		// Arrange
		let models = model_registry_with_alias(None);
		let backend = Arc::new(NoExclusiveCreateBackend);
		let registry = Arc::new(
			StorageRegistry::from_entries(
				StorageEntry::new(backend, Duration::from_secs(60)),
				[] as [(String, StorageEntry); 0],
			)
			.expect("default storage entry must be valid"),
		);

		// Act
		let result = activate_registry(registry, &models);

		// Assert
		assert!(matches!(
			result,
			Err(FileStorageError::UnsupportedExclusiveSave(alias)) if alias == "default"
		));
		assert!(matches!(
			active_storage_registry(),
			Err(FileStorageError::RegistryUnavailable)
		));
	}

	struct NoExclusiveCreateBackend;

	#[async_trait]
	impl StorageBackend for NoExclusiveCreateBackend {
		async fn save(&self, _name: &str, _content: &[u8]) -> reinhardt_storages::Result<String> {
			Err(StorageError::UnsupportedOperation(
				"test backend".to_string(),
			))
		}

		async fn open(&self, _name: &str) -> reinhardt_storages::Result<Vec<u8>> {
			Err(StorageError::NotFound("test backend".to_string()))
		}

		async fn delete(&self, _name: &str) -> reinhardt_storages::Result<()> {
			Ok(())
		}

		async fn exists(&self, _name: &str) -> reinhardt_storages::Result<bool> {
			Ok(false)
		}

		async fn url(&self, _name: &str, _expiry_secs: u64) -> reinhardt_storages::Result<String> {
			Err(StorageError::UnsupportedOperation(
				"test backend".to_string(),
			))
		}

		async fn size(&self, _name: &str) -> reinhardt_storages::Result<u64> {
			Err(StorageError::NotFound("test backend".to_string()))
		}

		async fn get_modified_time(
			&self,
			_name: &str,
		) -> reinhardt_storages::Result<DateTime<Utc>> {
			Err(StorageError::NotFound("test backend".to_string()))
		}

		fn capabilities(&self) -> StorageCapabilities {
			StorageCapabilities::default()
		}
	}
}
