//! In-memory message storage backend

use super::MessageStorage;
use crate::messages::message::Message;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// In-memory message storage
#[derive(Debug)]
pub struct MemoryStorage {
	messages: Arc<Mutex<VecDeque<Message>>>,
}

impl MemoryStorage {
	/// Create a new MemoryStorage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::storage::MemoryStorage;
	///
	/// let storage = MemoryStorage::new();
	/// // Creates a new storage instance with defaults
	/// ```
	pub fn new() -> Self {
		Self {
			messages: Arc::new(Mutex::new(VecDeque::new())),
		}
	}
}

impl Default for MemoryStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for MemoryStorage {
	fn add(&mut self, message: Message) {
		let mut messages = self.messages.lock().unwrap();
		messages.push_back(message);
	}

	fn get_all(&mut self) -> Vec<Message> {
		let mut messages = self.messages.lock().unwrap();
		messages.drain(..).collect()
	}

	fn peek(&self) -> Vec<Message> {
		let messages = self.messages.lock().unwrap();
		messages.iter().cloned().collect()
	}

	fn clear(&mut self) {
		let mut messages = self.messages.lock().unwrap();
		messages.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::levels::Level;

	#[test]
	fn test_messages_memory_storage() {
		let mut storage = MemoryStorage::new();

		storage.add(Message::new(Level::Info, "Test message"));
		assert_eq!(storage.peek().len(), 1);

		let messages = storage.get_all();
		assert_eq!(messages.len(), 1);
		assert_eq!(storage.peek().len(), 0);
	}

	// Tests from Django messages_tests/base.py - BaseTests
	#[test]
	fn test_add_messages() {
		let mut storage = MemoryStorage::new();

		assert_eq!(storage.peek().len(), 0);

		storage.add(Message::new(Level::Info, "Test message 1"));
		assert_eq!(storage.peek().len(), 1);

		storage.add(Message::new(Level::Info, "Test message 2").with_tags(vec!["tag".to_string()]));
		assert_eq!(storage.peek().len(), 2);
	}

	#[test]
	fn test_no_update() {
		let storage = MemoryStorage::new();

		// No messages added
		let messages = storage.peek();
		assert_eq!(messages.len(), 0);
	}

	#[test]
	fn test_add_update() {
		let mut storage = MemoryStorage::new();

		storage.add(Message::new(Level::Info, "Test message 1"));
		storage.add(Message::new(Level::Info, "Test message 1").with_tags(vec!["tag".to_string()]));

		let messages = storage.peek();
		assert_eq!(messages.len(), 2);
	}

	#[test]
	fn test_existing_add_read_update() {
		let mut storage = MemoryStorage::new();

		// Add initial messages
		storage.add(Message::new(Level::Info, "Test message 1"));
		storage.add(Message::new(Level::Info, "Test message 2").with_tags(vec!["tag".to_string()]));

		// Add another message
		storage.add(Message::new(Level::Info, "Test message 3"));

		// Read (simulates consumption)
		let _messages = storage.get_all();

		// After consumption, storage should be empty
		assert_eq!(storage.peek().len(), 0);
	}

	#[test]
	fn test_existing_read_add_update() {
		let mut storage = MemoryStorage::new();

		// Add initial messages
		storage.add(Message::new(Level::Info, "Test message 1"));
		storage.add(Message::new(Level::Info, "Test message 2").with_tags(vec!["tag".to_string()]));

		// Read (simulates consumption) - but we'll use peek to not consume
		let messages = storage.peek();
		assert_eq!(messages.len(), 2);

		// Add another message after reading
		storage.add(Message::new(Level::Info, "Test message 3"));

		// Should have 3 messages total
		assert_eq!(storage.peek().len(), 3);
	}

	#[test]
	fn test_existing_read() {
		let mut storage = MemoryStorage::new();

		// Add messages
		storage.add(Message::new(Level::Info, "Test message 1"));
		storage.add(Message::new(Level::Info, "Test message 2").with_tags(vec!["tag".to_string()]));

		// Reading via peek doesn't cause data loss
		let data1 = storage.peek();
		assert_eq!(data1.len(), 2);

		// Data is still there
		let data2 = storage.peek();
		assert_eq!(data2.len(), 2);
		assert_eq!(data1.len(), data2.len());
	}

	#[test]
	fn test_messages_storage_get() {
		let mut storage = MemoryStorage::new();

		// Set initial data
		let example_messages = vec!["test", "me"];
		for msg in &example_messages {
			storage.add(Message::new(Level::Info, *msg));
		}

		// Get messages
		let messages = storage.peek();
		assert_eq!(messages.len(), example_messages.len());
		for (i, msg) in messages.iter().enumerate() {
			assert_eq!(msg.text, example_messages[i]);
		}
	}

	#[test]
	fn test_messages_storage_clear() {
		let mut storage = MemoryStorage::new();

		storage.add(Message::new(Level::Info, "Test message 1"));
		storage.add(Message::new(Level::Info, "Test message 2"));
		assert_eq!(storage.peek().len(), 2);

		storage.clear();
		assert_eq!(storage.peek().len(), 0);
	}

	#[test]
	fn test_multiple_adds() {
		let mut storage = MemoryStorage::new();

		let levels = vec![
			(Level::Debug, "Debug message"),
			(Level::Info, "Info message"),
			(Level::Success, "Success message"),
			(Level::Warning, "Warning message"),
			(Level::Error, "Error message"),
		];

		for (level, text) in &levels {
			storage.add(Message::new(*level, *text));
		}

		let messages = storage.peek();
		assert_eq!(messages.len(), levels.len());

		for (i, msg) in messages.iter().enumerate() {
			assert_eq!(msg.level, levels[i].0);
			assert_eq!(msg.text, levels[i].1);
		}
	}
}
