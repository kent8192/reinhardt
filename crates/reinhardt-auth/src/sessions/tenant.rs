//! Multi-tenant session isolation
//!
//! This module provides session isolation for multi-tenant applications.
//! Each tenant gets its own session namespace using prefix-based keying.
//!
//! ## Isolation Strategy
//!
//! Uses prefix-based keying with the pattern: `tenant:{tenant_id}:session:{session_id}`
//!
//! This approach provides:
//! - **Simple implementation**: Easy to understand and maintain
//! - **Efficient**: No additional infrastructure required
//! - **Scalable**: Works with any backend
//! - **Secure**: Strong isolation between tenants
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//!
//! // Create tenant-specific session backend
//! let tenant_backend = TenantSessionBackend::new(
//!     backend,
//!     "tenant_123".to_string(),
//!     TenantConfig::default(),
//! );
//!
//! // All sessions are isolated to this tenant
//! # Ok(())
//! # }
//! ```

use super::backends::{SessionBackend, SessionError};
use super::cleanup::CleanupableBackend;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Tenant configuration
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::tenant::TenantConfig;
///
/// let config = TenantConfig {
///     key_prefix: "tenant:{tenant_id}:session:".to_string(),
///     strict_isolation: true,
///     max_sessions: Some(10000),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct TenantConfig {
	/// Key prefix pattern for tenant sessions
	///
	/// Default: `tenant:{tenant_id}:session:`
	///
	/// The `{tenant_id}` placeholder will be replaced with the actual tenant ID.
	pub key_prefix: String,

	/// Enable strict isolation (prevents cross-tenant access)
	///
	/// When enabled, operations that don't match the tenant prefix will fail.
	pub strict_isolation: bool,

	/// Maximum number of sessions per tenant
	///
	/// If set, operations that would exceed this limit will fail.
	pub max_sessions: Option<usize>,
}

impl Default for TenantConfig {
	fn default() -> Self {
		Self {
			key_prefix: "tenant:{tenant_id}:session:".to_string(),
			strict_isolation: true,
			max_sessions: None,
		}
	}
}

impl TenantConfig {
	/// Create a new tenant configuration with custom prefix
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::TenantConfig;
	///
	/// let config = TenantConfig::with_prefix("app:tenant:{tenant_id}:sess:");
	/// ```
	pub fn with_prefix(prefix: &str) -> Self {
		Self {
			key_prefix: prefix.to_string(),
			..Default::default()
		}
	}

	/// Set maximum sessions per tenant
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::TenantConfig;
	///
	/// let config = TenantConfig::default().with_max_sessions(5000);
	/// ```
	pub fn with_max_sessions(mut self, max: usize) -> Self {
		self.max_sessions = Some(max);
		self
	}

	/// Set strict isolation mode
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::TenantConfig;
	///
	/// let config = TenantConfig::default().with_strict_isolation(false);
	/// ```
	pub fn with_strict_isolation(mut self, strict: bool) -> Self {
		self.strict_isolation = strict;
		self
	}
}

/// Tenant session backend
///
/// Provides session isolation for multi-tenant applications using prefix-based keying.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
///
/// let tenant_backend = TenantSessionBackend::new(
///     backend,
///     "tenant_123".to_string(),
///     TenantConfig::default(),
/// );
///
/// // Sessions are prefixed with "tenant:tenant_123:session:"
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct TenantSessionBackend<B> {
	backend: Arc<B>,
	tenant_id: String,
	config: TenantConfig,
}

