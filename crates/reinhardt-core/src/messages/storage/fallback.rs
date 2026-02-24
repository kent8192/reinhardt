//! Fallback message storage backend
//!
//! Tries to store messages in cookies first, then falls back to session storage
//! if the messages don't fit in the cookie.

use super::{CookieStorage, MessageStorage, SessionStorage};
use crate::messages::message::Message;

/// Storage backend that falls back from cookie to session
///
/// This storage first tries to use CookieStorage. If messages don't fit
/// in the cookie (due to size limits), it falls back to SessionStorage.
pub struct FallbackStorage {
	cookie_storage: CookieStorage,
	session_storage: SessionStorage,
	used_storage: UsedStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsedStorage {
	None,
	Cookie,
	Session,
	Both,
}

impl FallbackStorage {
	/// Create a new FallbackStorage
	pub fn new() -> Self {
		Self {
			cookie_storage: CookieStorage::new(),
			session_storage: SessionStorage::new(),
			used_storage: UsedStorage::None,
		}
	}

	/// Create FallbackStorage with custom cookie name
	pub fn with_cookie_name(mut self, name: impl Into<String>) -> Self {
		self.cookie_storage = self.cookie_storage.with_cookie_name(name);
		self
	}

	/// Create FallbackStorage with custom session key
	pub fn with_session_key(mut self, key: impl Into<String>) -> Self {
		self.session_storage = self.session_storage.with_session_key(key);
		self
	}

	/// Create FallbackStorage with custom cookie size limit
	pub fn with_max_cookie_size(mut self, size: usize) -> Self {
		self.cookie_storage = self.cookie_storage.with_max_size(size);
		self
	}

	/// Get messages from cookie storage
	pub fn get_from_cookie(&mut self) -> Vec<Message> {
		self.cookie_storage.get_all()
	}

	/// Get messages from session storage
	pub fn get_from_session(&mut self) -> Vec<Message> {
		self.session_storage.get_all()
	}

	/// Store messages with fallback logic
	pub fn store(&mut self) -> Result<(), serde_json::Error> {
		// Try to fit messages in cookie
		let removed = self.cookie_storage.update_cookie()?;

		// If some messages were removed, store them in session
		if !removed.is_empty() {
			for msg in removed {
				self.session_storage.add(msg);
			}
			self.used_storage = UsedStorage::Both;
		} else if !self.cookie_storage.peek().is_empty() {
			self.used_storage = UsedStorage::Cookie;
		} else if !self.session_storage.peek().is_empty() {
			self.used_storage = UsedStorage::Session;
		}

		Ok(())
	}

	/// Update storage and return unstored messages
	///
	/// This attempts to store messages using the fallback logic and returns
	/// any messages that could not be stored.
	pub fn update(&mut self) -> Vec<Message> {
		// Store messages with fallback logic
		match self.store() {
			Ok(_) => Vec::new(), // All messages successfully stored
			Err(_) => {
				// If storage fails, return all messages as unstored
				self.get_all()
			}
		}
	}

	/// Get a string indicating which storage backend(s) were used
	///
	/// Returns one of: "none", "cookie", "session", "both"
	pub fn get_used_storage(&self) -> &str {
		match self.used_storage {
			UsedStorage::None => "none",
			UsedStorage::Cookie => "cookie",
			UsedStorage::Session => "session",
			UsedStorage::Both => "both",
		}
	}

	/// Get mutable reference to session storage
	///
	/// This allows direct manipulation of the session storage backend.
	pub fn session_storage_mut(&mut self) -> &mut SessionStorage {
		&mut self.session_storage
	}

	/// Flush all used storage backends
	///
	/// Clears messages from both cookie and session storage.
	pub fn flush_used_backends(&mut self) {
		self.cookie_storage.clear();
		self.session_storage.clear();
		self.used_storage = UsedStorage::None;
	}
}

impl Default for FallbackStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for FallbackStorage {
	fn add(&mut self, message: Message) {
		// Initially add to cookie storage
		// The store() method will handle fallback to session if needed
		self.cookie_storage.add(message);
	}

	fn get_all(&mut self) -> Vec<Message> {
		let mut messages = self.get_from_cookie();
		messages.extend(self.get_from_session());
		self.used_storage = UsedStorage::None;
		messages
	}

	fn peek(&self) -> Vec<Message> {
		let mut messages = self.cookie_storage.peek();
		messages.extend(self.session_storage.peek());
		messages
	}

	fn clear(&mut self) {
		self.cookie_storage.clear();
		self.session_storage.clear();
		self.used_storage = UsedStorage::None;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::levels::Level;

	#[test]
	fn test_fallback_storage_basic() {
		let mut storage = FallbackStorage::new();

		storage.add(Message::new(Level::Info, "Test message"));
		assert_eq!(storage.peek().len(), 1);

		let messages = storage.get_all();
		assert_eq!(messages.len(), 1);
		assert_eq!(storage.peek().len(), 0);
	}

	#[test]
	fn test_fallback_storage_custom_names() {
		let storage = FallbackStorage::new()
			.with_cookie_name("custom_messages")
			.with_session_key("custom_session_key");

		assert_eq!(storage.cookie_storage.cookie_name(), "custom_messages");
		assert_eq!(storage.session_storage.session_key(), "custom_session_key");
	}

	#[test]
	fn test_fallback_storage_size_limit() {
		let mut storage = FallbackStorage::new().with_max_cookie_size(100);

		// Add messages that will exceed cookie size
		for i in 0..20 {
			storage.add(Message::new(Level::Info, format!("Test message {}", i)));
		}

		// Store should trigger fallback
		storage.store().unwrap();

		// Should have messages in both storages if limit exceeded
		let all_messages = storage.peek();
		assert!(!all_messages.is_empty());
	}

	#[test]
	fn test_fallback_clear() {
		let mut storage = FallbackStorage::new();

		storage.add(Message::new(Level::Info, "Test 1"));
		storage.add(Message::new(Level::Info, "Test 2"));

		assert_eq!(storage.peek().len(), 2);

		storage.clear();
		assert_eq!(storage.peek().len(), 0);
	}
}
