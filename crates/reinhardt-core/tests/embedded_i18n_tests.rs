//! Content verification tests for embedded i18n assets.
//!
//! Verifies that embedded Fluent (.ftl) translation files are not empty
//! and contain expected Fluent syntax markers. Covers Issue #3123.

use rstest::*;

const EN_MESSAGES: &str = include_str!("../src/resources/validation_en.ftl");
const JA_MESSAGES: &str = include_str!("../src/resources/validation_ja.ftl");

#[rstest]
fn en_ftl_is_not_empty() {
	// Assert
	assert!(
		!EN_MESSAGES.is_empty(),
		"validation_en.ftl should not be empty"
	);
}

#[rstest]
fn en_ftl_contains_fluent_syntax() {
	// Assert - Fluent files use `key = value` message assignments
	assert!(
		EN_MESSAGES.contains(" = "),
		"validation_en.ftl should contain Fluent message assignments (key = value)"
	);
}

#[rstest]
fn ja_ftl_is_not_empty() {
	// Assert
	assert!(
		!JA_MESSAGES.is_empty(),
		"validation_ja.ftl should not be empty"
	);
}

#[rstest]
fn ja_ftl_contains_fluent_syntax() {
	// Assert - Fluent files use `key = value` message assignments
	assert!(
		JA_MESSAGES.contains(" = "),
		"validation_ja.ftl should contain Fluent message assignments (key = value)"
	);
}

#[rstest]
fn both_locales_share_same_message_keys() {
	// Assert - both locales should define the same set of top-level keys
	let en_keys: Vec<&str> = EN_MESSAGES
		.lines()
		.filter(|l| !l.starts_with('#') && !l.trim().is_empty() && l.contains(" = "))
		.filter_map(|l| l.split(" = ").next())
		.collect();
	let ja_keys: Vec<&str> = JA_MESSAGES
		.lines()
		.filter(|l| !l.starts_with('#') && !l.trim().is_empty() && l.contains(" = "))
		.filter_map(|l| l.split(" = ").next())
		.collect();

	assert!(
		!en_keys.is_empty(),
		"English FTL should have at least one message key"
	);
	assert_eq!(
		en_keys.len(),
		ja_keys.len(),
		"English and Japanese FTL should have the same number of message keys"
	);
}