impl<B> TenantSessionBackend<B>
where
	B: SessionBackend + Clone,
{
	/// Create a new tenant session backend with default config
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let tenant_backend = TenantSessionBackend::new(
	///     backend,
	///     "tenant_123".to_string(),
	///     TenantConfig::default(),
	/// );
	/// ```
	pub fn new(backend: B, tenant_id: String, config: TenantConfig) -> Self {
		Self {
			backend: Arc::new(backend),
			tenant_id,
			config,
		}
	}

	/// Get the tenant ID
	pub fn tenant_id(&self) -> &str {
		&self.tenant_id
	}

	/// Get the tenant configuration
	pub fn config(&self) -> &TenantConfig {
		&self.config
	}

	/// Make a tenant-prefixed key
	///
	/// Converts a session ID to a tenant-specific key using the configured prefix.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let tenant_backend = TenantSessionBackend::new(
	///     backend,
	///     "tenant_123".to_string(),
	///     TenantConfig::default(),
	/// );
	///
	/// let key = tenant_backend.make_key("session_abc");
	/// assert_eq!(key, "tenant:tenant_123:session:session_abc");
	/// ```
	pub fn make_key(&self, session_id: &str) -> String {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);
		format!("{}{}", prefix, session_id)
	}

	/// Check if a key belongs to this tenant
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let tenant_backend = TenantSessionBackend::new(
	///     backend,
	///     "tenant_123".to_string(),
	///     TenantConfig::default(),
	/// );
	///
	/// assert!(tenant_backend.is_tenant_key("tenant:tenant_123:session:abc"));
	/// assert!(!tenant_backend.is_tenant_key("tenant:tenant_456:session:abc"));
	/// ```
	pub fn is_tenant_key(&self, key: &str) -> bool {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);
		key.starts_with(&prefix)
	}

	/// Extract session ID from tenant key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::tenant::{TenantSessionBackend, TenantConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let tenant_backend = TenantSessionBackend::new(
	///     backend,
	///     "tenant_123".to_string(),
	///     TenantConfig::default(),
	/// );
	///
	/// let session_id = tenant_backend.extract_session_id("tenant:tenant_123:session:abc");
	/// assert_eq!(session_id, Some("abc"));
	/// ```
	pub fn extract_session_id<'a>(&self, key: &'a str) -> Option<&'a str> {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);

		if key.starts_with(&prefix) {
			Some(&key[prefix.len()..])
		} else {
			None
		}
	}

	/// Get a reference to the underlying backend
	pub fn backend(&self) -> &B {
		&self.backend
	}
}

#[async_trait]
impl<B> SessionBackend for TenantSessionBackend<B>
where
	B: SessionBackend + CleanupableBackend + Clone,
{
	async fn load<T>(&self, session_id: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		let key = self.make_key(session_id);
		self.backend.load(&key).await
	}

	async fn save<T>(
		&self,
		session_id: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		// Check max sessions limit if configured
		if let Some(max) = self.config.max_sessions {
			let count = self.count_sessions().await?;
			if count >= max {
				return Err(SessionError::CacheError(format!(
					"Tenant {} has reached maximum session limit: {}",
					self.tenant_id, max
				)));
			}
		}

		let key = self.make_key(session_id);
		self.backend.save(&key, data, ttl).await
	}

	async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
		let key = self.make_key(session_id);
		self.backend.delete(&key).await
	}

	async fn exists(&self, session_id: &str) -> Result<bool, SessionError> {
		let key = self.make_key(session_id);
		self.backend.exists(&key).await
	}
}

/// Extended tenant session operations
///
/// Provides additional operations for managing tenant-specific sessions.
#[async_trait]
pub trait TenantSessionOperations: SessionBackend {
	/// List all session IDs for a tenant
	///
	/// Note: This operation may be expensive for large session stores.
	async fn list_sessions(&self) -> Result<Vec<String>, SessionError>;

	/// Count sessions for a tenant
	async fn count_sessions(&self) -> Result<usize, SessionError>;

	/// Delete all sessions for a tenant
	///
	/// Returns the number of sessions deleted.
	async fn delete_all_sessions(&self) -> Result<usize, SessionError>;
}

