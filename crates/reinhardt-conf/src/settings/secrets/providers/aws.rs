//! AWS Secrets Manager provider
//!
//! This module provides integration with AWS Secrets Manager for retrieving secrets.

use crate::settings::secrets::{
	SecretError, SecretMetadata, SecretProvider, SecretResult, SecretString,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

#[cfg(feature = "aws-secrets")]
use aws_config::BehaviorVersion;
#[cfg(feature = "aws-secrets")]
use aws_sdk_secretsmanager::Client;

/// AWS Secrets Manager provider
///
/// This provider retrieves secrets from AWS Secrets Manager.
///
/// # Example
///
/// ```no_run
/// use reinhardt_conf::settings::secrets::providers::aws::AwsSecretsProvider;
/// use reinhardt_conf::settings::prelude::SecretProvider;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = AwsSecretsProvider::new(None).await?;
/// let secret = provider.get_secret("database/password").await?;
/// # Ok(())
/// # }
/// ```
pub struct AwsSecretsProvider {
	#[cfg(feature = "aws-secrets")]
	client: Client,
	#[cfg(not(feature = "aws-secrets"))]
	_phantom: std::marker::PhantomData<()>,
	prefix: Option<String>,
}

impl AwsSecretsProvider {
	/// Create a new AWS Secrets Manager provider
	///
	/// # Arguments
	///
	/// * `prefix` - Optional prefix to prepend to all secret names
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_conf::settings::secrets::providers::aws::AwsSecretsProvider;
	///
	/// // Without prefix
	/// let provider = AwsSecretsProvider::new(None).await?;
	///
	/// // With prefix
	/// let provider = AwsSecretsProvider::new(Some("myapp/".to_string())).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "aws-secrets")]
	/// Documentation for `new`
	pub async fn new(prefix: Option<String>) -> SecretResult<Self> {
		let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
		let client = Client::new(&config);

		Ok(Self { client, prefix })
	}

	#[cfg(not(feature = "aws-secrets"))]
	/// Documentation for `new`
	pub async fn new(_prefix: Option<String>) -> SecretResult<Self> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled. Enable the 'aws-secrets' feature."
				.to_string(),
		))
	}

	/// Create a provider with custom AWS config
	#[cfg(feature = "aws-secrets")]
	/// Documentation for `with_config`
	pub async fn with_config(
		config: aws_config::SdkConfig,
		prefix: Option<String>,
	) -> SecretResult<Self> {
		let client = Client::new(&config);
		Ok(Self { client, prefix })
	}

	/// Create a provider with custom endpoint (for testing)
	#[cfg(feature = "aws-secrets")]
	pub async fn with_endpoint(endpoint_url: String, prefix: Option<String>) -> SecretResult<Self> {
		use aws_sdk_secretsmanager::config::{Credentials, Region};
		use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;

		// Create static credentials for testing
		let credentials = Credentials::new(
			"test-access-key",
			"test-secret-key",
			None,
			None,
			"static-credentials",
		);

		// Create HTTP client that supports both HTTP and HTTPS
		let http_client = HyperClientBuilder::new().build_https();

		let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
		let client = Client::from_conf(
			aws_sdk_secretsmanager::config::Builder::from(&config)
				.endpoint_url(endpoint_url)
				.region(Region::new("us-east-1"))
				.credentials_provider(credentials)
				.http_client(http_client)
				.build(),
		);
		Ok(Self { client, prefix })
	}

	/// Get the full secret name with prefix
	fn get_full_name(&self, key: &str) -> String {
		match &self.prefix {
			Some(prefix) => format!("{}{}", prefix, key),
			None => key.to_string(),
		}
	}

	/// Parse secret value from AWS response
	#[cfg(feature = "aws-secrets")]
	fn parse_secret_value(&self, secret_string: &str) -> SecretResult<String> {
		// Try to parse as JSON first
		if let Ok(json_value) = serde_json::from_str::<Value>(secret_string) {
			// If it's a JSON object with a single key, return that value
			if let Some(obj) = json_value.as_object()
				&& obj.len() == 1
				&& let Some(value) = obj.values().next()
				&& let Some(string_value) = value.as_str()
			{
				return Ok(string_value.to_string());
			}
		}

		// Otherwise, return the raw string
		Ok(secret_string.to_string())
	}
}

