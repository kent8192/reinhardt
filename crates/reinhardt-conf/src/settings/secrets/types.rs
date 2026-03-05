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

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SecretError {
	NotFound(String),
	Provider(String),
	ProviderError(String),
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

pub type SecretResult<T> = Result<T, SecretError>;

pub struct SecretManager;
#[derive(Debug, Clone, Default)]
pub struct SecretMetadata {
	pub created_at: Option<chrono::DateTime<chrono::Utc>>,
	pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct SecretVersion;

#[async_trait]
pub trait SecretProvider: Send + Sync {
	async fn get_secret(&self, name: &str) -> SecretResult<SecretString>;
	async fn get_secret_with_metadata(
		&self,
		name: &str,
	) -> SecretResult<(SecretString, SecretMetadata)>;
	async fn set_secret(&self, name: &str, value: SecretString) -> SecretResult<()>;
	async fn delete_secret(&self, name: &str) -> SecretResult<()>;
	async fn list_secrets(&self) -> SecretResult<Vec<String>>;
	fn exists(&self, name: &str) -> bool;
	fn name(&self) -> &str;
}
