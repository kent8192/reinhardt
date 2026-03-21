//! Message assertion utilities for testing
//!
//! This module provides assertion functions for testing message functionality,
//! similar to Django's message testing utilities.

use reinhardt_core::messages::{Level, Message};

/// Error type for message assertion failures
#[derive(Debug, thiserror::Error)]
pub enum MessageAssertionError {
	/// The number of messages did not match the expected count.
	#[error("Message count mismatch: expected {expected}, got {actual}")]
	CountMismatch {
		/// Expected message count.
		expected: usize,
		/// Actual message count.
		actual: usize,
	},

	/// A specific message was not found in the collection.
	#[error("Message not found: {message}")]
	MessageNotFound {
		/// The message text that was not found.
		message: String,
	},

	/// The message level did not match the expected level.
	#[error("Message level mismatch: expected {expected:?}, got {actual:?}")]
	LevelMismatch {
		/// Expected message level.
		expected: Level,
		/// Actual message level.
		actual: Level,
	},

	/// The message tags did not match the expected tags.
	#[error("Message tags mismatch: expected {expected:?}, got {actual:?}")]
	TagsMismatch {
		/// Expected tag list.
		expected: Vec<String>,
		/// Actual tag list.
		actual: Vec<String>,
	},

	/// The message order did not match the expected order.
	#[error("Order mismatch: expected {expected:?}, got {actual:?}")]
	OrderMismatch {
		/// Expected message order.
		expected: Vec<String>,
		/// Actual message order.
		actual: Vec<String>,
	},
}

/// Result type for message assertions
pub type MessageAssertionResult<T> = Result<T, MessageAssertionError>;

/// Assert that the number of messages matches the expected count
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::assert_message_count;
/// use reinhardt_core::messages::Message;
///
/// let messages = vec![
///     Message::new(reinhardt_core::messages::Level::Info, "Test message".to_string()),
/// ];
/// assert_message_count(&messages, 1).unwrap();
/// ```
pub fn assert_message_count(
	messages: &[Message],
	expected_count: usize,
) -> MessageAssertionResult<()> {
	let actual_count = messages.len();
	if actual_count != expected_count {
		return Err(MessageAssertionError::CountMismatch {
			expected: expected_count,
			actual: actual_count,
		});
	}
	Ok(())
}

/// Assert that a specific message exists in the collection
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::{assert_message_exists, assert_message_level};
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::new(Level::Info, "Test message".to_string()),
/// ];
/// assert_message_exists(&messages, Level::Info, "Test message").unwrap();
/// ```
pub fn assert_message_exists(
	messages: &[Message],
	level: Level,
	text: &str,
) -> MessageAssertionResult<()> {
	let found = messages
		.iter()
		.any(|msg| msg.level == level && msg.text == text);

	if !found {
		return Err(MessageAssertionError::MessageNotFound {
			message: format!(
				"Message with level {:?} and text '{}' not found",
				level, text
			),
		});
	}
	Ok(())
}

/// Assert that a message has the expected level
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::assert_message_level;
/// use reinhardt_core::messages::{Message, Level};
///
/// let message = Message::new(Level::Info, "Test message".to_string());
/// assert_message_level(&message, Level::Info).unwrap();
/// ```
pub fn assert_message_level(
	message: &Message,
	expected_level: Level,
) -> MessageAssertionResult<()> {
	if message.level != expected_level {
		return Err(MessageAssertionError::LevelMismatch {
			expected: expected_level,
			actual: message.level,
		});
	}
	Ok(())
}

/// Assert that a message has the expected tags
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::assert_message_tags;
/// use reinhardt_core::messages::{Message, Level};
///
/// let mut message = Message::new(Level::Info, "Test message".to_string());
/// message.extra_tags = vec!["tag1".to_string(), "tag2".to_string()];
/// assert_message_tags(&message, vec!["tag1".to_string(), "tag2".to_string()]).unwrap();
/// ```
pub fn assert_message_tags(
	message: &Message,
	expected_tags: Vec<String>,
) -> MessageAssertionResult<()> {
	if message.extra_tags != expected_tags {
		return Err(MessageAssertionError::TagsMismatch {
			expected: expected_tags,
			actual: message.extra_tags.clone(),
		});
	}
	Ok(())
}

