//! Amazon S3 storage backend implementation.

use async_trait::async_trait;
use aws_config::Region;
use aws_sdk_s3::{Client, primitives::ByteStream};
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::config::S3Config;
use crate::{Result, StorageBackend, StorageError};

/// Amazon S3 storage backend.
#[derive(Debug, Clone)]
pub struct S3Storage {
	client: Client,
	bucket: String,
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
	/// Returns `` `StorageError::ConfigError` `` if AWS configuration fails.
	pub async fn new(config: S3Config) -> Result<Self> {
		let mut config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest());

		if let Some(region) = config.region {
			config_builder = config_builder.region(Region::new(region));
		}

		let sdk_config = config_builder.load().await;
		let mut client_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

		if let Some(endpoint) = config.endpoint {
			client_builder = client_builder.endpoint_url(endpoint).force_path_style(true);
		}

		let client = Client::from_conf(client_builder.build());

		Ok(Self {
			client,
			bucket: config.bucket,
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

		self.client
			.put_object()
			.bucket(&self.bucket)
			.key(&key)
			.body(ByteStream::from(content.to_vec()))
			.send()
			.await?;

		Ok(key)
	}

	async fn open(&self, name: &str) -> Result<Vec<u8>> {
		let key = self.get_key(name);

		let response = self
			.client
			.get_object()
			.bucket(&self.bucket)
			.key(&key)
			.send()
			.await?;

		let bytes = response
			.body
			.collect()
			.await
			.map_err(|e| StorageError::NetworkError(e.to_string()))?;

		Ok(bytes.to_vec())
	}

	async fn delete(&self, name: &str) -> Result<()> {
		if !self.exists(name).await? {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let key = self.get_key(name);

		self.client
			.delete_object()
			.bucket(&self.bucket)
			.key(&key)
			.send()
			.await?;

		Ok(())
	}

	async fn exists(&self, name: &str) -> Result<bool> {
		let key = self.get_key(name);

		match self
			.client
			.head_object()
			.bucket(&self.bucket)
			.key(&key)
			.send()
			.await
		{
			Ok(_) => Ok(true),
			Err(e) => {
				if let aws_sdk_s3::error::SdkError::ServiceError(ref service_err) = e
					&& service_err.err().is_not_found()
				{
					return Ok(false);
				}
				Err(e.into())
			}
		}
	}

	async fn url(&self, name: &str, expiry_secs: u64) -> Result<String> {
		if !self.exists(name).await? {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let key = self.get_key(name);

		let presigned_request = self
			.client
			.get_object()
			.bucket(&self.bucket)
			.key(&key)
			.presigned(
				aws_sdk_s3::presigning::PresigningConfig::expires_in(Duration::from_secs(
					expiry_secs,
				))
				.map_err(|e| StorageError::ConfigError(e.to_string()))?,
			)
			.await
			.map_err(|e| StorageError::NetworkError(e.to_string()))?;

		Ok(presigned_request.uri().to_string())
	}

	async fn size(&self, name: &str) -> Result<u64> {
		let key = self.get_key(name);

		let response = self
			.client
			.head_object()
			.bucket(&self.bucket)
			.key(&key)
			.send()
			.await?;

		response
			.content_length()
			.map(|size| size as u64)
			.ok_or_else(|| StorageError::Other("Content-Length header missing".to_string()))
	}

	async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>> {
		let key = self.get_key(name);

		let response = self
			.client
			.head_object()
			.bucket(&self.bucket)
			.key(&key)
			.send()
			.await?;

		let last_modified = response
			.last_modified()
			.ok_or_else(|| StorageError::Other("Last-Modified header missing".to_string()))?;

		let timestamp = last_modified.secs();
		let datetime = DateTime::from_timestamp(timestamp, 0)
			.ok_or_else(|| StorageError::Other("Invalid timestamp".to_string()))?;

		Ok(datetime)
	}
}
