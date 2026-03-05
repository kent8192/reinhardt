//! Session-based authentication
//!
//! Provides session management for storing user authentication state

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Session ID type
pub type SessionId = String;

/// Session data stored in the backend
///
/// # Examples
///
/// ```
/// use reinhardt_auth::session::Session;
///
/// let session = Session::new();
/// assert!(session.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
	/// Session data as key-value pairs
	pub data: HashMap<String, serde_json::Value>,
}

impl Session {
	/// Create a new empty session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	///
	/// let session = Session::new();
	/// assert!(session.is_empty());
	/// ```
	pub fn new() -> Self {
		Self {
			data: HashMap::new(),
		}
	}

	/// Set a value in the session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	/// use serde_json::json;
	///
	/// let mut session = Session::new();
	/// session.set("user_id", json!("123"));
	/// assert_eq!(session.get("user_id"), Some(&json!("123")));
	/// ```
	pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
		self.data.insert(key.into(), value);
	}

	/// Get a value from the session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	/// use serde_json::json;
	///
	/// let mut session = Session::new();
	/// session.set("user_id", json!("123"));
	/// assert_eq!(session.get("user_id"), Some(&json!("123")));
	/// assert_eq!(session.get("missing"), None);
	/// ```
	pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
		self.data.get(key)
	}

	/// Remove a value from the session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	/// use serde_json::json;
	///
	/// let mut session = Session::new();
	/// session.set("user_id", json!("123"));
	/// assert_eq!(session.remove("user_id"), Some(json!("123")));
	/// assert_eq!(session.get("user_id"), None);
	/// ```
	pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
		self.data.remove(key)
	}

	/// Check if session is empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	/// use serde_json::json;
	///
	/// let mut session = Session::new();
	/// assert!(session.is_empty());
	///
	/// session.set("key", json!("value"));
	/// assert!(!session.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.data.is_empty()
	}

	/// Clear all session data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::Session;
	/// use serde_json::json;
	///
	/// let mut session = Session::new();
	/// session.set("key", json!("value"));
	/// session.clear();
	/// assert!(session.is_empty());
	/// ```
	pub fn clear(&mut self) {
		self.data.clear();
	}
}

impl Default for Session {
	fn default() -> Self {
		Self::new()
	}
}

/// Session store trait for different backends
#[async_trait]
pub trait SessionStore: Send + Sync {
	/// Load session data by session ID
	async fn load(&self, session_id: &SessionId) -> Option<Session>;

	/// Save session data
	async fn save(&self, session_id: &SessionId, session: &Session);

	/// Delete session data
	async fn delete(&self, session_id: &SessionId);

	/// Create a new session ID
	fn create_session_id(&self) -> SessionId {
		Uuid::new_v4().to_string()
	}
}

/// In-memory session store for testing and development
///
/// # Examples
///
/// ```
/// use reinhardt_auth::session::{InMemorySessionStore, SessionStore};
///
/// #[tokio::main]
/// async fn main() {
///     let store = InMemorySessionStore::new();
///     let session_id = store.create_session_id();
///
///     let session = store.load(&session_id).await;
///     assert!(session.is_none());
/// }
/// ```
pub struct InMemorySessionStore {
	sessions: Arc<Mutex<HashMap<SessionId, Session>>>,
}

impl InMemorySessionStore {
	/// Create a new in-memory session store
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::session::InMemorySessionStore;
	///
	/// let store = InMemorySessionStore::new();
	/// ```
	pub fn new() -> Self {
		Self {
			sessions: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

impl Default for InMemorySessionStore {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
	async fn load(&self, session_id: &SessionId) -> Option<Session> {
		let sessions = self.sessions.lock().await;
		sessions.get(session_id).cloned()
	}

	async fn save(&self, session_id: &SessionId, session: &Session) {
		let mut sessions = self.sessions.lock().await;
		sessions.insert(session_id.clone(), session.clone());
	}

	async fn delete(&self, session_id: &SessionId) {
		let mut sessions = self.sessions.lock().await;
		sessions.remove(session_id);
	}
}

/// Session key constant for storing user ID
pub const SESSION_KEY_USER_ID: &str = "_auth_user_id";

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_session_new() {
		let session = Session::new();
		assert!(session.is_empty());
	}

	#[test]
	fn test_session_set_get() {
		let mut session = Session::new();
		session.set("key", serde_json::json!("value"));
		assert_eq!(session.get("key"), Some(&serde_json::json!("value")));
	}

	#[test]
	fn test_session_remove() {
		let mut session = Session::new();
		session.set("key", serde_json::json!("value"));
		assert_eq!(session.remove("key"), Some(serde_json::json!("value")));
		assert!(session.is_empty());
	}

	#[test]
	fn test_session_clear() {
		let mut session = Session::new();
		session.set("key1", serde_json::json!("value1"));
		session.set("key2", serde_json::json!("value2"));
		session.clear();
		assert!(session.is_empty());
	}

	#[tokio::test]
	async fn test_in_memory_session_store() {
		let store = InMemorySessionStore::new();
		let session_id = store.create_session_id();

		let mut session = Session::new();
		session.set("user_id", serde_json::json!("123"));

		store.save(&session_id, &session).await;
		let loaded = store.load(&session_id).await;
		assert!(loaded.is_some());
		assert_eq!(
			loaded.unwrap().get("user_id"),
			Some(&serde_json::json!("123"))
		);

		store.delete(&session_id).await;
		assert!(store.load(&session_id).await.is_none());
	}

	#[tokio::test]
	async fn test_session_store_create_session_id() {
		let store = InMemorySessionStore::new();
		let id1 = store.create_session_id();
		let id2 = store.create_session_id();

		assert_ne!(id1, id2);
		assert!(!id1.is_empty());
		assert!(!id2.is_empty());
	}
}
