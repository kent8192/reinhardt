//! State storage for OAuth2/OIDC CSRF protection
//!
//! Manages state, nonce, and code_verifier with TTL expiration.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;

use crate::sessions::backends::cache::{SessionBackend, SessionError};
use crate::social::core::SocialAuthError;

/// Data stored for each OAuth2 state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateData {
	/// OAuth2 state parameter
	pub state: String,
	/// OIDC nonce parameter (optional)
	pub nonce: Option<String>,
	/// PKCE code verifier (optional)
	pub code_verifier: Option<String>,
	/// Expiration timestamp
	pub expires_at: DateTime<Utc>,
}

impl StateData {
	/// Creates new state data with default TTL (10 minutes)
	pub fn new(state: String, nonce: Option<String>, code_verifier: Option<String>) -> Self {
		Self {
			state,
			nonce,
			code_verifier,
			expires_at: Utc::now() + Duration::minutes(10),
		}
	}

	/// Creates new state data with custom TTL
	pub fn with_ttl(
		state: String,
		nonce: Option<String>,
		code_verifier: Option<String>,
		ttl: Duration,
	) -> Self {
		Self {
			state,
			nonce,
			code_verifier,
			expires_at: Utc::now() + ttl,
		}
	}

	/// Checks if the state has expired
	pub fn is_expired(&self) -> bool {
		Utc::now() > self.expires_at
	}
}

/// OAuth state data with provider and caller-supplied binding context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ContextualStateData {
	state_data: StateData,
	provider_name: String,
	binding_digest: [u8; 32],
	context: Vec<u8>,
}

impl ContextualStateData {
	/// Creates contextual state data from an OAuth state record and binding.
	pub fn new(
		state_data: StateData,
		provider_name: String,
		binding: &[u8],
		context: Vec<u8>,
	) -> Result<Self, SocialAuthError> {
		if binding.is_empty() {
			return Err(SocialAuthError::StateValidation(
				"OAuth state binding must not be empty".to_string(),
			));
		}

		Ok(Self {
			state_data,
			provider_name,
			binding_digest: sha2::Sha256::digest(binding).into(),
			context,
		})
	}

	/// Returns the embedded OAuth state data.
	pub fn state_data(&self) -> &StateData {
		&self.state_data
	}

	/// Returns the provider recorded when authorization began.
	pub fn provider_name(&self) -> &str {
		&self.provider_name
	}

	/// Returns the opaque caller-supplied context bytes.
	pub fn context(&self) -> &[u8] {
		&self.context
	}

	/// Compares a binding with the stored digest in constant time.
	pub fn binding_matches(&self, binding: &[u8]) -> bool {
		if binding.is_empty() {
			return false;
		}

		let candidate: [u8; 32] = sha2::Sha256::digest(binding).into();
		self.binding_digest.ct_eq(&candidate).into()
	}

	/// Splits the OAuth state data and opaque context for a validated callback.
	pub fn into_parts(self) -> (StateData, Vec<u8>) {
		(self.state_data, self.context)
	}
}

/// Trait for state storage implementations
#[async_trait]
pub trait StateStore: Send + Sync {
	/// Stores state data
	async fn store(&self, data: StateData) -> Result<(), SocialAuthError>;

	/// Retrieves state data by state string
	async fn retrieve(&self, state: &str) -> Result<StateData, SocialAuthError>;

	/// Removes state data by state string
	async fn remove(&self, state: &str) -> Result<(), SocialAuthError>;

	/// Atomically consumes state data when the store provides that capability.
	async fn consume(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let data = self.retrieve(state).await?;
		self.remove(state).await?;
		Ok(data)
	}

	/// Stores contextual state data.
	///
	/// Stores that do not support contextual state return a storage error.
	async fn store_contextual(&self, _data: ContextualStateData) -> Result<(), SocialAuthError> {
		Err(SocialAuthError::Storage(
			"Context-aware OAuth state is not supported by this store".to_string(),
		))
	}

	/// Atomically consumes contextual state data.
	///
	/// Stores that do not support contextual state return a storage error.
	async fn consume_contextual(
		&self,
		_state: &str,
	) -> Result<ContextualStateData, SocialAuthError> {
		Err(SocialAuthError::Storage(
			"Context-aware OAuth state is not supported by this store".to_string(),
		))
	}
}

#[derive(Debug, Clone)]
enum StoredStateData {
	Legacy(StateData),
	Contextual(ContextualStateData),
}

