//! Message type definition

use super::levels::Level;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A user-facing message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
	/// Severity level of this message.
	pub level: Level,
	/// The message text content.
	pub text: String,
	/// Additional CSS class tags for rendering.
	pub extra_tags: Vec<String>,
}

impl Message {
	/// Creates a new message with the given level and text
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::new(Level::Info, "Hello world");
	/// assert_eq!(msg.level, Level::Info);
	/// assert_eq!(msg.text, "Hello world");
	/// ```
	pub fn new(level: Level, text: impl Into<String>) -> Self {
		Self {
			level,
			text: text.into(),
			extra_tags: Vec::new(),
		}
	}
	/// Creates a debug-level message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::debug("Debug info");
	/// assert_eq!(msg.level, Level::Debug);
	/// assert_eq!(msg.text, "Debug info");
	/// ```
	pub fn debug(text: impl Into<String>) -> Self {
		Self::new(Level::Debug, text)
	}
	/// Creates an info-level message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::info("Information");
	/// assert_eq!(msg.level, Level::Info);
	/// assert_eq!(msg.text, "Information");
	/// ```
	pub fn info(text: impl Into<String>) -> Self {
		Self::new(Level::Info, text)
	}
	/// Creates a success-level message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::success("Operation completed");
	/// assert_eq!(msg.level, Level::Success);
	/// assert_eq!(msg.text, "Operation completed");
	/// ```
	pub fn success(text: impl Into<String>) -> Self {
		Self::new(Level::Success, text)
	}
	/// Creates a warning-level message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::warning("Be careful");
	/// assert_eq!(msg.level, Level::Warning);
	/// assert_eq!(msg.text, "Be careful");
	/// ```
	pub fn warning(text: impl Into<String>) -> Self {
		Self::new(Level::Warning, text)
	}
	/// Creates an error-level message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::{Message, Level};
	///
	/// let msg = Message::error("Something went wrong");
	/// assert_eq!(msg.level, Level::Error);
	/// assert_eq!(msg.text, "Something went wrong");
	/// ```
	pub fn error(text: impl Into<String>) -> Self {
		Self::new(Level::Error, text)
	}
	/// Sets extra tags for the message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Message;
	///
	/// let msg = Message::info("Test").with_tags(vec!["urgent".to_string(), "user".to_string()]);
	/// assert_eq!(msg.extra_tags, vec!["urgent", "user"]);
	/// ```
	pub fn with_tags(mut self, tags: Vec<String>) -> Self {
		self.extra_tags = tags;
		self
	}
	/// Adds a single tag to the message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Message;
	///
	/// let mut msg = Message::info("Test");
	/// msg.add_tag("important".to_string());
	/// assert!(msg.extra_tags.contains(&"important".to_string()));
	/// ```
	pub fn add_tag(&mut self, tag: String) {
		self.extra_tags.push(tag);
	}
	/// Returns all tags including the level tag and extra tags
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Message;
	///
	/// let msg = Message::info("Test").with_tags(vec!["custom".to_string()]);
	/// let tags = msg.tags();
	/// assert_eq!(tags, vec!["info", "custom"]);
	/// ```
	pub fn tags(&self) -> Vec<String> {
		let mut tags = vec![self.level.as_str().to_string()];
		tags.extend(self.extra_tags.clone());
		tags
	}
}

/// Configuration for message level tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageConfig {
	level_tags: HashMap<i32, String>,
}

impl MessageConfig {
	/// Creates a new MessageConfig with default level tags
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::MessageConfig;
	///
	/// let config = MessageConfig::new();
	/// assert_eq!(config.get_tag(reinhardt_core::messages::Level::Info), Some("info"));
	/// ```
	pub fn new() -> Self {
		let mut level_tags = HashMap::new();
		level_tags.insert(10, "debug".to_string());
		level_tags.insert(20, "info".to_string());
		level_tags.insert(25, "success".to_string());
		level_tags.insert(30, "warning".to_string());
		level_tags.insert(40, "error".to_string());

		Self { level_tags }
	}

	/// Sets a custom tag for a specific level value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::MessageConfig;
	///
	/// let mut config = MessageConfig::new();
	/// config.set_tag(42, "custom".to_string());
	/// assert_eq!(config.get_tag(reinhardt_core::messages::Level::Custom(42)), Some("custom"));
	/// ```
	pub fn set_tag(&mut self, level: i32, tag: String) {
		self.level_tags.insert(level, tag);
	}

	/// Gets the tag for a specific level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::MessageConfig;
	///
	/// let config = MessageConfig::new();
	/// assert_eq!(config.get_tag(reinhardt_core::messages::Level::Info), Some("info"));
	/// assert_eq!(config.get_tag(reinhardt_core::messages::Level::Custom(99)), None);
	/// ```
	pub fn get_tag(&self, level: Level) -> Option<&str> {
		self.level_tags.get(&level.value()).map(|s| s.as_str())
	}

	/// Gets all configured level tags
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::MessageConfig;
	///
	/// let config = MessageConfig::new();
	/// let tags = config.get_all_tags();
	/// assert_eq!(tags.len(), 5);
	/// ```
	pub fn get_all_tags(&self) -> &HashMap<i32, String> {
		&self.level_tags
	}
}

impl Default for MessageConfig {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_messages_creation_unit() {
		let msg = Message::success("Operation completed");
		assert_eq!(msg.level, Level::Success);
		assert_eq!(msg.text, "Operation completed");
	}

