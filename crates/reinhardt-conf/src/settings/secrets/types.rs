//! Secret types that prevent accidental exposure
//!
//! These types wrap sensitive data and ensure it's not accidentally logged
//! or displayed in debug output.
//!
//! Core secret types (`SecretString`, `SecretValue`) are defined in
//! `settings::secret_types` and re-exported here for backward compatibility.

use async_trait::async_trait;
use std::fmt;

// Re-export core secret types from the always-available module
pub use crate::settings::secret_types::{SecretString, SecretValue};

/// Error type for secret management operations.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SecretError {
	/// The requested secret was not found.
	NotFound(String),
	/// A general provider error.
	Provider(String),
	/// An error returned by the secret provider backend.
	ProviderError(String),
	/// A network error occurred while communicating with the provider.
	NetworkError(String),
}

impl fmt::Display for SecretError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SecretError::NotFound(msg) => write!(f, "Secret not found: {}", msg),
			SecretError::Provider(msg) => write!(f, "Provider error: {}", msg),
			SecretError::ProviderError(msg) => write!(f, "Provider error: {}", msg),
			SecretError::NetworkError(msg) => write!(f, "Network error: {}", msg),
		}
	}
}

impl std::error::Error for SecretError {}

/// Result type alias for secret management operations.
pub type SecretResult<T> = Result<T, SecretError>;

/// Manages secret storage, retrieval, and lifecycle operations.
pub struct SecretManager;

/// Metadata associated with a stored secret.
#[derive(Debug, Clone, Default)]
pub struct SecretMetadata {
	/// Timestamp when the secret was created.
	pub created_at: Option<chrono::DateTime<chrono::Utc>>,
	/// Timestamp when the secret was last updated.
	pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Represents a specific version of a secret.
pub struct SecretVersion;

/// Trait for secret storage backends.
#[async_trait]
pub trait SecretProvider: Send + Sync {
	/// Retrieve a secret by name.
	async fn get_secret(&self, name: &str) -> SecretResult<SecretString>;
	/// Retrieve a secret along with its metadata.
	async fn get_secret_with_metadata(
		&self,
		name: &str,
	) -> SecretResult<(SecretString, SecretMetadata)>;
	/// Store or update a secret.
	async fn set_secret(&self, name: &str, value: SecretString) -> SecretResult<()>;
	/// Delete a secret by name.
	async fn delete_secret(&self, name: &str) -> SecretResult<()>;
	/// List all available secret names.
	async fn list_secrets(&self) -> SecretResult<Vec<String>>;
	/// Check whether a secret with the given name exists.
	fn exists(&self, name: &str) -> bool;
	/// Return the name of this provider.
	fn name(&self) -> &str;
}
