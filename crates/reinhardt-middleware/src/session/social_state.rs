//! Async session-backed OAuth state storage.

use async_trait::async_trait;
use chrono::Utc;
use reinhardt_auth::social::{ContextualStateData, SocialAuthError, StateData, StateStore};
use serde::{Deserialize, Serialize};

use super::{AtomicSessionBackend, SessionData};

const DEFAULT_KEY_PREFIX: &str = "_social_auth_state:";
const STATE_PAYLOAD_KEY: &str = "oauth_state";

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
enum StoredStateData {
	Legacy(StateData),
	Contextual(ContextualStateData),
}

impl StoredStateData {
	fn state_data(&self) -> &StateData {
		match self {
			Self::Legacy(data) => data,
			Self::Contextual(data) => data.state_data(),
		}
	}
}

/// State store backed by an async session backend with atomic take support.
pub struct AsyncSessionStateStore<B: AtomicSessionBackend> {
	backend: B,
	key_prefix: String,
}

impl<B: AtomicSessionBackend> AsyncSessionStateStore<B> {
	/// Creates a state store with the default OAuth session key prefix.
	pub fn new(backend: B) -> Self {
		Self {
			backend,
			key_prefix: DEFAULT_KEY_PREFIX.to_string(),
		}
	}

	/// Creates a state store with a custom OAuth session key prefix.
	pub fn with_prefix(backend: B, prefix: impl Into<String>) -> Self {
		Self {
			backend,
			key_prefix: prefix.into(),
		}
	}

	fn session_key(&self, state: &str) -> String {
		format!("{}{}", self.key_prefix, state)
	}

	fn session_for_payload(
		&self,
		payload: StoredStateData,
	) -> Result<SessionData, SocialAuthError> {
		let state_data = payload.state_data();
		let state = state_data.state.clone();
		let ttl = (state_data.expires_at - Utc::now())
			.to_std()
			.map_err(|_| SocialAuthError::InvalidState)?;
		if ttl.is_zero() {
			return Err(SocialAuthError::InvalidState);
		}

		let mut session = SessionData::new(ttl);
		session.id = self.session_key(&state);
		session
			.set(STATE_PAYLOAD_KEY.to_string(), payload)
			.map_err(|error| SocialAuthError::Storage(error.to_string()))?;
		Ok(session)
	}

	fn decode_payload(&self, session: SessionData) -> Result<StoredStateData, SocialAuthError> {
		let value = session
			.data
			.get(STATE_PAYLOAD_KEY)
			.cloned()
			.ok_or(SocialAuthError::InvalidState)?;
		serde_json::from_value(value).map_err(|error| SocialAuthError::Storage(error.to_string()))
	}
}