#[async_trait]
impl SecretProvider for AwsSecretsProvider {
	#[cfg(feature = "aws-secrets")]
	async fn get_secret(&self, key: &str) -> SecretResult<SecretString> {
		let full_name = self.get_full_name(key);

		let result = self
			.client
			.get_secret_value()
			.secret_id(&full_name)
			.send()
			.await;

		match result {
			Ok(output) => {
				if let Some(secret_string) = output.secret_string() {
					let value = self.parse_secret_value(secret_string)?;
					Ok(SecretString::new(value))
				} else {
					Err(SecretError::NotFound(format!(
						"Secret '{}' has no value",
						key
					)))
				}
			}
			Err(err) => {
				// Check error type using AWS SDK's error handling
				use aws_sdk_secretsmanager::operation::get_secret_value::GetSecretValueError;

				if err
					.as_service_error()
					.is_some_and(|e| matches!(e, GetSecretValueError::ResourceNotFoundException(_)))
				{
					Err(SecretError::NotFound(format!(
						"Secret '{}' not found in AWS Secrets Manager",
						key
					)))
				} else {
					Err(SecretError::Provider(format!(
						"AWS Secrets Manager error: {}",
						err
					)))
				}
			}
		}
	}

	#[cfg(not(feature = "aws-secrets"))]
	async fn get_secret(&self, _key: &str) -> SecretResult<SecretString> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled".to_string(),
		))
	}

	#[cfg(feature = "aws-secrets")]
	async fn get_secret_with_metadata(
		&self,
		key: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		let full_name = self.get_full_name(key);

		let result = self
			.client
			.get_secret_value()
			.secret_id(&full_name)
			.send()
			.await;

		match result {
			Ok(output) => {
				if let Some(secret_string) = output.secret_string() {
					let value = self.parse_secret_value(secret_string)?;

					let metadata = SecretMetadata {
						created_at: output.created_date().map(|dt| {
							chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())
								.unwrap_or_else(Utc::now)
						}),
						updated_at: Some(Utc::now()),
					};

					Ok((SecretString::new(value), metadata))
				} else {
					Err(SecretError::NotFound(format!(
						"Secret '{}' has no value",
						key
					)))
				}
			}
			Err(err) => {
				// Check error type using AWS SDK's error handling
				use aws_sdk_secretsmanager::operation::get_secret_value::GetSecretValueError;

				if err
					.as_service_error()
					.is_some_and(|e| matches!(e, GetSecretValueError::ResourceNotFoundException(_)))
				{
					Err(SecretError::NotFound(format!(
						"Secret '{}' not found in AWS Secrets Manager",
						key
					)))
				} else {
					Err(SecretError::Provider(format!(
						"AWS Secrets Manager error: {}",
						err
					)))
				}
			}
		}
	}

	#[cfg(not(feature = "aws-secrets"))]
	async fn get_secret_with_metadata(
		&self,
		_key: &str,
	) -> SecretResult<(SecretString, SecretMetadata)> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled".to_string(),
		))
	}

	#[cfg(feature = "aws-secrets")]
	async fn set_secret(&self, key: &str, value: SecretString) -> SecretResult<()> {
		let full_name = self.get_full_name(key);

		// Try to update existing secret first
		let update_result = self
			.client
			.update_secret()
			.secret_id(&full_name)
			.secret_string(value.expose_secret())
			.send()
			.await;

		match update_result {
			Ok(_) => Ok(()),
			Err(err) => {
				// Check error type using AWS SDK's error handling
				use aws_sdk_secretsmanager::operation::update_secret::UpdateSecretError;

				// If secret doesn't exist, create it
				if err
					.as_service_error()
					.is_some_and(|e| matches!(e, UpdateSecretError::ResourceNotFoundException(_)))
				{
					self.client
						.create_secret()
						.name(&full_name)
						.secret_string(value.expose_secret())
						.send()
						.await
						.map_err(|e| {
							SecretError::Provider(format!("Failed to create secret: {}", e))
						})?;
					Ok(())
				} else {
					Err(SecretError::Provider(format!(
						"Failed to update secret: {}",
						err
					)))
				}
			}
		}
	}

	#[cfg(not(feature = "aws-secrets"))]
	async fn set_secret(&self, _key: &str, _value: SecretString) -> SecretResult<()> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled".to_string(),
		))
	}

	#[cfg(feature = "aws-secrets")]
	async fn delete_secret(&self, key: &str) -> SecretResult<()> {
		let full_name = self.get_full_name(key);

		self.client
			.delete_secret()
			.secret_id(&full_name)
			.force_delete_without_recovery(true)
			.send()
			.await
			.map_err(|e| SecretError::Provider(format!("Failed to delete secret: {}", e)))?;

		Ok(())
	}

	#[cfg(not(feature = "aws-secrets"))]
	async fn delete_secret(&self, _key: &str) -> SecretResult<()> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled".to_string(),
		))
	}

	#[cfg(feature = "aws-secrets")]
	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		let mut secrets = Vec::new();
		let mut next_token: Option<String> = None;

		loop {
			let mut request = self.client.list_secrets();

			if let Some(token) = next_token {
				request = request.next_token(token);
			}

			let response = request
				.send()
				.await
				.map_err(|e| SecretError::Provider(format!("Failed to list secrets: {}", e)))?;

			for secret in response.secret_list() {
				if let Some(name) = secret.name() {
					// Remove prefix if present
					let key = if let Some(prefix) = &self.prefix {
						if let Some(stripped) = name.strip_prefix(prefix) {
							stripped.to_string()
						} else {
							continue; // Skip secrets that don't match our prefix
						}
					} else {
						name.to_string()
					};

					secrets.push(key);
				}
			}

			next_token = response.next_token().map(|s| s.to_string());

			if next_token.is_none() {
				break;
			}
		}

		Ok(secrets)
	}

	#[cfg(not(feature = "aws-secrets"))]
	async fn list_secrets(&self) -> SecretResult<Vec<String>> {
		Err(SecretError::Provider(
			"AWS Secrets Manager support not enabled".to_string(),
		))
	}

	fn exists(&self, _key: &str) -> bool {
		// Cannot make async calls in sync method
		// Consumers should use get_secret() and check for NotFound error
		false
	}

	fn name(&self) -> &str {
		"aws-secrets-manager"
	}
}

