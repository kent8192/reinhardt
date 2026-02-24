//! Cookie-based message storage backend

use super::MessageStorage;
use crate::messages::message::Message;
use crate::messages::utils::bisect::bisect_keep_right;
use std::collections::VecDeque;

/// Cookie-based message storage
///
/// Messages are serialized to JSON and stored in a cookie.
/// Automatically handles size limits by dropping oldest messages.
pub struct CookieStorage {
	messages: VecDeque<Message>,
	cookie_name: String,
	max_cookie_size: usize,
	not_finished: Vec<Message>,
}

impl CookieStorage {
	/// Default maximum cookie size (4KB)
	pub const DEFAULT_MAX_SIZE: usize = 4096;

	/// Create a new CookieStorage with default settings
	pub fn new() -> Self {
		Self {
			messages: VecDeque::new(),
			cookie_name: "messages".to_string(),
			max_cookie_size: Self::DEFAULT_MAX_SIZE,
			not_finished: Vec::new(),
		}
	}

	/// Set the cookie name
	pub fn with_cookie_name(mut self, name: impl Into<String>) -> Self {
		self.cookie_name = name.into();
		self
	}

	/// Set the maximum cookie size
	pub fn with_max_size(mut self, size: usize) -> Self {
		self.max_cookie_size = size;
		self
	}

	/// Get the cookie name
	pub fn cookie_name(&self) -> &str {
		&self.cookie_name
	}

	/// Get the maximum cookie size
	pub fn max_cookie_size(&self) -> usize {
		self.max_cookie_size
	}

	/// Serialize messages to JSON
	pub fn serialize(&self) -> Result<String, serde_json::Error> {
		serde_json::to_string(&self.messages)
	}

	/// Deserialize messages from JSON
	pub fn deserialize(&mut self, data: &str) -> Result<(), serde_json::Error> {
		self.messages = serde_json::from_str(data)?;
		Ok(())
	}

	/// Remove messages that don't fit in cookie size limit
	pub fn update_cookie(&mut self) -> Result<Vec<Message>, serde_json::Error> {
		let mut serialized = self.serialize()?;
		let mut removed = Vec::new();

		// If messages fit, we're done
		if serialized.len() <= self.max_cookie_size {
			return Ok(removed);
		}

		// Binary search to find how many messages we can keep
		let messages_vec: Vec<_> = self.messages.iter().cloned().collect();
		let keep = bisect_keep_right(&messages_vec, self.max_cookie_size, |msgs| {
			serde_json::to_vec(msgs).unwrap_or_default()
		});

		// Remove messages that don't fit
		while self.messages.len() > keep {
			if let Some(msg) = self.messages.pop_front() {
				removed.push(msg);
			}
		}

		serialized = self.serialize()?;

		// If still too big, remove more aggressively
		while serialized.len() > self.max_cookie_size && !self.messages.is_empty() {
			if let Some(msg) = self.messages.pop_front() {
				removed.push(msg);
			}
			serialized = self.serialize()?;
		}

		Ok(removed)
	}

	/// Update storage and return unstored messages
	///
	/// This applies the cookie size limit and returns any messages
	/// that couldn't be stored.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{CookieStorage, Level, Message, MessageStorage};
	///
	/// let mut storage = CookieStorage::new().with_max_size(100);
	/// storage.add(Message::new(Level::Info, "Short message"));
	/// storage.add(Message::new(Level::Info, "This is a very long message that will exceed the cookie size limit"));
	///
	/// let unstored = storage.update();
	/// assert!(unstored.len() > 0, "Some messages should be unstored");
	/// ```
	pub fn update(&mut self) -> Vec<Message> {
		match self.update_cookie() {
			Ok(removed) => removed,
			Err(_) => {
				// If serialization fails, return all messages as unstored
				self.get_all()
			}
		}
	}

	/// Get cookie value and unstored messages
	///
	/// Returns a tuple of (cookie_value, unstored_messages).
	/// The cookie_value is the serialized JSON string to store in the cookie.
	/// unstored_messages are messages that didn't fit in the size limit.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{CookieStorage, Level, Message, MessageStorage};
	///
	/// let mut storage = CookieStorage::new();
	/// storage.add(Message::new(Level::Info, "Test message"));
	///
	/// let (cookie_value, unstored) = storage.get_cookie_value().unwrap();
	/// assert!(cookie_value.len() > 0);
	/// assert_eq!(unstored.len(), 0);
	/// ```
	pub fn get_cookie_value(&mut self) -> Result<(String, Vec<Message>), serde_json::Error> {
		// First apply size limit
		let unstored = self.update_cookie()?;

		// Serialize remaining messages
		let cookie_value = self.serialize()?;

		Ok((cookie_value, unstored))
	}

