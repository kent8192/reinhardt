//! Environment variable secret provider

use crate::settings::secrets::{
	SecretError, SecretMetadata, SecretProvider, SecretResult, SecretString,
};
use async_trait::async_trait;
use std::env;

/// Environment variable secret provider
///
/// Reads secrets from environment variables with a configurable prefix.
/// This is useful for containerized environments and CI/CD pipelines.
pub struct EnvSecretProvider {
	prefix: String,
}

impl EnvSecretProvider {
	/// Create a new environment secret provider with a prefix
	///
	/// Example: With prefix "SECRET_", the key "database_password" will
	/// look for the environment variable "SECRET_DATABASE_PASSWORD"
	pub fn new(prefix: impl Into<String>) -> Self {
		Self {
			prefix: prefix.into(),
		}
	}
	/// Create a new environment secret provider without a prefix
	///
	pub fn without_prefix() -> Self {
		Self {
			prefix: String::new(),
		}
	}

	fn env_var_name(&self, key: &str) -> String {
		if self.prefix.is_empty() {
			key.to_uppercase()
		} else {
			format!("{}{}", self.prefix, key.to_uppercase())
		}
	}
}

impl Default for EnvSecretProvider {
	fn default() -> Self {
		Self::new("SECRET_")
	}
}

#[async_trait]
impl SecretProvider for EnvSecretProvider {
	async fn get_secret(&self, key: &str) -> SecretResult<SecretString> {
		let env_var = self.env_var_name(key);
		env::var(&env_var)
			.map(SecretString::new)
			.map_err(|_| SecretError::NotFound(format!("Environment variable: {}", env_var)))
	}

	async fn get_secret_with_metadata(
		&self,
		key: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		let secret = self.get_secret(key).await?;
		// Environment variables don't have metadata
		Ok((secret, SecretMetadata::default()))
	}

	async fn set_secret(&self, key: &str, value: SecretString) -> SecretResult<()> {
		let env_var = self.env_var_name(key);
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// Environment loading occurs during application startup before spawning threads.
		unsafe {
			env::set_var(&env_var, value.expose_secret());
		}
		Ok(())
	}

	async fn delete_secret(&self, key: &str) -> SecretResult<()> {
		let env_var = self.env_var_name(key);
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// Environment loading occurs during application startup before spawning threads.
		unsafe {
			env::remove_var(&env_var);
		}
		Ok(())
	}

	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		let secrets: Vec<String> = env::vars()
			.filter_map(|(k, _)| {
				if self.prefix.is_empty() {
					Some(k.to_lowercase())
				} else if k.starts_with(&self.prefix) {
					Some(k[self.prefix.len()..].to_lowercase())
				} else {
					None
				}
			})
			.collect();
		Ok(secrets)
	}

	fn exists(&self, key: &str) -> bool {
		let env_var = self.env_var_name(key);
		env::var(&env_var).is_ok()
	}

	fn name(&self) -> &str {
		"env"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_env_provider_with_prefix() {
		let provider = EnvSecretProvider::new("TEST_SECRET_");

		// Set a secret
		let secret = SecretString::new("test-value");
		provider.set_secret("db_password", secret).await.unwrap();

		// Verify it was set in the environment
		assert_eq!(env::var("TEST_SECRET_DB_PASSWORD").unwrap(), "test-value");

		// Get the secret
		let retrieved = provider.get_secret("db_password").await.unwrap();
		assert_eq!(retrieved.expose_secret(), "test-value");

		// Check exists
		assert!(provider.exists("db_password"));

		// Clean up
		provider.delete_secret("db_password").await.unwrap();
		assert!(!provider.exists("db_password"));
	}

	#[tokio::test]
	async fn test_env_provider_without_prefix() {
		let provider = EnvSecretProvider::without_prefix();

		let secret = SecretString::new("another-value");
		provider.set_secret("my_key", secret).await.unwrap();

		assert_eq!(env::var("MY_KEY").unwrap(), "another-value");

		let retrieved = provider.get_secret("my_key").await.unwrap();
		assert_eq!(retrieved.expose_secret(), "another-value");

		provider.delete_secret("my_key").await.unwrap();
	}

	#[tokio::test]
	async fn test_env_provider_not_found() {
		let provider = EnvSecretProvider::new("TEST_");
		let result = provider.get_secret("nonexistent_key").await;
		assert!(result.is_err());
		assert!(matches!(result, Err(SecretError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_env_provider_list() {
		let provider = EnvSecretProvider::new("LIST_TEST_");

		provider
			.set_secret("key1", SecretString::new("value1"))
			.await
			.unwrap();
		provider
			.set_secret("key2", SecretString::new("value2"))
			.await
			.unwrap();

		let keys = provider.list_secrets().await.unwrap();
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));

		// Clean up
		provider.delete_secret("key1").await.unwrap();
		provider.delete_secret("key2").await.unwrap();
	}
}