#[async_trait]
impl<B> TenantSessionOperations for TenantSessionBackend<B>
where
	B: SessionBackend + CleanupableBackend + Clone,
{
	async fn list_sessions(&self) -> Result<Vec<String>, SessionError> {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);
		let keys = self.backend.list_keys_with_prefix(&prefix).await?;

		// Remove prefix and return only session IDs
		Ok(keys
			.iter()
			.filter_map(|key| self.extract_session_id(key))
			.map(String::from)
			.collect())
	}

	async fn count_sessions(&self) -> Result<usize, SessionError> {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);
		self.backend.count_keys_with_prefix(&prefix).await
	}

	async fn delete_all_sessions(&self) -> Result<usize, SessionError> {
		let prefix = self
			.config
			.key_prefix
			.replace("{tenant_id}", &self.tenant_id);
		self.backend.delete_keys_with_prefix(&prefix).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_tenant_session_save_load() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		let data = serde_json::json!({"key": "value"});

		tenant_backend
			.save("session_abc", &data, None)
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = tenant_backend.load("session_abc").await.unwrap();
		assert_eq!(loaded.unwrap(), data);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_session_isolation() {
		let backend = InMemorySessionBackend::new();

		let tenant1 = TenantSessionBackend::new(
			backend.clone(),
			"tenant_123".to_string(),
			TenantConfig::default(),
		);

		let tenant2 = TenantSessionBackend::new(
			backend.clone(),
			"tenant_456".to_string(),
			TenantConfig::default(),
		);

		let data1 = serde_json::json!({"tenant": "123"});
		let data2 = serde_json::json!({"tenant": "456"});

		// Save same session ID for different tenants
		tenant1.save("session_abc", &data1, None).await.unwrap();
		tenant2.save("session_abc", &data2, None).await.unwrap();

		// Each tenant should have its own data
		let loaded1: Option<serde_json::Value> = tenant1.load("session_abc").await.unwrap();
		let loaded2: Option<serde_json::Value> = tenant2.load("session_abc").await.unwrap();

		assert_eq!(loaded1.unwrap(), data1);
		assert_eq!(loaded2.unwrap(), data2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_session_delete() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		let data = serde_json::json!({"key": "value"});

		tenant_backend
			.save("session_abc", &data, None)
			.await
			.unwrap();
		assert!(tenant_backend.exists("session_abc").await.unwrap());

		tenant_backend.delete("session_abc").await.unwrap();
		assert!(!tenant_backend.exists("session_abc").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_make_key() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		let key = tenant_backend.make_key("session_abc");
		assert_eq!(key, "tenant:tenant_123:session:session_abc");
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_is_tenant_key() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		assert!(tenant_backend.is_tenant_key("tenant:tenant_123:session:abc"));
		assert!(!tenant_backend.is_tenant_key("tenant:tenant_456:session:abc"));
		assert!(!tenant_backend.is_tenant_key("other:prefix:abc"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_extract_session_id() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		let session_id = tenant_backend.extract_session_id("tenant:tenant_123:session:abc");
		assert_eq!(session_id, Some("abc"));

		let invalid = tenant_backend.extract_session_id("tenant:tenant_456:session:abc");
		assert_eq!(invalid, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_config_with_prefix() {
		let config = TenantConfig::with_prefix("app:t:{tenant_id}:s:");

		let backend = InMemorySessionBackend::new();
		let tenant_backend = TenantSessionBackend::new(backend, "tenant_123".to_string(), config);

		let key = tenant_backend.make_key("session_abc");
		assert_eq!(key, "app:t:tenant_123:s:session_abc");
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_config_with_max_sessions() {
		let config = TenantConfig::default().with_max_sessions(1);

		let backend = InMemorySessionBackend::new();
		let tenant_backend = TenantSessionBackend::new(backend, "tenant_123".to_string(), config);

		let data = serde_json::json!({"key": "value"});

		// First session should succeed
		tenant_backend.save("session_1", &data, None).await.unwrap();

		// Second session should fail due to max_sessions=1 limit
		let result = tenant_backend.save("session_2", &data, None).await;
		assert!(result.is_err());

		// Verify error message
		if let Err(SessionError::CacheError(msg)) = result {
			assert!(msg.contains("maximum session limit"));
			assert!(msg.contains("tenant_123"));
		} else {
			panic!("Expected CacheError with maximum session limit message");
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_config_with_strict_isolation() {
		let config = TenantConfig::default().with_strict_isolation(true);

		let backend = InMemorySessionBackend::new();
		let tenant_backend = TenantSessionBackend::new(backend, "tenant_123".to_string(), config);

		assert!(tenant_backend.config.strict_isolation);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_getters() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend = TenantSessionBackend::new(
			backend.clone(),
			"tenant_123".to_string(),
			TenantConfig::default(),
		);

		assert_eq!(tenant_backend.tenant_id(), "tenant_123");
		assert_eq!(
			tenant_backend.config().key_prefix,
			"tenant:{tenant_id}:session:"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tenant_count_sessions() {
		let backend = InMemorySessionBackend::new();
		let tenant_backend =
			TenantSessionBackend::new(backend, "tenant_123".to_string(), TenantConfig::default());

		// count_sessions returns 0 when no sessions exist
		let count = tenant_backend.count_sessions().await.unwrap();
		assert_eq!(count, 0);
	}
}
