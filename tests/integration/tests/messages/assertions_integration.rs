//! Integration tests for message assertion helpers
//!
//! These tests require reinhardt-test utilities

use reinhardt_core::messages::{Level, Message};
use reinhardt_test::messages::{
	MessagesTestMixin, assert_message_count, assert_message_exists, assert_message_level,
	assert_message_tags, assert_messages,
};

// These tests are based on Django's messages_tests/tests.py::AssertMessagesTest

#[cfg(test)]
mod tests {
	use super::*;

	/// Test basic message assertion functionality
	/// Original: django/tests/messages_tests/tests.py::AssertMessagesTest::test_assertion
	#[test]
	fn test_assertion() {
		// Create messages at all levels
		let messages = vec![
			Message::debug("Debug message"),
			Message::info("Info message"),
			Message::success("Success message"),
			Message::warning("Warning message"),
			Message::error("Error message"),
		];

		let expected = vec![
			(Level::Debug, "Debug message".to_string()),
			(Level::Info, "Info message".to_string()),
			(Level::Success, "Success message".to_string()),
			(Level::Warning, "Warning message".to_string()),
			(Level::Error, "Error message".to_string()),
		];

		// Test ordered assertion
		assert_messages(&messages, &expected, true).unwrap();

		// Test unordered assertion (should also pass)
		let mut unordered_expected = expected.clone();
		unordered_expected.reverse();
		assert_messages(&messages, &unordered_expected, false).unwrap();

		// Test individual message assertions
		assert_message_count(&messages, 5).unwrap();
		assert_message_exists(&messages, Level::Info, "Info message").unwrap();
		assert_message_level(&messages[1], Level::Info).unwrap();
	}

	/// Test message assertion with extra_tags
	/// Original: django/tests/messages_tests/tests.py::AssertMessagesTest::test_with_tags
	#[test]
	fn test_with_tags() {
		// Create messages with extra_tags
		let messages = vec![
			Message::info("Info with tags")
				.with_tags(vec!["urgent".to_string(), "user".to_string()]),
			Message::warning("Warning with tags").with_tags(vec!["system".to_string()]),
			Message::error("Error with tags")
				.with_tags(vec!["critical".to_string(), "admin".to_string()]),
		];

		let expected = vec![
			(
				Level::Info,
				"Info with tags".to_string(),
				vec!["urgent".to_string(), "user".to_string()],
			),
			(
				Level::Warning,
				"Warning with tags".to_string(),
				vec!["system".to_string()],
			),
			(
				Level::Error,
				"Error with tags".to_string(),
				vec!["critical".to_string(), "admin".to_string()],
			),
		];

		// Test with MessagesTestMixin
		let mixin = MessagesTestMixin::new();
		mixin
			.assert_messages_with_tags(&messages, &expected)
			.unwrap();

		// Test individual tag assertions
		assert_message_tags(&messages[0], vec!["urgent".to_string(), "user".to_string()]).unwrap();
		assert_message_tags(&messages[1], vec!["system".to_string()]).unwrap();
		assert_message_tags(
			&messages[2],
			vec!["critical".to_string(), "admin".to_string()],
		)
		.unwrap();
	}

	/// Test that message assertion can check order (or ignore order)
	/// Original: django/tests/messages_tests/tests.py::AssertMessagesTest::test_ordered
	#[test]
	fn test_ordered() {
		let messages = vec![
			Message::info("First message"),
			Message::warning("Second message"),
			Message::error("Third message"),
		];

		let expected_ordered = vec![
			(Level::Info, "First message".to_string()),
			(Level::Warning, "Second message".to_string()),
			(Level::Error, "Third message".to_string()),
		];

		let expected_unordered = vec![
			(Level::Error, "Third message".to_string()),
			(Level::Info, "First message".to_string()),
			(Level::Warning, "Second message".to_string()),
		];

		// Test ordered assertion - should pass with correct order
		assert_messages(&messages, &expected_ordered, true).unwrap();

		// Test ordered assertion - should fail with wrong order
		let result = assert_messages(&messages, &expected_unordered, true);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("Order mismatch"));

		// Test unordered assertion - should pass with any order
		assert_messages(&messages, &expected_unordered, false).unwrap();
		assert_messages(&messages, &expected_ordered, false).unwrap();
	}

	/// Test that assertion fails with helpful error when message count differs
	/// Original: django/tests/messages_tests/tests.py::AssertMessagesTest::test_mismatching_length
	#[test]
	fn test_mismatching_length() {
		let messages = vec![Message::info("Only message")];

		let expected_empty = vec![];

		// Test count mismatch
		let result = assert_message_count(&messages, 0);
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.to_string().contains("Message count mismatch"));
		assert!(error.to_string().contains("expected 0, got 1"));

		// Test messages mismatch with different count
		let result = assert_messages(&messages, &expected_empty, false);
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.to_string().contains("Message count mismatch"));

		// Test with more expected than actual
		let expected_many = vec![
			(Level::Info, "First".to_string()),
			(Level::Warning, "Second".to_string()),
			(Level::Error, "Third".to_string()),
		];

		let result = assert_messages(&messages, &expected_many, false);
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.to_string().contains("Message count mismatch"));
		assert!(error.to_string().contains("expected 3, got 1"));
	}

	/// Test that assertion helper methods are hidden from test failure stack traces
	/// Original: django/tests/messages_tests/tests.py::AssertMessagesTest::test_method_frames_ignored_by_unittest
	#[test]
	fn test_method_frames_ignored_by_unittest() {
		let mixin = MessagesTestMixin::with_settings(true);

		// Test stack trace filtering
		let stack_trace =
			"at testing::assert_message_count\nat unittest::test_case\nat main::user_function";
		let filtered = mixin.filter_stack_trace(stack_trace);

		// Should remove testing framework frames
		assert!(!filtered.contains("testing::"));
		assert!(!filtered.contains("unittest::"));
		// The filtered result should contain the main function name
		assert!(filtered.contains("main::user_function"));

		// Test with ignore_method_frames disabled
		let mixin_no_filter = MessagesTestMixin::with_settings(false);
		let unfiltered = mixin_no_filter.filter_stack_trace(stack_trace);
		assert_eq!(unfiltered, stack_trace);
	}

	/// Test MessagesTestMixin functionality
	#[test]
	fn test_messages_test_mixin() {
		let mixin = MessagesTestMixin::new();
		let messages = vec![
			Message::info("Test message"),
			Message::warning("Another message"),
		];

		// Test basic functionality
		mixin.assert_message_count(&messages, 2).unwrap();
		mixin
			.assert_message_exists(&messages, Level::Info, "Test message")
			.unwrap();

		let expected = vec![
			(Level::Info, "Test message".to_string()),
			(Level::Warning, "Another message".to_string()),
		];

		mixin.assert_messages(&messages, &expected, true).unwrap();
		mixin.assert_messages(&messages, &expected, false).unwrap();
	}
}
