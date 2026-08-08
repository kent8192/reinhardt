use reinhardt_core::messages::{Level, Message};
use reinhardt_test::messages::MessagesTestMixin;
use rstest::rstest;

#[rstest]
fn message_mixin_delegates_success_and_rejection_paths() {
	// Arrange
	let mut saved = Message::new(Level::Success, "Profile saved".to_string());
	saved.extra_tags = vec!["profile".to_string(), "toast".to_string()];
	let messages = vec![saved];
	let expected_messages = vec![(Level::Success, "Profile saved".to_string())];
	let expected_tags = vec![(
		Level::Success,
		"Profile saved".to_string(),
		vec!["profile".to_string(), "toast".to_string()],
	)];
	let ordered_messages = vec![
		Message::new(Level::Warning, "Second message".to_string()),
		Message::new(Level::Info, "First message".to_string()),
	];
	let ordered_expected = vec![
		(Level::Info, "First message".to_string()),
		(Level::Warning, "Second message".to_string()),
	];
	let preserving = MessagesTestMixin::new();
	let filtering = MessagesTestMixin::with_settings(true);
	let stack_trace = "at app::handler\nat suite::assert_case\nat testing::helper\nat main";

	// Act
	let exists = preserving.assert_message_exists(&messages, Level::Success, "Profile saved");
	let ordered = preserving.assert_messages(&messages, &expected_messages, true);
	let tagged = preserving.assert_messages_with_tags(&messages, &expected_tags);
	let wrong_level = preserving
		.assert_message_exists(&messages, Level::Error, "Profile saved")
		.unwrap_err();
	let wrong_text = preserving
		.assert_message_exists(&messages, Level::Success, "Profile deleted")
		.unwrap_err();
	let wrong_order = preserving
		.assert_messages(&ordered_messages, &ordered_expected, true)
		.unwrap_err();
	let wrong_tags = preserving
		.assert_messages_with_tags(
			&messages,
			&[(
				Level::Success,
				"Profile saved".to_string(),
				vec!["other".to_string()],
			)],
		)
		.unwrap_err();
	let preserved = preserving.filter_stack_trace(stack_trace);
	let filtered = filtering.filter_stack_trace(stack_trace);

	// Assert
	exists.unwrap();
	ordered.unwrap();
	tagged.unwrap();
	assert_eq!(
		wrong_level.to_string(),
		"Message not found: Message with level Error and text 'Profile saved' not found"
	);
	assert_eq!(
		wrong_text.to_string(),
		"Message not found: Message with level Success and text 'Profile deleted' not found"
	);
	assert_eq!(
		wrong_order.to_string(),
		"Order mismatch: expected [\"First message\", \"Second message\"], got [\"Second message\", \"First message\"]"
	);
	assert_eq!(
		wrong_tags.to_string(),
		"Message not found: Message with level Success, text 'Profile saved', and tags [\"other\"] not found"
	);
	assert_eq!(preserved, stack_trace);
	assert_eq!(filtered, "at app::handler\nat main");
}
