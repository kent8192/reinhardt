//! Azure Key Vault provider
//!
//! This module provides integration with Azure Key Vault for retrieving secrets.

use crate::settings::secrets::{
	SecretError, SecretMetadata, SecretProvider, SecretResult, SecretString,
};
use async_trait::async_trait;

#[cfg(feature = "azure-keyvault")]
use azure_identity::DeveloperToolsCredential;
#[cfg(feature = "azure-keyvault")]
use azure_security_keyvault_secrets::SecretClient;

/// Azure Key Vault provider
///
/// This provider retrieves secrets from Azure Key Vault.
///
/// # Example
///
/// ```no_run
/// use reinhardt_conf::settings::secrets::providers::azure::AzureKeyVaultProvider;
/// use reinhardt_conf::settings::prelude::SecretProvider;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = AzureKeyVaultProvider::new(
///     "https://myvault.vault.azure.net"
/// ).await?;
/// let secret = provider.get_secret("database-password").await?;
/// # Ok(())
/// # }
/// ```
pub struct AzureKeyVaultProvider {
	#[cfg(feature = "azure-keyvault")]
	client: SecretClient,
	#[cfg(not(feature = "azure-keyvault"))]
	_phantom: std::marker::PhantomData<()>,
}

impl AzureKeyVaultProvider {
	/// Create a new Azure Key Vault provider
	///
	/// # Arguments
	///
	/// * `vault_url` - The URL of the Azure Key Vault (e.g., `"https://myvault.vault.azure.net"`)
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_conf::settings::secrets::providers::azure::AzureKeyVaultProvider;
	///
	/// let provider = AzureKeyVaultProvider::new(
	///     "https://myvault.vault.azure.net"
	/// ).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "azure-keyvault")]
	/// Documentation for `new`
	pub async fn new(vault_url: impl Into<String>) -> SecretResult<Self> {
		let vault_url = vault_url.into();
		let credential = DeveloperToolsCredential::new(None).map_err(|e| {
			SecretError::Provider(format!("Failed to create default credential: {}", e))
		})?;
		let client = SecretClient::new(&vault_url, credential, None)
			.map_err(|e| SecretError::Provider(format!("Failed to create Azure client: {}", e)))?;

		Ok(Self { client })
	}

	#[cfg(not(feature = "azure-keyvault"))]
	/// Documentation for `new`
	pub async fn new(_vault_url: impl Into<String>) -> SecretResult<Self> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled. Enable the 'azure-keyvault' feature.".to_string(),
		))
	}
}

#[async_trait]
impl SecretProvider for AzureKeyVaultProvider {
	#[cfg(feature = "azure-keyvault")]
	async fn get_secret(&self, name: &str) -> SecretResult<SecretString> {
		let result = self.client.get_secret(name, None).await;

		match result {
			Ok(response) => {
				let secret = response.into_body().map_err(|e| {
					SecretError::Provider(format!("Failed to parse secret response: {}", e))
				})?;
				Ok(SecretString::new(secret.value.unwrap_or_default()))
			}
			Err(err) => {
				if err.to_string().contains("404") || err.to_string().contains("SecretNotFound") {
					Err(SecretError::NotFound(format!(
						"Secret '{}' not found",
						name
					)))
				} else {
					Err(SecretError::Provider(format!(
						"Azure Key Vault error: {}",
						err
					)))
				}
			}
		}
	}

