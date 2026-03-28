//! Azure Blob Storage backend implementation.
//!
//! **Note**: This backend is not yet implemented (Phase 2).

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::config::AzureConfig;
use crate::{Result, StorageBackend};

/// Azure Blob Storage backend.
#[derive(Debug, Clone)]
pub struct AzureStorage {
	_config: AzureConfig,
}

impl AzureStorage {
	/// Create a new Azure storage backend.
	pub fn new(config: AzureConfig) -> Result<Self> {
		Ok(Self { _config: config })
	}
}

#[async_trait]
impl StorageBackend for AzureStorage {
	async fn save(&self, _name: &str, _content: &[u8]) -> Result<String> {
		todo!("Azure save not yet implemented")
	}

	async fn open(&self, _name: &str) -> Result<Vec<u8>> {
		todo!("Azure open not yet implemented")
	}

	async fn delete(&self, _name: &str) -> Result<()> {
		todo!("Azure delete not yet implemented")
	}

	async fn exists(&self, _name: &str) -> Result<bool> {
		todo!("Azure exists not yet implemented")
	}

	async fn url(&self, _name: &str, _expiry_secs: u64) -> Result<String> {
		todo!("Azure url not yet implemented")
	}

	async fn size(&self, _name: &str) -> Result<u64> {
		todo!("Azure size not yet implemented")
	}

	async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>> {
		todo!("Azure get_modified_time not yet implemented")
	}
}
