//! Message filtering utilities
//!
//! This module provides functions for filtering messages by level.

use crate::messages::levels::Level;
use crate::messages::message::Message;

/// Filter messages by minimum level
///
/// Returns only messages that have a level greater than or equal to the specified minimum level.
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::filter::filter_by_min_level;
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::debug("Debug message"),
///     Message::info("Info message"),
///     Message::warning("Warning message"),
///     Message::error("Error message"),
/// ];
///
/// let filtered = filter_by_min_level(&messages, Level::Info);
/// // Only Info, Warning, and Error messages are included
/// assert_eq!(filtered.len(), 3);
/// ```
pub fn filter_by_min_level(messages: &[Message], min_level: Level) -> Vec<Message> {
	messages
		.iter()
		.filter(|msg| msg.level >= min_level)
		.cloned()
		.collect()
}

/// Filter messages by exact level
///
/// Returns only messages that have the specified level.
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::filter::filter_by_level;
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::debug("Debug message"),
///     Message::info("Info message"),
///     Message::warning("Warning message"),
///     Message::error("Error message"),
/// ];
///
/// let filtered = filter_by_level(&messages, Level::Warning);
/// assert_eq!(filtered.len(), 1);
/// assert_eq!(filtered[0].text, "Warning message");
/// ```
pub fn filter_by_level(messages: &[Message], level: Level) -> Vec<Message> {
	messages
		.iter()
		.filter(|msg| msg.level == level)
		.cloned()
		.collect()
}

/// Filter messages by maximum level
///
/// Returns only messages that have a level less than or equal to the specified maximum level.
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::filter::filter_by_max_level;
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::debug("Debug message"),
///     Message::info("Info message"),
///     Message::warning("Warning message"),
///     Message::error("Error message"),
/// ];
///
/// let filtered = filter_by_max_level(&messages, Level::Info);
/// // Only Debug and Info messages are included
/// assert_eq!(filtered.len(), 2);
/// ```
pub fn filter_by_max_level(messages: &[Message], max_level: Level) -> Vec<Message> {
	messages
		.iter()
		.filter(|msg| msg.level <= max_level)
		.cloned()
		.collect()
}

/// Filter messages by level range
///
/// Returns only messages that have a level within the specified range (inclusive).
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::filter::filter_by_level_range;
/// use reinhardt_core::messages::{Message, Level};
///
/// let messages = vec![
///     Message::debug("Debug message"),
///     Message::info("Info message"),
///     Message::warning("Warning message"),
///     Message::error("Error message"),
/// ];
///
/// let filtered = filter_by_level_range(&messages, Level::Info, Level::Warning);
/// // Only Info and Warning messages are included
/// assert_eq!(filtered.len(), 2);
/// ```
pub fn filter_by_level_range(
	messages: &[Message],
	min_level: Level,
	max_level: Level,
) -> Vec<Message> {
	messages
		.iter()
		.filter(|msg| msg.level >= min_level && msg.level <= max_level)
		.cloned()
		.collect()
}

/// Filter messages by tag
///
/// Returns only messages that contain the specified tag.
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::filter::filter_by_tag;
/// use reinhardt_core::messages::Message;
///
/// let messages = vec![
///     Message::info("Normal message"),
///     Message::info("Important message").with_tags(vec!["important".to_string()]),
///     Message::warning("Urgent message").with_tags(vec!["urgent".to_string(), "important".to_string()]),
/// ];
///
/// let filtered = filter_by_tag(&messages, "important");
/// assert_eq!(filtered.len(), 2);
/// ```
pub fn filter_by_tag(messages: &[Message], tag: &str) -> Vec<Message> {
	messages
		.iter()
		.filter(|msg| msg.tags().contains(&tag.to_string()))
		.cloned()
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_filter_by_min_level() {
		let messages = vec![
			Message::debug("Debug"),
			Message::info("Info"),
			Message::success("Success"),
			Message::warning("Warning"),
			Message::error("Error"),
		];

		let filtered = filter_by_min_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 4);
		assert!(filtered.iter().all(|m| m.level >= Level::Info));

		let filtered = filter_by_min_level(&messages, Level::Warning);
		assert_eq!(filtered.len(), 2);
		assert!(filtered.iter().all(|m| m.level >= Level::Warning));
	}

	#[test]
	fn test_filter_by_level() {
		let messages = vec![
			Message::debug("Debug"),
			Message::info("Info 1"),
			Message::info("Info 2"),
			Message::warning("Warning"),
		];

		let filtered = filter_by_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 2);
		assert!(filtered.iter().all(|m| m.level == Level::Info));
	}

	#[test]
	fn test_filter_by_max_level() {
		let messages = vec![
			Message::debug("Debug"),
			Message::info("Info"),
			Message::success("Success"),
			Message::warning("Warning"),
			Message::error("Error"),
		];

		let filtered = filter_by_max_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 2);
		assert!(filtered.iter().all(|m| m.level <= Level::Info));

		let filtered = filter_by_max_level(&messages, Level::Warning);
		assert_eq!(filtered.len(), 4);
		assert!(filtered.iter().all(|m| m.level <= Level::Warning));
	}

	#[test]
	fn test_filter_by_level_range() {
		let messages = vec![
			Message::debug("Debug"),
			Message::info("Info"),
			Message::success("Success"),
			Message::warning("Warning"),
			Message::error("Error"),
		];

		let filtered = filter_by_level_range(&messages, Level::Info, Level::Warning);
		assert_eq!(filtered.len(), 3);
		assert!(
			filtered
				.iter()
				.all(|m| m.level >= Level::Info && m.level <= Level::Warning)
		);
	}

	#[test]
	fn test_filter_by_tag() {
		let messages = vec![
			Message::info("Normal"),
			Message::info("Important").with_tags(vec!["important".to_string()]),
			Message::warning("Urgent")
				.with_tags(vec!["urgent".to_string(), "important".to_string()]),
			Message::error("Critical").with_tags(vec!["critical".to_string()]),
		];

		let filtered = filter_by_tag(&messages, "important");
		assert_eq!(filtered.len(), 2);

		let filtered = filter_by_tag(&messages, "urgent");
		assert_eq!(filtered.len(), 1);

		let filtered = filter_by_tag(&messages, "nonexistent");
		assert_eq!(filtered.len(), 0);
	}

	#[test]
	fn test_filter_empty_messages() {
		let messages: Vec<Message> = vec![];

		let filtered = filter_by_min_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_max_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_level_range(&messages, Level::Info, Level::Warning);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_tag(&messages, "any_tag");
		assert_eq!(filtered.len(), 0);
	}

	#[test]
	fn test_filter_all_messages_excluded() {
		let messages = vec![
			Message::debug("Debug 1"),
			Message::debug("Debug 2"),
			Message::debug("Debug 3"),
		];

		let filtered = filter_by_min_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 0);

		let filtered = filter_by_max_level(&messages, Level::Custom(5));
		assert_eq!(filtered.len(), 0);
	}

	#[test]
	fn test_filter_all_messages_included() {
		let messages = vec![
			Message::info("Info 1"),
			Message::info("Info 2"),
			Message::info("Info 3"),
		];

		let filtered = filter_by_min_level(&messages, Level::Debug);
		assert_eq!(filtered.len(), 3);

		let filtered = filter_by_level(&messages, Level::Info);
		assert_eq!(filtered.len(), 3);

		let filtered = filter_by_max_level(&messages, Level::Error);
		assert_eq!(filtered.len(), 3);

		let filtered = filter_by_level_range(&messages, Level::Debug, Level::Error);
		assert_eq!(filtered.len(), 3);
	}
}