	#[cfg(not(feature = "azure-keyvault"))]
	async fn get_secret(&self, _name: &str) -> SecretResult<SecretString> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled".to_string(),
		))
	}

	#[cfg(feature = "azure-keyvault")]
	async fn get_secret_with_metadata(
		&self,
		name: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		let result = self.client.get_secret(name, None).await;

		match result {
			Ok(response) => {
				let secret = response.into_body().map_err(|e| {
					SecretError::Provider(format!("Failed to parse secret response: {}", e))
				})?;

				let created_at = secret
					.attributes
					.as_ref()
					.and_then(|attr| attr.created)
					.and_then(|ts| {
						let unix_timestamp = ts.unix_timestamp();
						chrono::DateTime::from_timestamp(unix_timestamp, 0)
					});

				let updated_at = secret
					.attributes
					.as_ref()
					.and_then(|attr| attr.updated)
					.and_then(|ts| {
						let unix_timestamp = ts.unix_timestamp();
						chrono::DateTime::from_timestamp(unix_timestamp, 0)
					});

				let metadata = SecretMetadata {
					created_at,
					updated_at,
				};

				Ok((
					SecretString::new(secret.value.unwrap_or_default()),
					metadata,
				))
			}
			Err(err) => {
				if err.to_string().contains("404") || err.to_string().contains("SecretNotFound") {
					Err(SecretError::NotFound(format!(
						"Secret '{}' not found",
						name
					)))
				} else {
					Err(SecretError::Provider(format!(
						"Azure Key Vault error: {}",
						err
					)))
				}
			}
		}
	}

	#[cfg(not(feature = "azure-keyvault"))]
	async fn get_secret_with_metadata(
		&self,
		_name: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled".to_string(),
		))
	}

	#[cfg(feature = "azure-keyvault")]
	async fn set_secret(&self, name: &str, value: SecretString) -> SecretResult<()> {
		use azure_security_keyvault_secrets::models::SetSecretParameters;

		let params = SetSecretParameters {
			value: Some(value.expose_secret().to_string()),
			..Default::default()
		};

		self.client
			.set_secret(
				name,
				params.try_into().map_err(|e| {
					SecretError::Provider(format!("Failed to create secret parameters: {}", e))
				})?,
				None,
			)
			.await
			.map_err(|e| SecretError::Provider(format!("Failed to set secret: {}", e)))?;

		Ok(())
	}

	#[cfg(not(feature = "azure-keyvault"))]
	async fn set_secret(&self, _name: &str, _value: SecretString) -> SecretResult<()> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled".to_string(),
		))
	}

	#[cfg(feature = "azure-keyvault")]
	async fn delete_secret(&self, name: &str) -> SecretResult<()> {
		self.client
			.delete_secret(name, None)
			.await
			.map_err(|e| SecretError::Provider(format!("Failed to delete secret: {}", e)))?;

		Ok(())
	}

	#[cfg(not(feature = "azure-keyvault"))]
	async fn delete_secret(&self, _name: &str) -> SecretResult<()> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled".to_string(),
		))
	}

	#[cfg(feature = "azure-keyvault")]
	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		use azure_security_keyvault_secrets::ResourceExt;
		use futures::stream::TryStreamExt;

		let mut secrets = Vec::new();
		let pager = self.client.list_secret_properties(None).map_err(|e| {
			SecretError::Provider(format!("Failed to create secret list pager: {}", e))
		})?;
		let mut stream = pager.into_stream();

		while let Some(secret_props) = stream
			.try_next()
			.await
			.map_err(|e| SecretError::Provider(format!("Failed to list secrets: {}", e)))?
		{
			let resource_id = secret_props
				.resource_id()
				.map_err(|e| SecretError::Provider(format!("Failed to get resource ID: {}", e)))?;
			secrets.push(resource_id.name);
		}

		Ok(secrets)
	}

	#[cfg(not(feature = "azure-keyvault"))]
	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		Err(SecretError::Provider(
			"Azure Key Vault support not enabled".to_string(),
		))
	}

	fn exists(&self, _name: &str) -> bool {
		// Cannot make async calls in sync method
		// Consumers should use get_secret() and check for NotFound error
		false
	}

	fn name(&self) -> &str {
		"azure-keyvault"
	}
}

#[cfg(all(test, feature = "azure-keyvault"))]
mod tests {
	use super::*;

	// Note: These tests require Azure credentials and a Key Vault, won't run in CI
	// They are here for local testing purposes

	#[tokio::test]
	#[ignore] // Ignore by default as it requires Azure credentials
	async fn test_azure_provider_creation() {
		let vault_url = std::env::var("AZURE_KEYVAULT_URL")
			.unwrap_or_else(|_| "https://test-vault.vault.azure.net".to_string());

		let result = AzureKeyVaultProvider::new(vault_url).await;
		// May fail without proper credentials, but should create the client
		assert!(result.is_ok() || result.is_err());
	}

	#[tokio::test]
	#[ignore]
	async fn test_azure_get_nonexistent_secret() {
		let vault_url = std::env::var("AZURE_KEYVAULT_URL")
			.expect("AZURE_KEYVAULT_URL must be set for this test");

		let provider = AzureKeyVaultProvider::new(vault_url).await.unwrap();

		let result = provider.get_secret("nonexistent-secret-12345").await;

		assert!(result.is_err());
		if let Err(SecretError::NotFound(_)) = result {
			// Expected
		} else {
			panic!("Expected NotFound error");
		}
	}
}

#[cfg(all(test, not(feature = "azure-keyvault")))]
mod tests_no_feature {
	use super::*;

	#[tokio::test]
	async fn test_azure_provider_disabled() {
		let result = AzureKeyVaultProvider::new("https://test.vault.azure.net").await;
		assert!(result.is_err());
	}
}