#[async_trait]
impl<B: AtomicSessionBackend + 'static> StateStore for AsyncSessionStateStore<B> {
	async fn store(&self, data: StateData) -> Result<(), SocialAuthError> {
		let session = self.session_for_payload(StoredStateData::Legacy(data))?;
		self.backend
			.save(&session)
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))
	}

	async fn retrieve(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let session = self
			.backend
			.load(&self.session_key(state))
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))?
			.ok_or(SocialAuthError::InvalidState)?;

		match self.decode_payload(session)? {
			StoredStateData::Legacy(data) if !data.is_expired() => Ok(data),
			StoredStateData::Legacy(_) | StoredStateData::Contextual(_) => {
				Err(SocialAuthError::InvalidState)
			}
		}
	}

	async fn remove(&self, state: &str) -> Result<(), SocialAuthError> {
		self.backend
			.destroy(&self.session_key(state))
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))
	}

	async fn consume(&self, state: &str) -> Result<StateData, SocialAuthError> {
		let session = self
			.backend
			.take(&self.session_key(state))
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))?
			.ok_or(SocialAuthError::InvalidState)?;

		match self.decode_payload(session)? {
			StoredStateData::Legacy(data) if !data.is_expired() => Ok(data),
			StoredStateData::Legacy(_) | StoredStateData::Contextual(_) => {
				Err(SocialAuthError::InvalidState)
			}
		}
	}

	async fn store_contextual(&self, data: ContextualStateData) -> Result<(), SocialAuthError> {
		let session = self.session_for_payload(StoredStateData::Contextual(data))?;
		self.backend
			.save(&session)
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))
	}

	async fn consume_contextual(
		&self,
		state: &str,
	) -> Result<ContextualStateData, SocialAuthError> {
		let session = self
			.backend
			.take(&self.session_key(state))
			.await
			.map_err(|error| SocialAuthError::Storage(error.to_string()))?
			.ok_or(SocialAuthError::InvalidState)?;

		match self.decode_payload(session)? {
			StoredStateData::Contextual(data) if !data.state_data().is_expired() => Ok(data),
			StoredStateData::Legacy(_) | StoredStateData::Contextual(_) => {
				Err(SocialAuthError::InvalidState)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::session::AsyncSessionBackend;
	use async_trait::async_trait;
	use reinhardt_auth::social::flow::{ContextualStateData, StateData, StateStore};
	use std::collections::HashMap;
	use std::sync::{Arc, Mutex};
	use std::time::{Duration, SystemTime};

	#[derive(Clone, Default)]
	struct TestAtomicBackend {
		sessions: Arc<Mutex<HashMap<String, SessionData>>>,
	}

	#[async_trait]
	impl AsyncSessionBackend for TestAtomicBackend {
		async fn load(&self, id: &str) -> reinhardt_http::Result<Option<SessionData>> {
			Ok(self.sessions.lock().unwrap().get(id).cloned())
		}

		async fn save(&self, session: &SessionData) -> reinhardt_http::Result<()> {
			self.sessions
				.lock()
				.unwrap()
				.insert(session.id.clone(), session.clone());
			Ok(())
		}

		async fn destroy(&self, id: &str) -> reinhardt_http::Result<()> {
			self.sessions.lock().unwrap().remove(id);
			Ok(())
		}

		async fn touch(&self, id: &str, ttl: Duration) -> reinhardt_http::Result<()> {
			if let Some(session) = self.sessions.lock().unwrap().get_mut(id) {
				session.touch(ttl);
			}
			Ok(())
		}
	}

	#[async_trait]
	impl AtomicSessionBackend for TestAtomicBackend {
		async fn take(&self, id: &str) -> reinhardt_http::Result<Option<SessionData>> {
			let session = self.sessions.lock().unwrap().remove(id);
			Ok(session.filter(|data| data.expires_at > SystemTime::now()))
		}
	}

	#[tokio::test]
	async fn contextual_state_has_one_winner_across_adapter_instances() {
		let backend = TestAtomicBackend::default();
		let first = AsyncSessionStateStore::new(backend.clone());
		let second = AsyncSessionStateStore::new(backend);
		first
			.store_contextual(
				ContextualStateData::new(
					StateData::new("state-1".to_string(), None, None),
					"github".to_string(),
					b"binding",
					b"context".to_vec(),
				)
				.unwrap(),
			)
			.await
			.unwrap();

		let (left, right) = tokio::join!(
			first.consume_contextual("state-1"),
			second.consume_contextual("state-1"),
		);
		let contexts: Vec<Vec<u8>> = [left, right]
			.into_iter()
			.filter_map(Result::ok)
			.map(|record| record.context().to_vec())
			.collect();

		assert_eq!(contexts, vec![b"context".to_vec()]);
	}

	#[tokio::test]
	async fn legacy_state_round_trips_through_async_adapter() {
		let store = AsyncSessionStateStore::new(TestAtomicBackend::default());
		let data = StateData::new(
			"state-1".to_string(),
			Some("nonce".to_string()),
			Some("verifier".to_string()),
		);
		store.store(data).await.unwrap();

		let retrieved = store.retrieve("state-1").await.unwrap();
		assert_eq!(retrieved.state, "state-1");
		assert_eq!(retrieved.nonce.as_deref(), Some("nonce"));
		assert_eq!(retrieved.code_verifier.as_deref(), Some("verifier"));

		let consumed = store.consume("state-1").await.unwrap();
		assert_eq!(consumed.state, "state-1");
		assert!(matches!(
			store.consume("state-1").await,
			Err(reinhardt_auth::social::SocialAuthError::InvalidState),
		));
	}

	#[tokio::test]
	async fn malformed_payload_maps_to_storage_error() {
		let backend = TestAtomicBackend::default();
		let store = AsyncSessionStateStore::new(backend.clone());
		let mut session = SessionData::new(Duration::from_secs(60));
		session.id = store.session_key("invalid");
		session.data.insert(
			STATE_PAYLOAD_KEY.to_string(),
			serde_json::json!({"kind": "invalid"}),
		);
		backend.save(&session).await.unwrap();

		let result = store.consume_contextual("invalid").await;

		assert!(matches!(
			result,
			Err(reinhardt_auth::social::SocialAuthError::Storage(_)),
		));
	}
}
