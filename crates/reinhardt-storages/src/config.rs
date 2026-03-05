//! Configuration types for storage backends.

use crate::{Result, StorageError};
use std::env;
use std::str::FromStr;

/// Storage backend type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendType {
	/// Amazon S3 storage
	S3,
	/// Google Cloud Storage
	Gcs,
	/// Azure Blob Storage
	Azure,
	/// Local file system
	Local,
}

impl std::fmt::Display for BackendType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			BackendType::S3 => write!(f, "S3"),
			BackendType::Gcs => write!(f, "GCS"),
			BackendType::Azure => write!(f, "Azure"),
			BackendType::Local => write!(f, "Local"),
		}
	}
}

impl FromStr for BackendType {
	type Err = StorageError;

	fn from_str(s: &str) -> Result<Self> {
		match s.to_lowercase().as_str() {
			"s3" => Ok(BackendType::S3),
			"gcs" => Ok(BackendType::Gcs),
			"azure" => Ok(BackendType::Azure),
			"local" => Ok(BackendType::Local),
			_ => Err(StorageError::ConfigError(format!(
				"Invalid backend type: {}",
				s
			))),
		}
	}
}

/// Configuration for S3 storage backend.
#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
pub struct S3Config {
	/// S3 bucket name
	pub bucket: String,
	/// AWS region (e.g., "us-east-1")
	pub region: Option<String>,
	/// Custom endpoint URL (for LocalStack or MinIO)
	pub endpoint: Option<String>,
	/// Path prefix for all files
	pub prefix: Option<String>,
}

/// Configuration for Google Cloud Storage backend.
#[cfg(feature = "gcs")]
#[derive(Debug, Clone)]
pub struct GcsConfig {
	/// GCS bucket name
	pub bucket: String,
	/// Path prefix for all files
	pub prefix: Option<String>,
}

/// Configuration for Azure Blob Storage backend.
#[cfg(feature = "azure")]
#[derive(Debug, Clone)]
pub struct AzureConfig {
	/// Storage account name
	pub account: String,
	/// Container name
	pub container: String,
	/// Path prefix for all files
	pub prefix: Option<String>,
}

/// Configuration for local file system backend.
#[cfg(feature = "local")]
#[derive(Debug, Clone)]
pub struct LocalConfig {
	/// Base directory path for file storage
	pub base_path: String,
}

/// Storage configuration.
#[derive(Debug, Clone)]
pub enum StorageConfig {
	#[cfg(feature = "s3")]
	S3(S3Config),
	#[cfg(feature = "gcs")]
	Gcs(GcsConfig),
	#[cfg(feature = "azure")]
	Azure(AzureConfig),
	#[cfg(feature = "local")]
	Local(LocalConfig),
}

impl StorageConfig {
	/// Load configuration from environment variables.
	///
	/// # Environment Variables
	///
	/// - `STORAGE_BACKEND`: Backend type ("s3", "gcs", "azure", "local")
	///
	/// ## S3 Backend
	/// - `S3_BUCKET`: Bucket name (required)
	/// - `S3_REGION`: AWS region (optional)
	/// - `S3_ENDPOINT`: Custom endpoint URL (optional)
	/// - `S3_PREFIX`: Path prefix (optional)
	///
	/// ## GCS Backend
	/// - `GCS_BUCKET`: Bucket name (required)
	/// - `GCS_PREFIX`: Path prefix (optional)
	///
	/// ## Azure Backend
	/// - `AZURE_ACCOUNT`: Storage account name (required)
	/// - `AZURE_CONTAINER`: Container name (required)
	/// - `AZURE_PREFIX`: Path prefix (optional)
	///
	/// ## Local Backend
	/// - `LOCAL_BASE_PATH`: Base directory path (required)
	pub fn from_env() -> Result<Self> {
		let backend_type = env::var("STORAGE_BACKEND").map_err(|_| {
			StorageError::ConfigError("STORAGE_BACKEND environment variable not set".to_string())
		})?;

		let backend_type = backend_type.parse::<BackendType>()?;

		match backend_type {
			#[cfg(feature = "s3")]
			BackendType::S3 => {
				let bucket = env::var("S3_BUCKET").map_err(|_| {
					StorageError::ConfigError("S3_BUCKET environment variable not set".to_string())
				})?;
				let region = env::var("S3_REGION").ok();
				let endpoint = env::var("S3_ENDPOINT").ok();
				let prefix = env::var("S3_PREFIX").ok();

				Ok(StorageConfig::S3(S3Config {
					bucket,
					region,
					endpoint,
					prefix,
				}))
			}
			#[cfg(feature = "gcs")]
			BackendType::Gcs => {
				let bucket = env::var("GCS_BUCKET").map_err(|_| {
					StorageError::ConfigError("GCS_BUCKET environment variable not set".to_string())
				})?;
				let prefix = env::var("GCS_PREFIX").ok();

				Ok(StorageConfig::Gcs(GcsConfig { bucket, prefix }))
			}
			#[cfg(feature = "azure")]
			BackendType::Azure => {
				let account = env::var("AZURE_ACCOUNT").map_err(|_| {
					StorageError::ConfigError(
						"AZURE_ACCOUNT environment variable not set".to_string(),
					)
				})?;
				let container = env::var("AZURE_CONTAINER").map_err(|_| {
					StorageError::ConfigError(
						"AZURE_CONTAINER environment variable not set".to_string(),
					)
				})?;
				let prefix = env::var("AZURE_PREFIX").ok();

				Ok(StorageConfig::Azure(AzureConfig {
					account,
					container,
					prefix,
				}))
			}
			#[cfg(feature = "local")]
			BackendType::Local => {
				let base_path = env::var("LOCAL_BASE_PATH").map_err(|_| {
					StorageError::ConfigError(
						"LOCAL_BASE_PATH environment variable not set".to_string(),
					)
				})?;

				Ok(StorageConfig::Local(LocalConfig { base_path }))
			}
			#[allow(unreachable_patterns)]
			_ => Err(StorageError::ConfigError(format!(
				"Backend type not enabled: {:?}",
				backend_type
			))),
		}
	}
}
