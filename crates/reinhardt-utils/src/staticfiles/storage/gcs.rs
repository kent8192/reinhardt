//! Google Cloud Storage backend
//!
//! Provides storage backend for Google Cloud Storage using the maintained
//! `google-cloud-storage` client instead of the unmaintained `cloud-storage`
//! crate (which pulled `ring 0.16.20` through `jsonwebtoken 7`).

use super::Storage;
use async_trait::async_trait;
use bytes::Bytes;
use google_cloud_auth::credentials::service_account::Builder as ServiceAccountBuilder;
use google_cloud_storage::client::{Storage as GcsClient, StorageControl};
use std::fmt;
use std::io;

/// Google Cloud Storage configuration
#[derive(Debug, Clone)]
pub struct GcsConfig {
	/// GCS bucket name
	pub bucket: String,
	/// GCS project ID
	pub project_id: String,
	/// Path prefix within bucket
	pub prefix: Option<String>,
	/// Base URL for generating file URLs
	pub base_url: String,
	/// Service account key JSON (optional, uses default credentials if not provided)
	pub service_account_key: Option<String>,
}

impl GcsConfig {
	/// Create a new GCS configuration
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_utils::staticfiles::storage::GcsConfig;
	///
	/// let config = GcsConfig::new(
	///     "my-bucket".to_string(),
	///     "my-project-id".to_string(),
	/// );
	/// ```
	pub fn new(bucket: String, project_id: String) -> Self {
		let base_url = format!("https://storage.googleapis.com/{bucket}");
		Self {
			bucket,
			project_id,
			prefix: None,
			base_url,
			service_account_key: None,
		}
	}

	/// Set service account key JSON
	pub fn with_service_account_key(mut self, key_json: String) -> Self {
		self.service_account_key = Some(key_json);
		self
	}

	/// Set path prefix within bucket
	pub fn with_prefix(mut self, prefix: String) -> Self {
		self.prefix = Some(prefix.trim_matches('/').to_string());
		self
	}

	/// Set base URL for file URLs
	pub fn with_base_url(mut self, base_url: String) -> Self {
		self.base_url = base_url.trim_end_matches('/').to_string();
		self
	}

	fn object_name(&self, name: &str) -> String {
		let name = name.trim_start_matches('/');
		match &self.prefix {
			Some(prefix) => format!("{prefix}/{name}"),
			None => name.to_string(),
		}
	}

	fn file_url(&self, name: &str) -> String {
		let name = name.trim_start_matches('/');
		match &self.prefix {
			Some(prefix) => format!("{}/{prefix}/{name}", self.base_url),
			None => format!("{}/{name}", self.base_url),
		}
	}

	fn bucket_resource(&self) -> String {
		format!("projects/_/buckets/{}", self.bucket)
	}
}

/// Google Cloud Storage backend
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_utils::staticfiles::storage::{GcsStorage, GcsConfig, Storage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = GcsConfig::new(
///     "my-bucket".to_string(),
///     "my-project-id".to_string(),
/// );
///
/// let storage = GcsStorage::new(config).await?;
///
/// // Save a file
/// let url = storage.save("css/style.css", b"body { color: red; }").await?;
/// # Ok(())
/// # }
/// ```
pub struct GcsStorage {
	storage: GcsClient,
	control: StorageControl,
	config: GcsConfig,
}

impl fmt::Debug for GcsStorage {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("GcsStorage")
			.field("config", &self.config)
			.finish_non_exhaustive()
	}
}

impl GcsStorage {
	/// Create a new GCS storage backend
	pub async fn new(config: GcsConfig) -> io::Result<Self> {
		let (storage, control) = Self::create_clients(&config).await?;
		Ok(Self {
			storage,
			control,
			config,
		})
	}

	/// Create GCS clients from configuration.
	///
	/// Authentication uses a service-account JSON payload when provided,
	/// otherwise Application Default Credentials.
	async fn create_clients(config: &GcsConfig) -> io::Result<(GcsClient, StorageControl)> {
		let mut storage_builder = GcsClient::builder();
		let mut control_builder = StorageControl::builder();

		if let Some(key_json) = &config.service_account_key {
			let service_account_key: serde_json::Value =
				serde_json::from_str(key_json).map_err(|err| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Invalid service account key JSON: {err}"),
					)
				})?;
			let credentials = ServiceAccountBuilder::new(service_account_key)
				.build()
				.map_err(|err| {
					io::Error::new(
						io::ErrorKind::InvalidData,
						format!("Invalid GCS service account credentials: {err}"),
					)
				})?;
			storage_builder = storage_builder.with_credentials(credentials.clone());
			control_builder = control_builder.with_credentials(credentials);
		}

		let storage = storage_builder.build().await.map_err(|err| {
			io::Error::other(format!("Failed to create GCS storage client: {err}"))
		})?;
		let control = control_builder.build().await.map_err(|err| {
			io::Error::other(format!("Failed to create GCS control client: {err}"))
		})?;
		Ok((storage, control))
	}

	fn map_sdk_error(err: google_cloud_storage::Error, name: &str) -> io::Error {
		let message = err.to_string();
		let lowered = message.to_ascii_lowercase();
		if message.contains("404") || lowered.contains("not found") {
			io::Error::new(
				io::ErrorKind::NotFound,
				format!("GCS object not found: {name}"),
			)
		} else {
			io::Error::other(message)
		}
	}

	async fn object_exists(&self, object: &str) -> io::Result<bool> {
		match self
			.control
			.get_object()
			.set_bucket(self.config.bucket_resource())
			.set_object(object)
			.send()
			.await
		{
			Ok(_) => Ok(true),
			Err(err) => {
				let mapped = Self::map_sdk_error(err, object);
				if mapped.kind() == io::ErrorKind::NotFound {
					Ok(false)
				} else {
					Err(mapped)
				}
			}
		}
	}
}

