//! Immutable file detection tests

use reinhardt_whitenoise::immutable::{is_immutable, is_immutable_with_test};
use rstest::rstest;

#[rstest]
#[case("app.abc123def456.js", true)]
#[case("style.1234567890ab.css", true)]
#[case("image.000000000000.png", true)]
#[case("file.ffffffffffff.svg", true)]
fn test_is_immutable_with_hash(#[case] path: &str, #[case] expected: bool) {
	assert_eq!(is_immutable(path), expected);
}

#[rstest]
#[case("app.js", false)]
#[case("style.css", false)]
#[case("image.png", false)]
#[case("app.min.js", false)]
#[case("app.abc.js", false)]
fn test_is_immutable_without_hash(#[case] path: &str, #[case] expected: bool) {
	assert_eq!(is_immutable(path), expected);
}

#[rstest]
#[case("app.abc.js", false)]
#[case("app.abc123.js", false)] // Only 6 hex chars
#[case("app.12345678901.js", false)] // Only 11 hex chars
#[case("app.1234567890123.js", false)] // 13 hex chars
fn test_is_immutable_short_or_long_hash(#[case] path: &str, #[case] expected: bool) {
	assert_eq!(is_immutable(path), expected);
}

#[rstest]
fn test_is_immutable_custom_function() {
	let is_min = |path: &str| path.contains(".min.");

	assert!(is_immutable_with_test("app.min.js", Some(is_min)));
	assert!(is_immutable_with_test("style.min.css", Some(is_min)));
	assert!(!is_immutable_with_test("app.js", Some(is_min)));
	assert!(!is_immutable_with_test("app.abc123def456.js", Some(is_min)));
}

#[rstest]
fn test_is_immutable_combined() {
	let combined = |path: &str| {
		let default_immutable = is_immutable(path);
		let is_min = path.contains(".min.");
		default_immutable || is_min
	};

	assert!(is_immutable_with_test(
		"app.abc123def456.js",
		Some(combined)
	));
	assert!(is_immutable_with_test("app.min.js", Some(combined)));
	assert!(!is_immutable_with_test("app.js", Some(combined)));
}

#[rstest]
fn test_is_immutable_none_test() {
	// When None is passed, should use default pattern
	assert!(is_immutable_with_test(
		"app.abc123def456.js",
		None::<fn(&str) -> bool>
	));
	assert!(!is_immutable_with_test("app.js", None::<fn(&str) -> bool>));
}