	/// Load messages from cookie data
	///
	/// Deserializes messages from a cookie value. If the cookie data is
	/// invalid or empty, this method silently ignores the error and
	/// maintains an empty state (matching Django's behavior).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{CookieStorage, Level, Message, MessageStorage};
	///
	/// let mut storage = CookieStorage::new();
	/// storage.add(Message::new(Level::Info, "Test"));
	/// let (cookie_value, _) = storage.get_cookie_value().unwrap();
	///
	/// let mut new_storage = CookieStorage::new();
	/// new_storage.load_from_cookie(&cookie_value).unwrap();
	/// assert_eq!(new_storage.peek().len(), 1);
	/// ```
	pub fn load_from_cookie(&mut self, cookie_data: &str) -> Result<(), serde_json::Error> {
		// Empty cookie data is valid - just means no messages
		if cookie_data.is_empty() {
			return Ok(());
		}

		// Try to deserialize, but if it fails, just clear storage
		// This matches Django's behavior of ignoring bad cookie data
		match self.deserialize(cookie_data) {
			Ok(_) => Ok(()),
			Err(_) => {
				// Bad cookie data - clear and return Ok
				self.clear();
				Ok(())
			}
		}
	}

	/// Encode text for use in a cookie (RFC 6265 compliant)
	///
	/// Percent-encodes characters that are not allowed in RFC 6265 cookie values:
	/// - Comma (,) -> %2C
	/// - Semicolon (;) -> %3B
	/// - Backslash (\) -> %5C
	/// - Double quote (") -> %22
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::CookieStorage;
	///
	/// let text = r#"Test with special chars: , ; \ ""#;
	/// let encoded = CookieStorage::encode_for_cookie(text);
	///
	/// assert!(!encoded.contains(","));
	/// assert!(!encoded.contains(";"));
	/// assert!(!encoded.contains("\\"));
	/// assert!(!encoded.contains("\""));
	/// ```
	pub fn encode_for_cookie(text: &str) -> String {
		// RFC 6265 prohibits these characters in cookie values
		// Use percent-encoding to replace them with %XX sequences
		let mut result = String::with_capacity(text.len() * 2);
		for c in text.chars() {
			match c {
				',' => result.push_str("%2C"),
				';' => result.push_str("%3B"),
				'\\' => result.push_str("%5C"),
				'"' => result.push_str("%22"),
				_ => result.push(c),
			}
		}
		result
	}
}

impl Default for CookieStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for CookieStorage {
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
		self.not_finished.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::levels::Level;

	#[test]
	fn test_cookie_storage_basic() {
		let mut storage = CookieStorage::new();

		storage.add(Message::new(Level::Info, "Test message"));
		assert_eq!(storage.peek().len(), 1);

		let messages = storage.get_all();
		assert_eq!(messages.len(), 1);
		assert_eq!(storage.peek().len(), 0);
	}

	#[test]
	fn test_cookie_storage_custom_name() {
		let storage = CookieStorage::new().with_cookie_name("custom_messages");
		assert_eq!(storage.cookie_name(), "custom_messages");
	}

	#[test]
	fn test_cookie_storage_max_size() {
		let storage = CookieStorage::new().with_max_size(8192);
		assert_eq!(storage.max_cookie_size(), 8192);
	}

	#[test]
	fn test_cookie_storage_serialize() {
		let mut storage = CookieStorage::new();
		storage.add(Message::new(Level::Info, "Test"));

		let serialized = storage.serialize().unwrap();
		assert!(serialized.contains("Test"));
	}

	#[test]
	fn test_cookie_storage_deserialize() {
		let mut storage = CookieStorage::new();
		storage.add(Message::new(Level::Info, "Test"));

		let serialized = storage.serialize().unwrap();

		let mut storage2 = CookieStorage::new();
		storage2.deserialize(&serialized).unwrap();

		assert_eq!(storage2.peek().len(), 1);
	}
}
