//! AWS credential helpers.

use std::{env, fmt};

use aws_credential_types::provider::ProvideCredentials;
use aws_types::region::Region;

use crate::{ProviderError, Result};

/// AWS credentials used for signing provider requests.
#[derive(Clone, PartialEq, Eq)]
pub struct AwsCredentials {
	access_key_id: String,
	secret_access_key: String,
	session_token: Option<String>,
}

impl fmt::Debug for AwsCredentials {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AwsCredentials")
			.field("access_key_id", &"<redacted>")
			.field("secret_access_key", &"<redacted>")
			.field(
				"session_token",
				&self.session_token.as_ref().map(|_| "<redacted>"),
			)
			.finish()
	}
}

impl AwsCredentials {
	/// Create AWS credentials without a session token.
	pub fn new(access_key_id: impl Into<String>, secret_access_key: impl Into<String>) -> Self {
		Self {
			access_key_id: access_key_id.into(),
			secret_access_key: secret_access_key.into(),
			session_token: None,
		}
	}

	/// Attach a session token to these credentials.
	#[must_use]
	pub fn with_session_token(mut self, session_token: impl Into<String>) -> Self {
		self.session_token = Some(session_token.into());
		self
	}

	/// Load AWS credentials from environment variables.
	///
	/// Reads `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and optional
	/// `AWS_SESSION_TOKEN`.
	///
	/// # Errors
	///
	/// Returns [`ProviderError::Config`] when either required credential
	/// variable is missing.
	pub fn from_env() -> Result<Self> {
		Self::from_env_optional()?.ok_or_else(|| {
			ProviderError::Config(
				"AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY must be set".to_string(),
			)
		})
	}

	/// Load AWS credentials from environment variables when present.
	///
	/// # Errors
	///
	/// Returns [`ProviderError::Config`] when the required credential
	/// variables are incomplete.
	pub fn from_env_optional() -> Result<Option<Self>> {
		let access_key_id = env::var("AWS_ACCESS_KEY_ID").ok();
		let secret_access_key = env::var("AWS_SECRET_ACCESS_KEY").ok();
		let session_token = env::var("AWS_SESSION_TOKEN").ok();

		match (access_key_id, secret_access_key, session_token) {
			(Some(access_key_id), Some(secret_access_key), session_token) => {
				let credentials = AwsCredentials {
					access_key_id,
					secret_access_key,
					session_token,
				};
				Ok(Some(credentials))
			}
			(None, None, None) => Ok(None),
			_ => Err(ProviderError::Config(
				"AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, and AWS_SESSION_TOKEN must form a complete static credential set".to_string(),
			)),
		}
	}

	/// Return the AWS access key ID.
	#[must_use]
	pub fn access_key_id(&self) -> &str {
		&self.access_key_id
	}

	/// Return the AWS secret access key.
	#[must_use]
	pub fn secret_access_key(&self) -> &str {
		&self.secret_access_key
	}

	/// Return the AWS session token, when present.
	#[must_use]
	pub fn session_token(&self) -> Option<&str> {
		self.session_token.as_deref()
	}
}

/// AWS credential resolution strategy.
#[derive(Clone, PartialEq, Eq)]
pub enum AwsCredentialsSource {
	/// Use these credentials directly.
	Static(AwsCredentials),
	/// Use the AWS SDK default credential provider chain.
	DefaultChain {
		/// Optional region override applied to the default SDK config loader.
		region_override: Option<String>,
	},
}

impl fmt::Debug for AwsCredentialsSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Static(_) => f
				.debug_tuple("Static")
				.field(&"<redacted credentials>")
				.finish(),
			Self::DefaultChain { region_override } => f
				.debug_struct("DefaultChain")
				.field("region_override", region_override)
				.finish(),
		}
	}
}

impl AwsCredentialsSource {
	/// Create a default provider-chain credential source.
	#[must_use]
	pub fn default_chain(region_override: Option<String>) -> Self {
		Self::DefaultChain { region_override }
	}

	/// Resolve credentials and region for request signing.
	///
	/// # Errors
	///
	/// Returns an error if the configured credential source cannot provide
	/// credentials.
	pub async fn resolve(&self) -> Result<AwsSigningConfig> {
		match self {
			AwsCredentialsSource::Static(credentials) => Ok(AwsSigningConfig {
				credentials: credentials.clone(),
				region: None,
			}),
			AwsCredentialsSource::DefaultChain { region_override } => {
				let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
				if let Some(region) = region_override {
					loader = loader.region(Region::new(region.clone()));
				}

				let sdk_config = loader.load().await;
				let provider = sdk_config.credentials_provider().ok_or_else(|| {
					ProviderError::Config(
						"AWS default credential provider chain is not configured".to_string(),
					)
				})?;
				let credentials = provider.provide_credentials().await.map_err(|err| {
					ProviderError::Config(format!(
						"failed to load AWS credentials from the default provider chain: {err}"
					))
				})?;
				let mut provider_credentials = AwsCredentials::new(
					credentials.access_key_id().to_string(),
					credentials.secret_access_key().to_string(),
				);
				if let Some(token) = credentials.session_token() {
					provider_credentials =
						provider_credentials.with_session_token(token.to_string());
				}

				Ok(AwsSigningConfig {
					credentials: provider_credentials,
					region: sdk_config
						.region()
						.map(|region| region.as_ref().to_string()),
				})
			}
		}
	}
}

/// Resolved AWS signing configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct AwsSigningConfig {
	/// Credentials used to sign the request.
	pub credentials: AwsCredentials,
	/// Region resolved by the default AWS SDK config loader, if any.
	pub region: Option<String>,
}

impl fmt::Debug for AwsSigningConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AwsSigningConfig")
			.field("credentials", &"<redacted credentials>")
			.field("region", &self.region)
			.finish()
	}
}

#[cfg(test)]
mod tests;
