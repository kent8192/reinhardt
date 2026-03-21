//! Django-style session object

use super::backends::SessionBackend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Django-style session object with dictionary-like interface
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::Session;
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let mut session = Session::new(backend);
///
/// // Set session data
/// session.set("user_id", 42)?;
/// session.set("username", "alice")?;
///
/// // Get session data
/// let user_id: i32 = session.get("user_id")?.unwrap();
/// assert_eq!(user_id, 42);
///
/// // Check if key exists
/// assert!(session.contains_key("username"));
///
/// // Delete a key
/// session.delete("username");
///
/// // Save session
/// session.save().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Session<B: SessionBackend> {
	backend: B,
	session_key: Option<String>,
	data: HashMap<String, Value>,
	is_modified: bool,
	is_accessed: bool,
	/// Last activity timestamp (UTC)
	last_activity: Option<chrono::DateTime<chrono::Utc>>,
	/// Session timeout in seconds (default: 1800 = 30 minutes)
	timeout: u64,
}

impl<B: SessionBackend> Session<B> {
	/// Create a new session with the given backend
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let session = Session::new(backend);
	/// ```
	pub fn new(backend: B) -> Self {
		Self {
			backend,
			session_key: None,
			data: HashMap::new(),
			is_modified: false,
			is_accessed: false,
			last_activity: Some(chrono::Utc::now()),
			timeout: 1800, // 30 minutes (Django default)
		}
	}