impl StoredStateData {
	fn is_expired(&self) -> bool {
		match self {
			Self::Legacy(data) => data.is_expired(),
			Self::Contextual(data) => data.state_data().is_expired(),
		}
	}
}

/// In-memory state store for development and testing
///
/// This implementation is NOT suitable for production use in multi-instance deployments.
/// For production, use a distributed store like Redis or database-backed storage.
#[derive(Debug, Default)]
pub struct InMemoryStateStore {
	store: RwLock<HashMap<String, StoredStateData>>,
}

impl InMemoryStateStore {
	/// Creates a new in-memory state store
	pub fn new() -> Self {
		Self {
			store: RwLock::new(HashMap::new()),
		}
	}

	/// Removes expired entries from the store
	async fn cleanup_expired(&self) {
		let mut store = self.store.write().await;
		store.retain(|_, data| !data.is_expired());
	}
}

#[async_trait]
impl StateStore for InMemoryStateStore {
	async fn store(&self, data: StateData) -> Result<(), SocialAuthError> {
		// Cleanup expired entries before storing
		self.cleanup_expired().await;

		let mut store = self.store.write().await;
		store.insert(data.state.clone(), StoredStateData::Legacy(data));
		Ok(())
	}

	async fn retrieve(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let store = self.store.read().await;
		let data = match store.get(state) {
			Some(StoredStateData::Legacy(data)) => data.clone(),
			Some(StoredStateData::Contextual(_)) | None => {
				return Err(SocialAuthError::InvalidState);
			}
		};

		if data.is_expired() {
			return Err(SocialAuthError::InvalidState);
		}

		Ok(data)
	}

	async fn remove(&self, state: &str) -> Result<(), SocialAuthError> {
		let mut store = self.store.write().await;
		store.remove(state).ok_or(SocialAuthError::InvalidState)?;
		Ok(())
	}

	async fn consume(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let mut store = self.store.write().await;
		match store.remove(state) {
			Some(StoredStateData::Legacy(data)) if !data.is_expired() => Ok(data),
			Some(StoredStateData::Legacy(_)) | Some(StoredStateData::Contextual(_)) | None => {
				Err(SocialAuthError::InvalidState)
			}
		}
	}

	async fn store_contextual(&self, data: ContextualStateData) -> Result<(), SocialAuthError> {
		self.cleanup_expired().await;

		let mut store = self.store.write().await;
		store.insert(
			data.state_data().state.clone(),
			StoredStateData::Contextual(data),
		);
		Ok(())
	}

	async fn consume_contextual(
		&self,
		state: &str,
	) -> Result<ContextualStateData, SocialAuthError> {
		let mut store = self.store.write().await;
		match store.remove(state) {
			Some(StoredStateData::Contextual(data)) if !data.state_data().is_expired() => Ok(data),
			Some(StoredStateData::Legacy(_)) | Some(StoredStateData::Contextual(_)) | None => {
				Err(SocialAuthError::InvalidState)
			}
		}
	}
}

/// Session-based state store for production use
///
/// Integrates with Reinhardt's session management system to persist
/// OAuth2/OIDC state across requests. Each state entry is stored as
/// a session key with a prefix and automatic TTL expiration.
pub struct SessionStateStore<B: SessionBackend> {
	backend: B,
	key_prefix: String,
}

/// Default key prefix for session state entries
const DEFAULT_KEY_PREFIX: &str = "_social_auth_state:";

impl<B: SessionBackend> SessionStateStore<B> {
	/// Creates a new session-based state store with the given backend
	pub fn new(backend: B) -> Self {
		Self {
			backend,
			key_prefix: DEFAULT_KEY_PREFIX.to_string(),
		}
	}

	/// Creates a new session-based state store with a custom key prefix
	pub fn with_prefix(backend: B, prefix: impl Into<String>) -> Self {
		Self {
			backend,
			key_prefix: prefix.into(),
		}
	}

	/// Builds the full session key for a given state parameter
	fn session_key(&self, state: &str) -> String {
		format!("{}{}", self.key_prefix, state)
	}

	/// Computes the TTL in seconds from `StateData::expires_at`
	///
	/// Returns `None` if the state has already expired (TTL <= 0).
	fn compute_ttl(data: &StateData) -> Option<u64> {
		let remaining = data.expires_at - Utc::now();
		let seconds = remaining.num_seconds();
		if seconds > 0 {
			Some(seconds as u64)
		} else {
			None
		}
	}
}