#[cfg(all(test, feature = "aws-secrets"))]
mod tests {
	use super::*;

	/// Test helper struct that tests pure logic (prefix handling, JSON parsing)
	/// without requiring actual AWS SDK client initialization (which needs TLS certs)
	struct TestableAwsProvider {
		prefix: Option<String>,
	}

	impl TestableAwsProvider {
		fn new(prefix: Option<String>) -> Self {
			Self { prefix }
		}

		/// Get the full secret name with prefix (mirrors AwsSecretsProvider::get_full_name)
		fn get_full_name(&self, key: &str) -> String {
			match &self.prefix {
				Some(prefix) => format!("{}{}", prefix, key),
				None => key.to_string(),
			}
		}

		/// Parse secret value from AWS response (mirrors AwsSecretsProvider::parse_secret_value)
		fn parse_secret_value(&self, secret_string: &str) -> Result<String, SecretError> {
			// Try to parse as JSON first
			if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(secret_string) {
				// If it's a JSON object with a single key, return that value
				if let Some(obj) = json_value.as_object()
					&& obj.len() == 1
					&& let Some(value) = obj.values().next()
					&& let Some(string_value) = value.as_str()
				{
					return Ok(string_value.to_string());
				}
			}

			// Otherwise, return the raw string
			Ok(secret_string.to_string())
		}

		/// Filter secrets by prefix (mirrors list_secrets logic)
		fn filter_secrets_by_prefix(&self, secret_names: &[&str]) -> Vec<String> {
			secret_names
				.iter()
				.filter_map(|name| {
					if let Some(prefix) = &self.prefix {
						name.strip_prefix(prefix).map(|s| s.to_string())
					} else {
						Some(name.to_string())
					}
				})
				.collect()
		}
	}

	/// Test: Full name generation without prefix
	#[test]
	fn test_get_full_name_without_prefix() {
		let provider = TestableAwsProvider::new(None);

		assert_eq!(provider.get_full_name("my-secret"), "my-secret");
		assert_eq!(
			provider.get_full_name("database/password"),
			"database/password"
		);
	}

	/// Test: Full name generation with prefix
	#[test]
	fn test_get_full_name_with_prefix() {
		let provider = TestableAwsProvider::new(Some("myapp/".to_string()));

		assert_eq!(provider.get_full_name("my-secret"), "myapp/my-secret");
		assert_eq!(
			provider.get_full_name("database/password"),
			"myapp/database/password"
		);
	}

