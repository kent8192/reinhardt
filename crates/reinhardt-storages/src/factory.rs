//! Factory functions for creating storage backends.

#![allow(deprecated)] // Public compatibility factory accepts StorageConfig until removal.

use crate::{
	NamedStorageSettings, Result, StorageBackend, StorageConfig, StorageError, StorageSettings,
};
use std::sync::Arc;

/// Create a storage backend from settings.
pub async fn create_storage_from_settings(
	settings: &StorageSettings,
) -> Result<Arc<dyn StorageBackend>> {
	let config = settings.to_config()?;
	create_storage(config).await
}

/// Create a named storage backend from a storage registry entry.
pub(crate) async fn create_storage_from_named_settings(
	settings: &NamedStorageSettings,
	alias: &str,
) -> Result<Arc<dyn StorageBackend>> {
	let config = settings.to_config_for_alias(alias)?;
	create_storage(config).await
}

/// Create a storage backend from compatibility configuration.
pub async fn create_storage(config: StorageConfig) -> Result<Arc<dyn StorageBackend>> {
	match config {
		#[cfg(feature = "s3")]
		StorageConfig::S3(s3_config) => {
			let storage = crate::backends::s3::S3Storage::new(s3_config).await?;
			Ok(Arc::new(storage))
		}
		#[cfg(feature = "gcs")]
		StorageConfig::Gcs(gcs_config) => {
			let storage = crate::backends::gcs::GcsStorage::new(gcs_config).await?;
			Ok(Arc::new(storage))
		}
		#[cfg(feature = "azure")]
		StorageConfig::Azure(azure_config) => {
			let storage = crate::backends::azure::AzureStorage::new(azure_config).await?;
			Ok(Arc::new(storage))
		}
		#[cfg(feature = "local")]
		StorageConfig::Local(local_config) => {
			let storage = crate::backends::local::LocalStorage::new(local_config)?;
			Ok(Arc::new(storage))
		}
		#[allow(unreachable_patterns)]
		_ => Err(StorageError::ConfigError(
			"Backend not supported".to_string(),
		)),
	}
}
