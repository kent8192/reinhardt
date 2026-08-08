//! Contract tests for storage-backend exclusive creation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reinhardt_storages::{Result, StorageBackend, StorageError};

struct LegacyBackend;

#[async_trait]
impl StorageBackend for LegacyBackend {
	async fn save(&self, name: &str, _content: &[u8]) -> Result<String> {
		Ok(name.to_owned())
	}

	async fn open(&self, _name: &str) -> Result<Vec<u8>> {
		Ok(Vec::new())
	}

	async fn delete(&self, _name: &str) -> Result<()> {
		Ok(())
	}

	async fn exists(&self, _name: &str) -> Result<bool> {
		Ok(false)
	}

	async fn url(&self, _name: &str, _expiry_secs: u64) -> Result<String> {
		Ok(String::new())
	}

	async fn size(&self, _name: &str) -> Result<u64> {
		Ok(0)
	}

	async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>> {
		Ok(Utc::now())
	}
}

#[tokio::test]
async fn legacy_backends_report_exclusive_create_as_unsupported() {
	let backend = LegacyBackend;

	assert_eq!(backend.capabilities().exclusive_create, false);
	assert!(matches!(
		backend.save_if_absent("example.txt", b"content").await,
		Err(StorageError::UnsupportedOperation(message))
			if message == "atomic exclusive create is not supported"
	));
}