#[async_trait]
impl Storage for GcsStorage {
	async fn save(&self, name: &str, content: &[u8]) -> io::Result<String> {
		let object_name = self.config.object_name(name);

		self.storage
			.write_object(
				self.config.bucket_resource(),
				object_name.clone(),
				Bytes::copy_from_slice(content),
			)
			.send_buffered()
			.await
			.map_err(|err| Self::map_sdk_error(err, &object_name))?;

		Ok(self.url(name))
	}

	fn exists(&self, name: &str) -> bool {
		let object_name = self.config.object_name(name);
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current()
				.block_on(async { self.object_exists(&object_name).await.unwrap_or(false) })
		})
	}

	async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		let object_name = self.config.object_name(name);
		let mut response = self
			.storage
			.read_object(self.config.bucket_resource(), object_name.clone())
			.send()
			.await
			.map_err(|err| Self::map_sdk_error(err, &object_name))?;
		let mut content = Vec::new();
		while let Some(chunk) = response.next().await {
			content
				.extend_from_slice(&chunk.map_err(|err| Self::map_sdk_error(err, &object_name))?);
		}
		Ok(content)
	}

	async fn delete(&self, name: &str) -> io::Result<()> {
		let object_name = self.config.object_name(name);
		if !self.object_exists(&object_name).await? {
			return Err(io::Error::new(
				io::ErrorKind::NotFound,
				format!("GCS object not found: {object_name}"),
			));
		}

		self.control
			.delete_object()
			.set_bucket(self.config.bucket_resource())
			.set_object(object_name.clone())
			.send()
			.await
			.map_err(|err| Self::map_sdk_error(err, &object_name))?;
		Ok(())
	}

	fn url(&self, name: &str) -> String {
		self.config.file_url(name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn gcs_config_creation_sets_default_base_url() {
		// Arrange
		let bucket = "test-bucket";
		let project_id = "test-project";

		// Act
		let config = GcsConfig::new(bucket.to_string(), project_id.to_string());

		// Assert
		assert_eq!(config.bucket, bucket);
		assert_eq!(config.project_id, project_id);
		assert_eq!(
			config.base_url,
			"https://storage.googleapis.com/test-bucket"
		);
	}

	#[rstest]
	fn gcs_config_with_service_account_key_stores_json() {
		// Arrange
		let key_json = "{\"type\": \"service_account\"}";

		// Act
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_service_account_key(key_json.to_string());

		// Assert
		assert_eq!(config.service_account_key.as_deref(), Some(key_json));
	}

	#[rstest]
	fn gcs_config_with_prefix_stores_trimmed_prefix() {
		// Arrange / Act
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_prefix("/static/".to_string());

		// Assert
		assert_eq!(config.prefix.as_deref(), Some("static"));
	}

	#[rstest]
	fn gcs_config_with_base_url_strips_trailing_slash() {
		// Arrange / Act
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_base_url("https://cdn.example.com/".to_string());

		// Assert
		assert_eq!(config.base_url, "https://cdn.example.com");
	}

	#[rstest]
	fn object_name_generation_strips_leading_slash() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string());

		// Act / Assert
		assert_eq!(config.object_name("file.txt"), "file.txt");
		assert_eq!(config.object_name("/file.txt"), "file.txt");
	}

	#[rstest]
	fn object_name_generation_includes_prefix() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_prefix("static".to_string());

		// Act / Assert
		assert_eq!(config.object_name("file.txt"), "static/file.txt");
		assert_eq!(config.object_name("/file.txt"), "static/file.txt");
	}

	#[rstest]
	fn url_generation_uses_bucket_base() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string());

		// Act / Assert
		assert_eq!(
			config.file_url("file.txt"),
			"https://storage.googleapis.com/test-bucket/file.txt"
		);
	}

	#[rstest]
	fn url_generation_includes_prefix() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_prefix("static".to_string());

		// Act / Assert
		assert_eq!(
			config.file_url("file.txt"),
			"https://storage.googleapis.com/test-bucket/static/file.txt"
		);
	}

	#[rstest]
	fn url_generation_uses_custom_base() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_base_url("https://cdn.example.com".to_string());

		// Act / Assert
		assert_eq!(
			config.file_url("file.txt"),
			"https://cdn.example.com/file.txt"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn create_client_with_invalid_service_account_key_fails() {
		// Arrange
		let config = GcsConfig::new("test-bucket".to_string(), "test-project".to_string())
			.with_service_account_key("invalid json".to_string());

		// Act
		let result = GcsStorage::new(config).await;

		// Assert
		let error = result.expect_err("invalid JSON must fail client creation");
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		assert!(
			error
				.to_string()
				.contains("Invalid service account key JSON"),
			"Error message should indicate JSON validation failure, got: {error}"
		);
	}

	#[rstest]
	fn service_account_key_json_validation_rejects_non_json() {
		// Arrange
		let valid_json = r#"{"type": "service_account"}"#;
		let invalid_json = "not json";

		// Act / Assert
		assert!(serde_json::from_str::<serde_json::Value>(valid_json).is_ok());
		assert!(serde_json::from_str::<serde_json::Value>(invalid_json).is_err());
	}
}