fn map_session_error(err: SessionError) -> SocialAuthError {
	SocialAuthError::Storage(err.to_string())
}

#[async_trait]
impl<B: SessionBackend + 'static> StateStore for SessionStateStore<B> {
	async fn store(&self, data: StateData) -> Result<(), SocialAuthError> {
		let key = self.session_key(&data.state);
		let ttl = Self::compute_ttl(&data);
		self.backend
			.save(&key, &data, ttl)
			.await
			.map_err(map_session_error)
	}

	async fn retrieve(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let key = self.session_key(state);
		let data: Option<StateData> = self.backend.load(&key).await.map_err(map_session_error)?;

		let data = data.ok_or(SocialAuthError::InvalidState)?;

		if data.is_expired() {
			// Clean up the expired entry
			let _ = self.backend.delete(&key).await;
			return Err(SocialAuthError::InvalidState);
		}

		Ok(data)
	}

	async fn remove(&self, state: &str) -> Result<(), SocialAuthError> {
		let key = self.session_key(state);
		self.backend.delete(&key).await.map_err(map_session_error)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::backends::InMemorySessionBackend;
	use rstest::rstest;
	use std::sync::Arc;

	#[rstest]
	#[tokio::test]
	async fn test_state_data_expiration() {
		// Arrange
		let data = StateData::new("test_state".to_string(), None, None);
		let expired_data = StateData::with_ttl(
			"expired_state".to_string(),
			None,
			None,
			Duration::seconds(-1),
		);

		// Act & Assert
		assert!(!data.is_expired());
		assert!(expired_data.is_expired());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_store_retrieve() {
		// Arrange
		let store = InMemoryStateStore::new();
		let data = StateData::new(
			"test_state".to_string(),
			Some("test_nonce".to_string()),
			Some("test_verifier".to_string()),
		);

		// Act
		store.store(data.clone()).await.unwrap();
		let retrieved = store.retrieve("test_state").await.unwrap();

		// Assert
		assert_eq!(retrieved.state, "test_state");
		assert_eq!(retrieved.nonce, Some("test_nonce".to_string()));
		assert_eq!(retrieved.code_verifier, Some("test_verifier".to_string()));
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_store_remove() {
		// Arrange
		let store = InMemoryStateStore::new();
		let data = StateData::new("test_state".to_string(), None, None);
		store.store(data).await.unwrap();

		// Act
		store.remove("test_state").await.unwrap();

		// Assert
		let result = store.retrieve("test_state").await;
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_store_nonexistent() {
		// Arrange
		let store = InMemoryStateStore::new();

		// Act
		let result = store.retrieve("nonexistent").await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_store_expired() {
		// Arrange
		let store = InMemoryStateStore::new();
		let expired_data = StateData::with_ttl(
			"expired_state".to_string(),
			None,
			None,
			Duration::seconds(-1),
		);
		store.store(expired_data).await.unwrap();

		// Act
		let result = store.retrieve("expired_state").await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_contextual_state_round_trip() {
		// Arrange
		let store = InMemoryStateStore::new();
		let data = ContextualStateData::new(
			StateData::new("state-1".to_string(), None, None),
			"github".to_string(),
			b"browser-binding",
			b"link-user-42".to_vec(),
		)
		.unwrap();
		store.store_contextual(data).await.unwrap();

		// Act
		let consumed = store.consume_contextual("state-1").await.unwrap();

		// Assert
		assert_eq!(consumed.provider_name(), "github");
		assert_eq!(consumed.context(), b"link-user-42");
		assert!(consumed.binding_matches(b"browser-binding"));
		assert!(matches!(
			store.consume_contextual("state-1").await,
			Err(SocialAuthError::InvalidState),
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_contextual_consume_has_one_winner() {
		// Arrange
		let store = Arc::new(InMemoryStateStore::new());
		let data = ContextualStateData::new(
			StateData::new("state-1".to_string(), None, None),
			"github".to_string(),
			b"binding",
			Vec::new(),
		)
		.unwrap();
		store.store_contextual(data).await.unwrap();

		// Act
		let (first, second) = tokio::join!(
			store.consume_contextual("state-1"),
			store.consume_contextual("state-1"),
		);

		// Assert
		assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
	}

	#[rstest]
	fn test_contextual_state_rejects_empty_binding() {
		let result = ContextualStateData::new(
			StateData::new("state-1".to_string(), None, None),
			"github".to_string(),
			b"",
			Vec::new(),
		);

		assert!(matches!(result, Err(SocialAuthError::StateValidation(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_contextual_state_rejects_expired_record() {
		let store = InMemoryStateStore::new();
		let data = ContextualStateData::new(
			StateData::with_ttl("expired".to_string(), None, None, Duration::seconds(-1)),
			"github".to_string(),
			b"binding",
			Vec::new(),
		)
		.unwrap();
		store.store_contextual(data).await.unwrap();

		assert!(matches!(
			store.consume_contextual("expired").await,
			Err(SocialAuthError::InvalidState),
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_legacy_consume_has_one_winner() {
		let store = Arc::new(InMemoryStateStore::new());
		store
			.store(StateData::new("state-1".to_string(), None, None))
			.await
			.unwrap();

		let (first, second) = tokio::join!(store.consume("state-1"), store.consume("state-1"));

		assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_rejects_contextual_record() {
		let store = SessionStateStore::new(InMemorySessionBackend::new());
		let data = ContextualStateData::new(
			StateData::new("state-1".to_string(), None, None),
			"github".to_string(),
			b"binding",
			Vec::new(),
		)
		.unwrap();

		assert!(matches!(
			store.store_contextual(data).await,
			Err(SocialAuthError::Storage(_)),
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_expired() {
		// Arrange
		let store = InMemoryStateStore::new();
		let valid_data = StateData::new("valid".to_string(), None, None);
		let expired_data =
			StateData::with_ttl("expired".to_string(), None, None, Duration::seconds(-1));
		store.store(valid_data).await.unwrap();
		store.store(expired_data).await.unwrap();

		// Act
		let new_data = StateData::new("new".to_string(), None, None);
		store.store(new_data).await.unwrap();

		// Assert
		assert!(store.retrieve("valid").await.is_ok());
		assert!(store.retrieve("new").await.is_ok());
		assert!(store.retrieve("expired").await.is_err());
	}

	// SessionStateStore tests

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_store_and_retrieve() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let store = SessionStateStore::new(backend);
		let data = StateData::new(
			"oauth_state_abc".to_string(),
			Some("nonce_123".to_string()),
			Some("verifier_xyz".to_string()),
		);

		// Act
		store.store(data).await.unwrap();
		let retrieved = store.retrieve("oauth_state_abc").await.unwrap();

		// Assert
		assert_eq!(retrieved.state, "oauth_state_abc");
		assert_eq!(retrieved.nonce, Some("nonce_123".to_string()));
		assert_eq!(retrieved.code_verifier, Some("verifier_xyz".to_string()));
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_retrieve_expired_state() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let store = SessionStateStore::new(backend);
		let expired_data = StateData::with_ttl(
			"expired_state".to_string(),
			None,
			None,
			Duration::seconds(-1),
		);
		// Store directly via backend to bypass TTL filtering at store time
		let key = format!("{}{}", DEFAULT_KEY_PREFIX, "expired_state");
		store
			.backend
			.save(&key, &expired_data, Some(300))
			.await
			.unwrap();

		// Act
		let result = store.retrieve("expired_state").await;

		// Assert
		assert!(matches!(result, Err(SocialAuthError::InvalidState)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_retrieve_non_existent() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let store = SessionStateStore::new(backend);

		// Act
		let result = store.retrieve("non_existent_state").await;

		// Assert
		assert!(matches!(result, Err(SocialAuthError::InvalidState)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_delete() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let store = SessionStateStore::new(backend);
		let data = StateData::new("state_to_delete".to_string(), None, None);
		store.store(data).await.unwrap();

		// Act
		store.remove("state_to_delete").await.unwrap();
		let result = store.retrieve("state_to_delete").await;

		// Assert
		assert!(matches!(result, Err(SocialAuthError::InvalidState)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_state_store_custom_key_prefix() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let custom_prefix = "custom_prefix:";
		let store = SessionStateStore::with_prefix(backend.clone(), custom_prefix);
		let data = StateData::new("prefixed_state".to_string(), None, None);

		// Act
		store.store(data).await.unwrap();

		// Assert
		let exists_with_custom_prefix: bool = backend
			.exists("custom_prefix:prefixed_state")
			.await
			.unwrap();
		let exists_with_default_prefix: bool = backend
			.exists("_social_auth_state:prefixed_state")
			.await
			.unwrap();
		assert!(exists_with_custom_prefix);
		assert!(!exists_with_default_prefix);
	}
}