	#[test]
	fn test_message_tags() {
		let msg = Message::info("Test").with_tags(vec!["custom".to_string()]);
		let tags = msg.tags();
		assert!(tags.contains(&"info".to_string()));
		assert!(tags.contains(&"custom".to_string()));
	}

	// Tests from Django messages_tests/tests.py - MessageTests
	#[test]
	fn test_eq() {
		let msg_1 = Message::new(Level::Info, "Test message 1");
		let msg_2 = Message::new(Level::Info, "Test message 2");
		let msg_3 = Message::new(Level::Warning, "Test message 1");

		// Clone for comparison
		let msg_1_clone = msg_1.clone();
		assert_eq!(msg_1.text, msg_1_clone.text);
		assert_eq!(msg_1.level, msg_1_clone.level);

		assert_ne!(msg_1.text, msg_2.text);
		assert_ne!(msg_1.level, msg_3.level);
		assert_ne!(msg_2.text, msg_3.text);
	}

	#[test]
	fn test_repr() {
		let tests = vec![
			(Level::Info, "thing", vec![], "thing"),
			(
				Level::Warning,
				"careful",
				vec!["tag1".to_string(), "tag2".to_string()],
				"careful",
			),
			(Level::Error, "oops", vec!["tag".to_string()], "oops"),
		];

		for (level, message, extra_tags, expected_text) in tests {
			let msg = if extra_tags.is_empty() {
				Message::new(level, message)
			} else {
				Message::new(level, message).with_tags(extra_tags)
			};
			assert_eq!(msg.text, expected_text);
			assert_eq!(msg.level, level);
		}
	}

	// Tests from Django messages_tests/base.py - BaseTests
	#[test]
	fn test_add() {
		let msg1 = Message::new(Level::Info, "Test message 1");
		let msg2 = Message::new(Level::Info, "Test message 2").with_tags(vec!["tag".to_string()]);

		assert_eq!(msg1.text, "Test message 1");
		assert_eq!(msg2.text, "Test message 2");
		assert!(msg2.extra_tags.contains(&"tag".to_string()));
	}

	#[test]
	fn test_tags() {
		let msg = Message::new(Level::Info, "A generic info message");
		let tags = msg.tags();
		assert_eq!(tags[0], "info");

		let msg_with_tags = Message::new(Level::Debug, "A debugging message")
			.with_tags(vec!["extra-tag".to_string()]);
		let tags = msg_with_tags.tags();
		assert_eq!(tags[0], "debug");
		assert!(tags.contains(&"extra-tag".to_string()));
	}

	#[test]
	fn test_level_tag() {
		let msg_info = Message::new(Level::Info, "test");
		let msg_debug = Message::new(Level::Debug, "test");
		let msg_warning = Message::new(Level::Warning, "test");
		let msg_error = Message::new(Level::Error, "test");
		let msg_success = Message::new(Level::Success, "test");

		assert_eq!(msg_info.level.as_str(), "info");
		assert_eq!(msg_debug.level.as_str(), "debug");
		assert_eq!(msg_warning.level.as_str(), "warning");
		assert_eq!(msg_error.level.as_str(), "error");
		assert_eq!(msg_success.level.as_str(), "success");
	}

	#[test]
	fn test_extra_tags() {
		// Test with empty tags
		let msg_empty = Message::new(Level::Info, "message");
		assert!(msg_empty.extra_tags.is_empty());

		// Test with tags
		let msg_with_tags = Message::new(Level::Info, "message")
			.with_tags(vec!["some".to_string(), "tags".to_string()]);
		assert_eq!(
			msg_with_tags.extra_tags,
			vec!["some".to_string(), "tags".to_string()]
		);
	}

	#[test]
	fn test_all_level_shortcuts() {
		let debug_msg = Message::debug("Debug message");
		assert_eq!(debug_msg.level, Level::Debug);
		assert_eq!(debug_msg.text, "Debug message");

		let info_msg = Message::info("Info message");
		assert_eq!(info_msg.level, Level::Info);
		assert_eq!(info_msg.text, "Info message");

		let success_msg = Message::success("Success message");
		assert_eq!(success_msg.level, Level::Success);
		assert_eq!(success_msg.text, "Success message");

		let warning_msg = Message::warning("Warning message");
		assert_eq!(warning_msg.level, Level::Warning);
		assert_eq!(warning_msg.text, "Warning message");

		let error_msg = Message::error("Error message");
		assert_eq!(error_msg.level, Level::Error);
		assert_eq!(error_msg.text, "Error message");
	}

	#[test]
	fn test_message_config() {
		let config = MessageConfig::new();
		assert_eq!(config.get_tag(Level::Info), Some("info"));
		assert_eq!(config.get_tag(Level::Debug), Some("debug"));
		assert_eq!(config.get_tag(Level::Custom(99)), None);
	}

	#[test]
	fn test_message_config_custom_tags() {
		let mut config = MessageConfig::new();
		config.set_tag(42, "custom".to_string());
		config.set_tag(50, "urgent".to_string());

		assert_eq!(config.get_tag(Level::Custom(42)), Some("custom"));
		assert_eq!(config.get_tag(Level::Custom(50)), Some("urgent"));
		assert_eq!(config.get_tag(Level::Custom(99)), None);
	}

	#[test]
	fn test_message_config_default() {
		let config = MessageConfig::default();
		assert_eq!(config.get_tag(Level::Info), Some("info"));
		assert_eq!(config.get_all_tags().len(), 5);
	}
}
