//! Immutable file detection
//!
//! This module provides functionality to detect if a static file is immutable
//! based on its filename pattern. Immutable files can be cached forever.

use regex::Regex;
use std::sync::OnceLock;

/// Checks if a file path indicates an immutable file
///
/// By default, matches files with 12 hexadecimal characters before the extension.
/// Pattern: `^.+\.[0-9a-f]{12}\..+$`
///
/// # Examples
///
/// ```rust
/// use reinhardt_whitenoise::immutable::is_immutable;
///
/// assert!(is_immutable("app.abc123def456.js"));
/// assert!(is_immutable("style.1234567890ab.css"));
/// assert!(!is_immutable("app.js"));
/// assert!(!is_immutable("style.css"));
/// ```
pub fn is_immutable(path: &str) -> bool {
	is_immutable_with_test(path, None::<fn(&str) -> bool>)
}

/// Checks if a file is immutable using an optional custom test function
///
/// # Arguments
///
/// * `path` - The file path to test
/// * `custom_test` - Optional custom test function
///
/// # Examples
///
/// ```rust
/// use reinhardt_whitenoise::immutable::is_immutable_with_test;
///
/// // With custom test
/// let is_min = |path: &str| path.contains(".min.");
/// assert!(is_immutable_with_test("app.min.js", Some(is_min)));
///
/// // With default test
/// assert!(is_immutable_with_test("app.abc123def456.js", None::<fn(&str) -> bool>));
/// ```
pub fn is_immutable_with_test<F>(path: &str, custom_test: Option<F>) -> bool
where
	F: Fn(&str) -> bool,
{
	if let Some(test) = custom_test {
		return test(path);
	}

	// Default: Match 12 hex digits before extension
	// Pattern: ^.+\.[0-9a-f]{12}\..+$
	static IMMUTABLE_REGEX: OnceLock<Regex> = OnceLock::new();
	let regex = IMMUTABLE_REGEX.get_or_init(|| Regex::new(r"^.+\.[0-9a-f]{12}\..+$").unwrap());

	regex.is_match(path)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("app.abc123def456.js", true)]
	#[case("style.1234567890ab.css", true)]
	#[case("app.js", false)]
	#[case("style.css", false)]
	#[case("app.abc.js", false)]
	#[case("app.abc123.js", false)] // Only 6 hex chars
	fn test_is_immutable_default(#[case] path: &str, #[case] expected: bool) {
		assert_eq!(is_immutable(path), expected);
	}

	#[rstest]
	fn test_is_immutable_with_custom_test() {
		let is_min = |path: &str| path.contains(".min.");

		assert!(is_immutable_with_test("app.min.js", Some(is_min)));
		assert!(is_immutable_with_test("style.min.css", Some(is_min)));
		assert!(!is_immutable_with_test("app.js", Some(is_min)));
	}

	#[rstest]
	fn test_is_immutable_with_combined_test() {
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
}
