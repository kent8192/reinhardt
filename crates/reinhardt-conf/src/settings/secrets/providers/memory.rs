//! In-memory secret provider for testing

use crate::settings::secrets::{
	SecretError, SecretMetadata, SecretProvider, SecretResult, SecretString,
};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;

/// In-memory secret provider (for development/testing only)
pub struct MemorySecretProvider {
	secrets: RwLock<HashMap<String, SecretString>>,
	metadata: RwLock<HashMap<String, SecretMetadata>>,
}

impl MemorySecretProvider {
	/// Create a new memory secret provider
	pub fn new() -> Self {
		Self {
			secrets: RwLock::new(HashMap::new()),
			metadata: RwLock::new(HashMap::new()),
		}
	}
}

impl Default for MemorySecretProvider {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl SecretProvider for MemorySecretProvider {
	async fn get_secret(&self, key: &str) -> SecretResult<SecretString> {
		self.secrets
			.read()
			.get(key)
			.cloned()
			.ok_or_else(|| SecretError::NotFound(key.to_string()))
	}

	async fn get_secret_with_metadata(
		&self,
		key: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		let secret = self.get_secret(key).await?;
		let metadata = self.metadata.read().get(key).cloned().unwrap_or_default();
		Ok((secret, metadata))
	}

	async fn set_secret(&self, key: &str, value: SecretString) -> SecretResult<()> {
		self.secrets.write().insert(key.to_string(), value);
		let now = chrono::Utc::now();
		let metadata = SecretMetadata {
			created_at: Some(now),
			updated_at: Some(now),
		};
		self.metadata.write().insert(key.to_string(), metadata);
		Ok(())
	}

	async fn delete_secret(&self, key: &str) -> SecretResult<()> {
		self.secrets.write().remove(key);
		self.metadata.write().remove(key);
		Ok(())
	}

	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		Ok(self.secrets.read().keys().cloned().collect())
	}

	fn exists(&self, key: &str) -> bool {
		self.secrets.read().contains_key(key)
	}

	fn name(&self) -> &str {
		"memory"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_memory_provider_basic() {
		let provider = MemorySecretProvider::new();

		// Set a secret
		let secret = SecretString::new("my-secret-value");
		provider
			.set_secret("test_key", secret.clone())
			.await
			.unwrap();

		// Get the secret
		let retrieved = provider.get_secret("test_key").await.unwrap();
		assert_eq!(retrieved.expose_secret(), "my-secret-value");

		// Check exists
		assert!(provider.exists("test_key"));
		assert!(!provider.exists("nonexistent"));

		// List secrets
		let keys = provider.list_secrets().await.unwrap();
		assert!(keys.contains(&"test_key".to_string()));

		// Delete secret
		provider.delete_secret("test_key").await.unwrap();
		assert!(!provider.exists("test_key"));
	}

	#[tokio::test]
	async fn test_memory_provider_not_found() {
		let provider = MemorySecretProvider::new();
		let result = provider.get_secret("nonexistent").await;
		assert!(result.is_err());
		assert!(matches!(result, Err(SecretError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_memory_provider_with_metadata() {
		let provider = MemorySecretProvider::new();

		let secret = SecretString::new("test-value");
		provider.set_secret("key1", secret).await.unwrap();

		let (retrieved_secret, metadata) = provider.get_secret_with_metadata("key1").await.unwrap();

		assert_eq!(retrieved_secret.expose_secret(), "test-value");
		assert!(metadata.created_at.is_some());
		assert!(metadata.updated_at.is_some());
	}
}
