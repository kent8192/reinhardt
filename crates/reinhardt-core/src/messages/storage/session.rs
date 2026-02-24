//! Session-based message storage backend

use super::MessageStorage;
use crate::messages::message::Message;
use std::collections::VecDeque;

/// Session-based message storage
///
/// Messages are serialized to JSON and stored in the session.
/// Requires session middleware to be installed.
pub struct SessionStorage {
	messages: VecDeque<Message>,
	session_key: String,
	session_available: bool,
}

impl SessionStorage {
	/// Default session key for storing messages
	pub const DEFAULT_SESSION_KEY: &'static str = "_messages";
	/// Create a new SessionStorage with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::new();
	/// assert_eq!(storage.session_key(), "_messages");
	/// assert!(storage.is_session_available());
	/// ```
	pub fn new() -> Self {
		Self {
			messages: VecDeque::new(),
			session_key: Self::DEFAULT_SESSION_KEY.to_string(),
			session_available: true,
		}
	}
	/// Create SessionStorage without session middleware
	///
	/// This is used for testing error handling when session is not available
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::without_session();
	/// assert!(!storage.is_session_available());
	/// assert!(storage.require_session().is_err());
	/// ```
	pub fn without_session() -> Self {
		Self {
			messages: VecDeque::new(),
			session_key: Self::DEFAULT_SESSION_KEY.to_string(),
			session_available: false,
		}
	}
	/// Set the session key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::new().with_session_key("custom_messages");
	/// assert_eq!(storage.session_key(), "custom_messages");
	/// ```
	pub fn with_session_key(mut self, key: impl Into<String>) -> Self {
		self.session_key = key.into();
		self
	}
	/// Get the session key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::new();
	/// assert_eq!(storage.session_key(), "_messages");
	/// ```
	pub fn session_key(&self) -> &str {
		&self.session_key
	}
	/// Check if session is available
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::new();
	/// assert!(storage.is_session_available());
	/// ```
	pub fn is_session_available(&self) -> bool {
		self.session_available
	}
	/// Serialize messages for session storage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{storage::{SessionStorage, MessageStorage}, Message, Level};
	///
	/// let mut storage = SessionStorage::new();
	/// storage.add(Message::new(Level::Info, "Test"));
	/// let json = storage.serialize_for_session().unwrap();
	/// assert!(json.contains("Test"));
	/// ```
	pub fn serialize_for_session(&self) -> Result<String, serde_json::Error> {
		let messages: Vec<&Message> = self.messages.iter().collect();
		serde_json::to_string(&messages)
	}
	/// Load messages from session data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{storage::{SessionStorage, MessageStorage}, Message, Level};
	///
	/// let mut storage = SessionStorage::new();
	/// storage.add(Message::new(Level::Info, "Test"));
	/// let json = storage.serialize_for_session().unwrap();
	///
	/// let mut new_storage = SessionStorage::new();
	/// new_storage.load_from_session(&json).unwrap();
	/// assert_eq!(new_storage.peek().len(), 1);
	/// ```
	pub fn load_from_session(&mut self, session_data: &str) -> Result<(), serde_json::Error> {
		let messages: Vec<Message> = serde_json::from_str(session_data)?;
		self.messages = messages.into_iter().collect();
		Ok(())
	}
	/// Validate that session middleware is available
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::SessionStorage;
	///
	/// let storage = SessionStorage::new();
	/// assert!(storage.require_session().is_ok());
	///
	/// let storage_no_session = SessionStorage::without_session();
	/// assert!(storage_no_session.require_session().is_err());
	/// ```
	pub fn require_session(&self) -> Result<(), String> {
		if !self.session_available {
			return Err(
				"SessionStorage requires session middleware to be installed. \
                 Add SessionMiddleware to your middleware stack."
					.to_string(),
			);
		}
		Ok(())
	}
}

impl Default for SessionStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for SessionStorage {
	fn add(&mut self, message: Message) {
		self.messages.push_back(message);
	}

	fn get_all(&mut self) -> Vec<Message> {
		self.messages.drain(..).collect()
	}

	fn peek(&self) -> Vec<Message> {
		self.messages.iter().cloned().collect()
	}

	fn clear(&mut self) {
		self.messages.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::levels::Level;

	#[test]
	fn test_session_storage_creation() {
		let storage = SessionStorage::new();
		assert_eq!(storage.session_key(), "_messages");
		assert!(storage.is_session_available());
	}

	#[test]
	fn test_session_storage_custom_key() {
		let storage = SessionStorage::new().with_session_key("custom_messages");
		assert_eq!(storage.session_key(), "custom_messages");
	}

	#[test]
	fn test_session_storage_add_get() {
		let mut storage = SessionStorage::new();
		storage.add(Message::new(Level::Info, "Test message"));

		let messages = storage.peek();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].text, "Test message");
	}

	#[test]
	fn test_session_storage_serialize() {
		let mut storage = SessionStorage::new();
		storage.add(Message::new(Level::Info, "Test"));
		storage.add(Message::new(Level::Error, "Error"));

		let serialized = storage.serialize_for_session().unwrap();
		assert!(serialized.contains("Test"));
		assert!(serialized.contains("Error"));
	}

	#[test]
	fn test_session_storage_round_trip() {
		let mut storage = SessionStorage::new();
		storage.add(Message::new(Level::Success, "Success message"));
		storage.add(Message::new(Level::Warning, "Warning message"));

		let session_data = storage.serialize_for_session().unwrap();

		let mut new_storage = SessionStorage::new();
		new_storage.load_from_session(&session_data).unwrap();

		let messages = new_storage.peek();
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].text, "Success message");
		assert_eq!(messages[1].text, "Warning message");
	}

	#[test]
	fn test_session_storage_without_session() {
		let storage = SessionStorage::without_session();
		assert!(!storage.is_session_available());

		let result = storage.require_session();
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.contains("SessionStorage requires session middleware")
		);
	}

	#[test]
	fn test_session_storage_with_session() {
		let storage = SessionStorage::new();
		assert!(storage.is_session_available());

		let result = storage.require_session();
		assert!(result.is_ok());
	}

	#[test]
	fn test_messages_session_storage_clear() {
		let mut storage = SessionStorage::new();
		storage.add(Message::new(Level::Info, "Message 1"));
		storage.add(Message::new(Level::Info, "Message 2"));

		assert_eq!(storage.peek().len(), 2);

		storage.clear();
		assert_eq!(storage.peek().len(), 0);
	}

	#[test]
	fn test_session_storage_get_all_clears() {
		let mut storage = SessionStorage::new();
		storage.add(Message::new(Level::Info, "Message 1"));
		storage.add(Message::new(Level::Info, "Message 2"));

		let messages = storage.get_all();
		assert_eq!(messages.len(), 2);

		// get_all should clear the storage
		assert_eq!(storage.peek().len(), 0);
	}
}