/// Assert that messages match the expected list (order-sensitive)
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::assert_messages;
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::new(Level::Info, "First message".to_string()),
///     Message::new(Level::Warning, "Second message".to_string()),
/// ];
/// let expected = vec![
///     (Level::Info, "First message".to_string()),
///     (Level::Warning, "Second message".to_string()),
/// ];
/// assert_messages(&messages, &expected, true).unwrap();
/// ```
pub fn assert_messages(
	messages: &[Message],
	expected: &[(Level, String)],
	ordered: bool,
) -> MessageAssertionResult<()> {
	// First check count
	assert_message_count(messages, expected.len())?;

	if ordered {
		// Check order-sensitive comparison
		for (i, (expected_level, expected_text)) in expected.iter().enumerate() {
			if i >= messages.len() {
				return Err(MessageAssertionError::MessageNotFound {
					message: format!("Expected message at index {} not found", i),
				});
			}

			let actual_message = &messages[i];
			if actual_message.level != *expected_level || actual_message.text != *expected_text {
				return Err(MessageAssertionError::OrderMismatch {
					expected: expected.iter().map(|(_, text)| text.clone()).collect(),
					actual: messages.iter().map(|msg| msg.text.clone()).collect(),
				});
			}
		}
	} else {
		// Check order-insensitive comparison
		for (expected_level, expected_text) in expected {
			assert_message_exists(messages, *expected_level, expected_text)?;
		}
	}

	Ok(())
}

/// Test mixin for message testing utilities
///
/// This struct provides helper methods for testing message functionality,
/// similar to Django's MessagesTestMixin.
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::messages::MessagesTestMixin;
/// use reinhardt_core::messages::{Message, Level};
///
/// let mut mixin = MessagesTestMixin::new();
/// let messages = vec![
///     Message::new(Level::Info, "Test message".to_string()),
/// ];
/// mixin.assert_message_count(&messages, 1).unwrap();
/// ```
#[derive(Debug, Default)]
pub struct MessagesTestMixin {
	/// Whether to ignore method frames in stack traces
	pub ignore_method_frames: bool,
}

impl MessagesTestMixin {
	/// Create a new MessagesTestMixin
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a new MessagesTestMixin with custom settings
	pub fn with_settings(ignore_method_frames: bool) -> Self {
		Self {
			ignore_method_frames,
		}
	}

	/// Assert message count (delegates to assert_message_count)
	pub fn assert_message_count(
		&self,
		messages: &[Message],
		expected_count: usize,
	) -> MessageAssertionResult<()> {
		assert_message_count(messages, expected_count)
	}

	/// Assert message exists (delegates to assert_message_exists)
	pub fn assert_message_exists(
		&self,
		messages: &[Message],
		level: Level,
		text: &str,
	) -> MessageAssertionResult<()> {
		assert_message_exists(messages, level, text)
	}

	/// Assert messages match expected list (delegates to assert_messages)
	pub fn assert_messages(
		&self,
		messages: &[Message],
		expected: &[(Level, String)],
		ordered: bool,
	) -> MessageAssertionResult<()> {
		assert_messages(messages, expected, ordered)
	}

	/// Assert messages with tags
	///
	/// This method checks that messages exist with the specified level, text, and tags.
	pub fn assert_messages_with_tags(
		&self,
		messages: &[Message],
		expected: &[(Level, String, Vec<String>)],
	) -> MessageAssertionResult<()> {
		assert_message_count(messages, expected.len())?;

		for (expected_level, expected_text, expected_tags) in expected {
			let found = messages.iter().find(|msg| {
				msg.level == *expected_level
					&& msg.text == *expected_text
					&& msg.extra_tags == *expected_tags
			});

			if found.is_none() {
				return Err(MessageAssertionError::MessageNotFound {
					message: format!(
						"Message with level {:?}, text '{}', and tags {:?} not found",
						expected_level, expected_text, expected_tags
					),
				});
			}
		}

		Ok(())
	}

