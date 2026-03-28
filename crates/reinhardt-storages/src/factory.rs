//! Factory function for creating storage backends.

use crate::{Result, StorageBackend, StorageConfig, StorageError};
use std::sync::Arc;

/// Create a storage backend from configuration.
///
/// This factory function creates the appropriate storage backend based on
/// the provided configuration.
///
/// # Arguments
///
/// * `config` - Storage configuration
///
/// # Returns
///
/// A boxed trait object implementing `` `StorageBackend` ``.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_storages::{create_storage, StorageConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = StorageConfig::from_env()?;
///     let storage = create_storage(config).await?;
///     Ok(())
/// }
/// ```
pub async fn create_storage(config: StorageConfig) -> Result<Arc<dyn StorageBackend>> {
	match config {
		#[cfg(feature = "s3")]
		StorageConfig::S3(s3_config) => {
			let storage = crate::backends::s3::S3Storage::new(s3_config).await?;
			Ok(Arc::new(storage))
		}
		#[cfg(feature = "gcs")]
		StorageConfig::Gcs(_gcs_config) => {
			todo!("GCS backend not yet implemented")
		}
		#[cfg(feature = "azure")]
		StorageConfig::Azure(_azure_config) => {
			todo!("Azure backend not yet implemented")
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