	/// Create a session from an existing session key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let session = Session::from_key(backend, "session_key_123".to_string()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn from_key(
		backend: B,
		session_key: String,
	) -> Result<Self, super::backends::SessionError> {
		let data: HashMap<String, Value> = backend
			.load(&session_key)
			.await?
			.unwrap_or_else(HashMap::new);

		Ok(Self {
			backend,
			session_key: Some(session_key),
			data,
			is_modified: false,
			is_accessed: true,
			last_activity: Some(chrono::Utc::now()),
			timeout: 1800, // 30 minutes (Django default)
		})
	}

	/// Get a value from the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("count", 42)?;
	/// let count: i32 = session.get("count")?.unwrap();
	/// assert_eq!(count, 42);
	/// # Ok(())
	/// # }
	/// ```
	pub fn get<T>(&mut self, key: &str) -> Result<Option<T>, serde_json::Error>
	where
		T: for<'de> Deserialize<'de>,
	{
		self.is_accessed = true;
		self.last_activity = Some(chrono::Utc::now());
		self.data
			.get(key)
			.map(|v| serde_json::from_value(v.clone()))
			.transpose()
	}

	/// Set a value in the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("user_id", 123)?;
	/// session.set("username", "alice")?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn set<T>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error>
	where
		T: Serialize,
	{
		let json_value = serde_json::to_value(value)?;
		self.data.insert(key.to_string(), json_value);
		self.is_modified = true;
		self.is_accessed = true;
		self.last_activity = Some(chrono::Utc::now());
		Ok(())
	}

	/// Delete a key from the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("temp", "value")?;
	/// assert!(session.contains_key("temp"));
	///
	/// session.delete("temp");
	/// assert!(!session.contains_key("temp"));
	/// # Ok(())
	/// # }
	/// ```
	pub fn delete(&mut self, key: &str) -> Option<Value> {
		self.is_modified = true;
		self.is_accessed = true;
		self.data.remove(key)
	}

	/// Check if a key exists in the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("key", "value")?;
	/// assert!(session.contains_key("key"));
	/// assert!(!session.contains_key("nonexistent"));
	/// # Ok(())
	/// # }
	/// ```
	pub fn contains_key(&self, key: &str) -> bool {
		self.data.contains_key(key)
	}

	/// Get the session key (creates one if it doesn't exist)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let key = session.get_or_create_key();
	/// assert!(!key.is_empty());
	/// ```
	pub fn get_or_create_key(&mut self) -> &str {
		if self.session_key.is_none() {
			self.session_key = Some(Self::generate_key());
		}
		self.session_key.as_ref().unwrap()
	}

	/// Generate a new random session key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let key = Session::<InMemorySessionBackend>::generate_key();
	/// assert!(!key.is_empty());
	/// assert!(key.len() > 20);
	/// ```
	pub fn generate_key() -> String {
		Uuid::new_v4().to_string()
	}

	/// Flush the session (delete all data and create new key)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("key", "value")?;
	/// let old_key = session.get_or_create_key().to_string();
	///
	/// session.flush().await?;
	///
	/// // Data is cleared
	/// assert!(!session.contains_key("key"));
	/// // New session key is generated
	/// assert_ne!(session.get_or_create_key(), old_key);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn flush(&mut self) -> Result<(), super::backends::SessionError> {
		// Delete old session if it exists
		if let Some(old_key) = &self.session_key {
			self.backend.delete(old_key).await?;
		}

		// Clear data and create new key
		self.data.clear();
		self.session_key = Some(Self::generate_key());
		self.is_modified = true;

		Ok(())
	}

	/// Cycle the session key (keep data but change key)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("user_id", 123)?;
	/// let old_key = session.get_or_create_key().to_string();
	///
	/// session.cycle_key().await?;
	///
	/// // Data is preserved
	/// let user_id: i32 = session.get("user_id")?.unwrap();
	/// assert_eq!(user_id, 123);
	///
	/// // Session key has changed
	/// assert_ne!(session.get_or_create_key(), old_key);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn cycle_key(&mut self) -> Result<(), super::backends::SessionError> {
		// Delete old session if it exists
		if let Some(old_key) = &self.session_key {
			self.backend.delete(old_key).await?;
		}

		// Generate new key
		self.session_key = Some(Self::generate_key());
		self.is_modified = true;

		Ok(())
	}

	/// Regenerate session ID to prevent session fixation attacks
	///
	/// This method creates a new session ID while preserving all session data.
	/// It should be called after authentication (login) to prevent session fixation attacks.
	///
	/// # Security
	///
	/// Session fixation is an attack where an attacker sets a victim's session ID
	/// to a known value. By regenerating the session ID after login, we ensure that
	/// even if an attacker knew the pre-authentication session ID, it becomes invalid
	/// after the user logs in.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// // User logs in
	/// session.set("user_id", 123)?;
	///
	/// // Regenerate session ID to prevent session fixation
	/// session.regenerate_id().await?;
	///
	/// // Session data is preserved
	/// let user_id: i32 = session.get("user_id")?.unwrap();
	/// assert_eq!(user_id, 123);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn regenerate_id(&mut self) -> Result<(), super::backends::SessionError> {
		// Delegate to cycle_key which implements the same logic
		self.cycle_key().await
	}

	/// Save the session to the backend
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("data", "value")?;
	/// session.save().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn save(&mut self) -> Result<(), super::backends::SessionError> {
		if !self.is_modified {
			return Ok(());
		}

		let key = self.get_or_create_key().to_string();
		self.backend.save(&key, &self.data, Some(3600)).await?;
		self.is_modified = false;

		Ok(())
	}

	/// Check if the session has been modified
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// assert!(!session.is_modified());
	///
	/// session.set("key", "value")?;
	/// assert!(session.is_modified());
	/// # Ok(())
	/// # }
	/// ```
	pub fn is_modified(&self) -> bool {
		self.is_modified
	}

	/// Check if the session has been accessed
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// assert!(!session.is_accessed());
	///
	/// session.get::<String>("key")?;
	/// assert!(session.is_accessed());
	/// # Ok(())
	/// # }
	/// ```
	pub fn is_accessed(&self) -> bool {
		self.is_accessed
	}

	/// Get the current session key (if any)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let session = Session::new(backend);
	///
	/// assert!(session.session_key().is_none());
	/// ```
	pub fn session_key(&self) -> Option<&str> {
		self.session_key.as_deref()
	}

	/// Get all session keys
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("user_id", 123)?;
	/// session.set("username", "alice")?;
	///
	/// let keys = session.keys();
	/// assert_eq!(keys.len(), 2);
	/// assert!(keys.contains(&"user_id".to_string()));
	/// assert!(keys.contains(&"username".to_string()));
	/// # Ok(())
	/// # }
	/// ```
	pub fn keys(&mut self) -> Vec<String> {
		self.is_accessed = true;
		self.data.keys().cloned().collect()
	}

	/// Get all session values
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("count", 42)?;
	/// session.set("name", "test")?;
	///
	/// let values = session.values();
	/// assert_eq!(values.len(), 2);
	/// # Ok(())
	/// # }
	/// ```
	pub fn values(&mut self) -> Vec<&Value> {
		self.is_accessed = true;
		self.data.values().collect()
	}

	/// Get all session key-value pairs
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("user_id", 123)?;
	/// session.set("role", "admin")?;
	///
	/// let items = session.items();
	/// assert_eq!(items.len(), 2);
	///
	/// // Find specific item
	/// let user_id_item = items.iter()
	///     .find(|(k, _)| k.as_str() == "user_id")
	///     .unwrap();
	/// assert_eq!(user_id_item.1.as_i64().unwrap(), 123);
	/// # Ok(())
	/// # }
	/// ```
	pub fn items(&mut self) -> Vec<(&String, &Value)> {
		self.is_accessed = true;
		self.data.iter().collect()
	}

	/// Clear all session data
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("user_id", 123)?;
	/// session.set("username", "alice")?;
	///
	/// assert_eq!(session.keys().len(), 2);
	///
	/// session.clear();
	///
	/// assert_eq!(session.keys().len(), 0);
	/// assert!(session.is_modified());
	/// # Ok(())
	/// # }
	/// ```
	pub fn clear(&mut self) {
		self.data.clear();
		self.is_modified = true;
		self.is_accessed = true;
	}

	/// Manually mark the session as modified
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// assert!(!session.is_modified());
	///
	/// session.mark_modified();
	/// assert!(session.is_modified());
	/// ```
	pub fn mark_modified(&mut self) {
		self.is_modified = true;
	}

	/// Manually mark the session as unmodified
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("key", "value")?;
	/// assert!(session.is_modified());
	///
	/// session.mark_unmodified();
	/// assert!(!session.is_modified());
	/// # Ok(())
	/// # }
	/// ```
	pub fn mark_unmodified(&mut self) {
		self.is_modified = false;
	}

	/// Set session timeout in seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// // Set timeout to 1 hour (3600 seconds)
	/// session.set_timeout(3600);
	/// ```
	pub fn set_timeout(&mut self, timeout: u64) {
		self.timeout = timeout;
	}

	/// Get session timeout in seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let session = Session::new(backend);
	///
	/// // Default timeout is 1800 seconds (30 minutes)
	/// assert_eq!(session.get_timeout(), 1800);
	/// ```
	pub fn get_timeout(&self) -> u64 {
		self.timeout
	}

	/// Update last activity timestamp
	///
	/// This should be called on each request to update the session's last activity time.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// // Update activity timestamp
	/// session.update_activity();
	/// ```
	pub fn update_activity(&mut self) {
		self.last_activity = Some(chrono::Utc::now());
		self.is_modified = true;
	}

	/// Get last activity timestamp
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let session = Session::new(backend);
	///
	/// // Last activity is set on creation
	/// assert!(session.get_last_activity().is_some());
	/// ```
	pub fn get_last_activity(&self) -> Option<chrono::DateTime<chrono::Utc>> {
		self.last_activity
	}

	/// Check if session has timed out
	///
	/// Returns `true` if the session has exceeded its timeout period based on last activity.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// // New session should not be timed out
	/// assert!(!session.is_timed_out());
	///
	/// // Set very short timeout
	/// session.set_timeout(0);
	/// // Still not timed out immediately (last_activity is now)
	/// assert!(!session.is_timed_out());
	/// ```
	pub fn is_timed_out(&self) -> bool {
		if let Some(last_activity) = self.last_activity {
			let now = chrono::Utc::now();
			let elapsed = now.signed_duration_since(last_activity);
			elapsed.num_seconds() as u64 > self.timeout
		} else {
			// No last_activity means session never used, not timed out
			false
		}
	}

	/// Validate session timeout
	///
	/// Returns an error if the session has timed out.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// // Validate timeout (should succeed for new session)
	/// session.validate_timeout()?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn validate_timeout(&self) -> Result<(), super::backends::SessionError> {
		if self.is_timed_out() {
			Err(super::backends::SessionError::SessionExpired)
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;

	#[tokio::test]
	async fn test_session_set_get() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 42).unwrap();
		let user_id: i32 = session.get("user_id").unwrap().unwrap();
		assert_eq!(user_id, 42);
	}

	#[tokio::test]
	async fn test_session_delete() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("key", "value").unwrap();
		assert!(session.contains_key("key"));

		session.delete("key");
		assert!(!session.contains_key("key"));
	}

	#[tokio::test]
	async fn test_session_flush() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("data", "value").unwrap();
		let old_key = session.get_or_create_key().to_string();

		session.flush().await.unwrap();

		assert!(!session.contains_key("data"));
		assert_ne!(session.get_or_create_key(), old_key);
	}

	#[tokio::test]
	async fn test_session_cycle_key() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 123).unwrap();
		let old_key = session.get_or_create_key().to_string();

		session.cycle_key().await.unwrap();

		let user_id: i32 = session.get("user_id").unwrap().unwrap();
		assert_eq!(user_id, 123);
		assert_ne!(session.get_or_create_key(), old_key);
	}

	#[tokio::test]
	async fn test_session_is_modified() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_modified());

		session.set("key", "value").unwrap();
		assert!(session.is_modified());
	}

	#[tokio::test]
	async fn test_session_is_accessed() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_accessed());

		session.get::<String>("key").unwrap();
		assert!(session.is_accessed());
	}

	#[tokio::test]
	async fn test_session_save() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("data", "test_value").unwrap();
		session.save().await.unwrap();

		// Verify data was saved
		let key = session.session_key().unwrap().to_string();
		let loaded: Option<HashMap<String, Value>> = session.backend.load(&key).await.unwrap();
		assert!(loaded.is_some());
	}

	#[tokio::test]
	async fn test_session_keys() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 123).unwrap();
		session.set("username", "alice").unwrap();
		session.set("role", "admin").unwrap();

		let keys = session.keys();
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"user_id".to_string()));
		assert!(keys.contains(&"username".to_string()));
		assert!(keys.contains(&"role".to_string()));
	}

	#[tokio::test]
	async fn test_session_keys_marks_accessed() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_accessed());

		session.set("key", "value").unwrap();
		// Reset accessed flag to verify keys() sets it
		session.is_accessed = false;

		let _keys = session.keys();
		assert!(session.is_accessed());
	}

	#[tokio::test]
	async fn test_session_values() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("count", 42).unwrap();
		session.set("name", "test").unwrap();

		let values = session.values();
		assert_eq!(values.len(), 2);

		// Verify values contain expected data
		let has_42 = values.iter().any(|v| v.as_i64() == Some(42));
		let has_test = values.iter().any(|v| v.as_str() == Some("test"));
		assert!(has_42);
		assert!(has_test);
	}

	#[tokio::test]
	async fn test_session_values_marks_accessed() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_accessed());

		session.set("key", "value").unwrap();
		// Reset accessed flag to verify values() sets it
		session.is_accessed = false;

		let _values = session.values();
		assert!(session.is_accessed());
	}

	#[tokio::test]
	async fn test_session_items() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 123).unwrap();
		session.set("role", "admin").unwrap();

		let items = session.items();
		assert_eq!(items.len(), 2);

		// Find specific item
		let user_id_item = items.iter().find(|(k, _)| k.as_str() == "user_id").unwrap();
		assert_eq!(user_id_item.1.as_i64().unwrap(), 123);

		let role_item = items.iter().find(|(k, _)| k.as_str() == "role").unwrap();
		assert_eq!(role_item.1.as_str().unwrap(), "admin");
	}

	#[tokio::test]
	async fn test_session_items_marks_accessed() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_accessed());

		session.set("key", "value").unwrap();
		// Reset accessed flag to verify items() sets it
		session.is_accessed = false;

		let _items = session.items();
		assert!(session.is_accessed());
	}

	#[tokio::test]
	async fn test_session_clear() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 123).unwrap();
		session.set("username", "alice").unwrap();

		assert_eq!(session.keys().len(), 2);
		assert!(session.is_modified()); // set() marks modified

		session.clear();

		assert_eq!(session.keys().len(), 0);
		assert!(session.is_modified());
		assert!(session.is_accessed());
	}

	#[tokio::test]
	async fn test_session_clear_preserves_session_key() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("data", "value").unwrap();
		let session_key = session.get_or_create_key().to_string();

		session.clear();

		// Session key should remain the same
		assert_eq!(session.get_or_create_key(), session_key);
	}

	#[tokio::test]
	async fn test_session_mark_modified() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		assert!(!session.is_modified());

		session.mark_modified();
		assert!(session.is_modified());
	}

	#[tokio::test]
	async fn test_session_mark_unmodified() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("key", "value").unwrap();
		assert!(session.is_modified());

		session.mark_unmodified();
		assert!(!session.is_modified());
	}

	#[tokio::test]
	async fn test_session_mark_unmodified_prevents_save() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("data", "value").unwrap();
		assert!(session.is_modified());

		session.mark_unmodified();

		// save() should return early without error
		session.save().await.unwrap();

		// Session key should not be created since save didn't actually persist
		assert!(session.session_key().is_none());
	}
}