	/// Filter stack trace to remove method frames (for unittest compatibility)
	///
	/// This method filters out frames that are part of the testing framework
	/// to make stack traces more readable, similar to Django's unittest behavior.
	pub fn filter_stack_trace(&self, stack_trace: &str) -> String {
		if !self.ignore_method_frames {
			return stack_trace.to_string();
		}

		// Simple filtering - remove lines that contain common test framework patterns
		let lines: Vec<&str> = stack_trace.lines().collect();
		let filtered_lines: Vec<&str> = lines
			.into_iter()
			.filter(|line| {
				!line.contains("::assert_")
					&& !line.contains("::test_")
					&& !line.contains("unittest::")
					&& !line.contains("testing::")
			})
			.collect();

		filtered_lines.join("\n")
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::messages::{Level, Message};

	#[test]
	fn test_assert_message_count_success() {
		let messages = vec![Message::new(Level::Info, "Test message".to_string())];
		assert!(assert_message_count(&messages, 1).is_ok());
	}

	#[test]
	fn test_assert_message_count_failure() {
		let messages = vec![Message::new(Level::Info, "Test message".to_string())];
		assert!(assert_message_count(&messages, 2).is_err());
	}

	#[test]
	fn test_assert_message_exists_success() {
		let messages = vec![Message::new(Level::Info, "Test message".to_string())];
		assert!(assert_message_exists(&messages, Level::Info, "Test message").is_ok());
	}

	#[test]
	fn test_assert_message_exists_failure() {
		let messages = vec![Message::new(Level::Info, "Test message".to_string())];
		assert!(assert_message_exists(&messages, Level::Warning, "Test message").is_err());
	}

	#[test]
	fn test_assert_message_level_success() {
		let message = Message::new(Level::Info, "Test message".to_string());
		assert!(assert_message_level(&message, Level::Info).is_ok());
	}

	#[test]
	fn test_assert_message_level_failure() {
		let message = Message::new(Level::Info, "Test message".to_string());
		assert!(assert_message_level(&message, Level::Warning).is_err());
	}

	#[test]
	fn test_assert_message_tags_success() {
		let mut message = Message::new(Level::Info, "Test message".to_string());
		message.extra_tags = vec!["tag1".to_string(), "tag2".to_string()];
		assert!(
			assert_message_tags(&message, vec!["tag1".to_string(), "tag2".to_string()]).is_ok()
		);
	}

	#[test]
	fn test_assert_message_tags_failure() {
		let mut message = Message::new(Level::Info, "Test message".to_string());
		message.extra_tags = vec!["tag1".to_string()];
		assert!(assert_message_tags(&message, vec!["tag2".to_string()]).is_err());
	}

	#[test]
	fn test_assert_messages_ordered_success() {
		let messages = vec![
			Message::new(Level::Info, "First message".to_string()),
			Message::new(Level::Warning, "Second message".to_string()),
		];
		let expected = vec![
			(Level::Info, "First message".to_string()),
			(Level::Warning, "Second message".to_string()),
		];
		assert!(assert_messages(&messages, &expected, true).is_ok());
	}

	#[test]
	fn test_assert_messages_unordered_success() {
		let messages = vec![
			Message::new(Level::Warning, "Second message".to_string()),
			Message::new(Level::Info, "First message".to_string()),
		];
		let expected = vec![
			(Level::Info, "First message".to_string()),
			(Level::Warning, "Second message".to_string()),
		];
		assert!(assert_messages(&messages, &expected, false).is_ok());
	}

	#[test]
	fn test_messages_test_mixin() {
		let mixin = MessagesTestMixin::new();
		let messages = vec![Message::new(Level::Info, "Test message".to_string())];
		assert!(mixin.assert_message_count(&messages, 1).is_ok());
	}

	#[test]
	fn test_filter_stack_trace() {
		let mixin = MessagesTestMixin::with_settings(true);
		let stack_trace = "at testing::assert_message_count\nat unittest::test_case\nat main";
		let filtered = mixin.filter_stack_trace(stack_trace);
		assert!(!filtered.contains("testing::"));
		assert!(!filtered.contains("unittest::"));
	}
}