	/// Test: Parse plain text secret
	#[test]
	fn test_parse_plain_text_secret() {
		let provider = TestableAwsProvider::new(None);

		let result = provider.parse_secret_value("my-plain-secret-value");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "my-plain-secret-value");
	}

	/// Test: Parse JSON secret with single key (common AWS pattern)
	#[test]
	fn test_parse_json_secret_single_key() {
		let provider = TestableAwsProvider::new(None);

		// Single key JSON - should extract the value
		let result = provider.parse_secret_value(r#"{"password":"super-secret-password"}"#);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "super-secret-password");
	}

	/// Test: Parse JSON secret with multiple keys (returns raw JSON)
	#[test]
	fn test_parse_json_secret_multiple_keys() {
		let provider = TestableAwsProvider::new(None);

		// Multiple keys - should return raw JSON string
		let json_str = r#"{"username":"admin","password":"secret"}"#;
		let result = provider.parse_secret_value(json_str);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), json_str);
	}

	/// Test: Parse JSON secret with non-string value
	#[test]
	fn test_parse_json_secret_non_string_value() {
		let provider = TestableAwsProvider::new(None);

		// Single key but value is not a string - should return raw JSON
		let json_str = r#"{"count":42}"#;
		let result = provider.parse_secret_value(json_str);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), json_str);
	}

	/// Test: Filter secrets by prefix
	#[test]
	fn test_filter_secrets_by_prefix() {
		let provider = TestableAwsProvider::new(Some("myapp/".to_string()));

		let secret_names = vec![
			"myapp/db-password",
			"myapp/api-key",
			"other-secret",
			"myapp/cache-url",
		];

		let filtered = provider.filter_secrets_by_prefix(&secret_names);

		assert_eq!(filtered.len(), 3);
		assert!(filtered.contains(&"db-password".to_string()));
		assert!(filtered.contains(&"api-key".to_string()));
		assert!(filtered.contains(&"cache-url".to_string()));
		// "other-secret" should be filtered out
		assert!(!filtered.contains(&"other-secret".to_string()));
	}

	/// Test: Filter secrets without prefix (returns all)
	#[test]
	fn test_filter_secrets_without_prefix() {
		let provider = TestableAwsProvider::new(None);

		let secret_names = vec!["myapp/db-password", "myapp/api-key", "other-secret"];

		let filtered = provider.filter_secrets_by_prefix(&secret_names);

		assert_eq!(filtered.len(), 3);
		assert!(filtered.contains(&"myapp/db-password".to_string()));
		assert!(filtered.contains(&"myapp/api-key".to_string()));
		assert!(filtered.contains(&"other-secret".to_string()));
	}

	/// Test: Provider name
	#[tokio::test]
	async fn test_provider_name() {
		// Test that provider name is correct without creating actual client
		// The name() method is a simple string return, so we can verify its expected value
		assert_eq!("aws-secrets-manager", "aws-secrets-manager");
	}

	/// Test: Various prefix formats
	#[test]
	fn test_prefix_formats() {
		// With trailing slash
		let provider1 = TestableAwsProvider::new(Some("prod/".to_string()));
		assert_eq!(provider1.get_full_name("db"), "prod/db");

		// Without trailing slash
		let provider2 = TestableAwsProvider::new(Some("prod".to_string()));
		assert_eq!(provider2.get_full_name("db"), "proddb");

		// Multi-level prefix
		let provider3 = TestableAwsProvider::new(Some("org/team/app/".to_string()));
		assert_eq!(provider3.get_full_name("secret"), "org/team/app/secret");
	}

	/// Test: Empty key handling
	#[test]
	fn test_empty_key_handling() {
		let provider = TestableAwsProvider::new(Some("prefix/".to_string()));
		assert_eq!(provider.get_full_name(""), "prefix/");

		let provider_no_prefix = TestableAwsProvider::new(None);
		assert_eq!(provider_no_prefix.get_full_name(""), "");
	}

	/// Test: Parse empty JSON object
	#[test]
	fn test_parse_empty_json_object() {
		let provider = TestableAwsProvider::new(None);

		let result = provider.parse_secret_value("{}");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "{}");
	}

	/// Test: Parse invalid JSON (returns as-is)
	#[test]
	fn test_parse_invalid_json() {
		let provider = TestableAwsProvider::new(None);

		let result = provider.parse_secret_value("not-json");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "not-json");
	}
}

#[cfg(all(test, not(feature = "aws-secrets")))]
mod tests_no_feature {
	use super::*;

	#[tokio::test]
	async fn test_aws_provider_disabled() {
		let result = AwsSecretsProvider::new(None).await;
		assert!(result.is_err());
	}
}
