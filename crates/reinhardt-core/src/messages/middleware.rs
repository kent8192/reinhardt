//! Messages container for request processing
//!
//! This module provides the `MessagesContainer` struct for storing and managing
//! messages during request processing.
//!
//! ## Note
//!
//! HTTP middleware integration (`MessagesMiddleware`) has been moved to
//! `reinhardt-http` crate to prevent circular dependencies.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::messages::middleware::MessagesContainer;
//! use reinhardt_core::messages::Message;
//!
//! let container = MessagesContainer::new(vec![]);
//! container.add(Message::info("Hello"));
//! container.add(Message::success("Operation completed"));
//!
//! let messages = container.get_messages();
//! assert_eq!(messages.len(), 2);
//! ```

use super::message::Message;
use std::sync::{Arc, Mutex};

/// Container for messages stored in request extensions
///
/// This struct is used to store messages in the request extensions
/// during request processing.
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::messages::middleware::MessagesContainer;
/// use reinhardt_core::messages::Message;
///
/// let container = MessagesContainer::new(vec![]);
/// container.add(Message::info("Hello"));
/// ```
#[derive(Clone)]
pub struct MessagesContainer {
	messages: Arc<Mutex<Vec<Message>>>,
}

impl MessagesContainer {
	/// Create a new messages container
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::messages::middleware::MessagesContainer;
	/// use reinhardt_core::messages::Message;
	///
	/// let messages = vec![Message::info("Initial message")];
	/// let container = MessagesContainer::new(messages);
	/// ```
	pub fn new(messages: Vec<Message>) -> Self {
		Self {
			messages: Arc::new(Mutex::new(messages)),
		}
	}

	/// Add a message to the container
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::messages::middleware::MessagesContainer;
	/// use reinhardt_core::messages::Message;
	///
	/// let container = MessagesContainer::new(vec![]);
	/// container.add(Message::success("Operation completed"));
	/// ```
	pub fn add(&self, message: Message) {
		let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
		messages.push(message);
	}

	/// Get all messages from the container
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::messages::middleware::MessagesContainer;
	/// use reinhardt_core::messages::Message;
	///
	/// let container = MessagesContainer::new(vec![Message::info("Test")]);
	/// let messages = container.get_messages();
	/// assert_eq!(messages.len(), 1);
	/// ```
	pub fn get_messages(&self) -> Vec<Message> {
		let messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
		messages.clone()
	}

	/// Clear all messages from the container
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::messages::middleware::MessagesContainer;
	/// use reinhardt_core::messages::Message;
	///
	/// let container = MessagesContainer::new(vec![Message::info("Test")]);
	/// container.clear();
	/// assert_eq!(container.get_messages().len(), 0);
	/// ```
	pub fn clear(&self) {
		let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
		messages.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::Level;

	#[test]
	fn test_messages_container_new() {
		let messages = vec![Message::info("Test message")];
		let container = MessagesContainer::new(messages);
		let loaded_messages = container.get_messages();
		assert_eq!(loaded_messages.len(), 1);
		assert_eq!(loaded_messages[0].text, "Test message");
	}

	#[test]
	fn test_messages_container_add() {
		let container = MessagesContainer::new(vec![]);
		container.add(Message::success("Success"));
		container.add(Message::error("Error"));

		let messages = container.get_messages();
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].text, "Success");
		assert_eq!(messages[1].text, "Error");
	}

	#[test]
	fn test_messages_container_clear() {
		let container = MessagesContainer::new(vec![Message::info("Test")]);
		assert_eq!(container.get_messages().len(), 1);

		container.clear();
		assert_eq!(container.get_messages().len(), 0);
	}

	#[test]
	fn test_messages_container_with_different_levels() {
		let container = MessagesContainer::new(vec![]);
		container.add(Message::debug("Debug message"));
		container.add(Message::info("Info message"));
		container.add(Message::success("Success message"));
		container.add(Message::warning("Warning message"));
		container.add(Message::error("Error message"));

		let messages = container.get_messages();
		assert_eq!(messages.len(), 5);
		assert_eq!(messages[0].level, Level::Debug);
		assert_eq!(messages[1].level, Level::Info);
		assert_eq!(messages[2].level, Level::Success);
		assert_eq!(messages[3].level, Level::Warning);
		assert_eq!(messages[4].level, Level::Error);
	}
}
