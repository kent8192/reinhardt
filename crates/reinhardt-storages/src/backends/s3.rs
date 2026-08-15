//! Amazon S3 storage backend implementation.

#![allow(deprecated)] // Backend constructor keeps accepting legacy config during compatibility.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reinhardt_providers::{
	ProviderError,
	aws::{AwsCredentialsSource, S3Client, S3ClientConfig},
};
use std::time::Duration;

use crate::config::S3Config;
use crate::{Result, StorageBackend, StorageCapabilities, StorageError};

/// Amazon S3 storage backend.
#[derive(Debug, Clone)]
pub struct S3Storage {
	client: S3Client,
	prefix: Option<String>,
}

impl S3Storage {
	/// Create a new S3 storage backend.
	///
	/// # Arguments
	///
	/// * `config` - S3 configuration
	///
	/// # Errors
	///
	/// Keeps returning `Result` for compatibility with the storage factory.
	/// Credential loading is deferred until an S3 operation signs a request.
	pub async fn new(config: S3Config) -> Result<Self> {
		let force_path_style = config.endpoint.is_some();
		let region = config.region;
		let client = S3Client::new(S3ClientConfig {
			bucket: config.bucket,
			region: region.clone(),
			endpoint: config.endpoint,
			credentials: AwsCredentialsSource::default_chain(region),
			force_path_style,
		});

		Ok(Self {
			client,
			prefix: config.prefix,
		})
	}

	/// Get the full key path with prefix.
	fn get_key(&self, name: &str) -> String {
		if let Some(prefix) = &self.prefix {
			format!("{}/{}", prefix.trim_end_matches('/'), name)
		} else {
			name.to_string()
		}
	}
}

#[async_trait]
impl StorageBackend for S3Storage {
	async fn save(&self, name: &str, content: &[u8]) -> Result<String> {
		let key = self.get_key(name);

		self.client.put_object(&key, content.to_vec()).await?;

		Ok(key)
	}

	async fn save_if_absent(&self, name: &str, content: &[u8]) -> Result<String> {
		let key = self.get_key(name);

		self.client
			.put_object_if_absent(&key, content.to_vec())
			.await
			.map_err(|err| map_provider_already_exists(err, name))?;

		Ok(name.to_owned())
	}

	async fn save_if_absent_with_adoption(
		&self,
		name: &str,
		content: &[u8],
		adoption: std::sync::Arc<dyn crate::StoredObjectAdoption>,
	) -> Result<String> {
		let backend = self.clone();
		let name = name.to_owned();
		let content = content.to_vec();
		crate::backend::save_if_absent_with_adoption_task(
			async move { backend.save_if_absent(&name, &content).await },
			adoption,
		)
		.await
	}

	fn capabilities(&self) -> StorageCapabilities {
		StorageCapabilities {
			exclusive_create: true,
		}
	}

	async fn open(&self, name: &str) -> Result<Vec<u8>> {
		let key = self.get_key(name);

		let bytes = self
			.client
			.get_object(&key)
			.await
			.map_err(|err| map_provider_not_found(err, name))?;
		Ok(bytes.to_vec())
	}

	async fn delete(&self, name: &str) -> Result<()> {
		if !self.exists(name).await? {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let key = self.get_key(name);

		self.client
			.delete_object(&key)
			.await
			.map_err(|err| map_provider_not_found(err, name))?;

		Ok(())
	}

	async fn exists(&self, name: &str) -> Result<bool> {
		let key = self.get_key(name);

		Ok(self.client.head_object(&key).await?.is_some())
	}

	async fn url(&self, name: &str, expiry_secs: u64) -> Result<String> {
		if !self.exists(name).await? {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let key = self.get_key(name);

		Ok(self
			.client
			.presigned_get_url(&key, Duration::from_secs(expiry_secs))
			.await?)
	}

	async fn size(&self, name: &str) -> Result<u64> {
		let key = self.get_key(name);

		let metadata = self
			.client
			.head_object(&key)
			.await?
			.ok_or_else(|| StorageError::NotFound(name.to_string()))?;

		metadata
			.size
			.ok_or_else(|| StorageError::Other("Content-Length header missing".to_string()))
	}

	async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>> {
		let key = self.get_key(name);

		let metadata = self
			.client
			.head_object(&key)
			.await?
			.ok_or_else(|| StorageError::NotFound(name.to_string()))?;

		metadata
			.last_modified
			.ok_or_else(|| StorageError::Other("Last-Modified header missing".to_string()))
	}
}

fn map_provider_not_found(err: ProviderError, name: &str) -> StorageError {
	match err {
		ProviderError::NotFound(_) => StorageError::NotFound(name.to_string()),
		err => err.into(),
	}
}

fn map_provider_already_exists(err: ProviderError, name: &str) -> StorageError {
	match err {
		ProviderError::Service { status: 412, .. } => StorageError::AlreadyExists(name.to_owned()),
		err => err.into(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn precondition_failures_map_to_the_logical_name() {
		let err = map_provider_already_exists(
			ProviderError::Service {
				status: 412,
				message: "precondition failed".to_string(),
			},
			"avatars/a.png",
		);

		assert!(matches!(
			err,
			StorageError::AlreadyExists(name) if name == "avatars/a.png"
		));
	}

	#[test]
	fn conditional_request_conflicts_are_not_relabeled_as_already_exists() {
		let err = map_provider_already_exists(
			ProviderError::Service {
				status: 409,
				message: "conditional request conflict".to_string(),
			},
			"avatars/a.png",
		);

		assert!(
			matches!(err, StorageError::NetworkError(message) if message == "conditional request conflict")
		);
	}
}
