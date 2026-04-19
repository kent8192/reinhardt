// URL path utilities for route prefix joining and normalization.
//
// These functions operate on URL path segments (e.g., `/api/v1/users/`),
// **not** filesystem paths. They are always `/`-separated and
// platform-independent.

/// Join a URL prefix and path, collapsing a double slash at the boundary.
///
/// This handles the common case where a prefix ends with `/` and a path
/// starts with `/`, which would otherwise produce `//` in the result.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(join_prefix_path("/api/", "/users"), "/api/users");
/// assert_eq!(join_prefix_path("/api", "/users"), "/api/users");
/// assert_eq!(join_prefix_path("", "/users"), "/users");
/// ```
pub(crate) fn join_prefix_path(prefix: &str, path: &str) -> String {
	if prefix.is_empty() {
		return path.to_string();
	}
	if path.is_empty() {
		return prefix.to_string();
	}
	if prefix.ends_with('/') && path.starts_with('/') {
		format!("{}{}", prefix, &path[1..])
	} else {
		format!("{}{}", prefix, path)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ========================================================================
	// Normal cases
	// ========================================================================

	#[rstest]
	#[case("/api/", "/users", "/api/users")]
	#[case("/api", "/users", "/api/users")]
	#[case("/api/v1/", "/health", "/api/v1/health")]
	#[case("/api/v1", "/health", "/api/v1/health")]
	fn normal_prefix_and_path(#[case] prefix: &str, #[case] path: &str, #[case] expected: &str) {
		assert_eq!(join_prefix_path(prefix, path), expected);
	}

	// ========================================================================
	// Empty inputs (boundary)
	// ========================================================================

	#[rstest]
	fn empty_prefix_returns_path() {
		assert_eq!(join_prefix_path("", "/users"), "/users");
	}

	#[rstest]
	fn empty_path_returns_prefix() {
		assert_eq!(join_prefix_path("/api/", ""), "/api/");
	}

	#[rstest]
	fn both_empty_returns_empty() {
		assert_eq!(join_prefix_path("", ""), "");
	}

	// ========================================================================
	// Slash boundary handling
	// ========================================================================

	#[rstest]
	fn trailing_slash_prefix_and_leading_slash_path() {
		// The core case: collapse the double slash
		assert_eq!(join_prefix_path("/api/", "/users"), "/api/users");
	}

	#[rstest]
	fn no_trailing_slash_prefix_and_leading_slash_path() {
		assert_eq!(join_prefix_path("/api", "/users"), "/api/users");
	}

	#[rstest]
	fn trailing_slash_prefix_and_no_leading_slash_path() {
		assert_eq!(join_prefix_path("/api/", "users"), "/api/users");
	}

	#[rstest]
	fn no_slashes_at_boundary() {
		// Neither has boundary slash — concatenated directly
		assert_eq!(join_prefix_path("/api", "users"), "/apiusers");
	}

	// ========================================================================
	// Slash-only inputs
	// ========================================================================

	#[rstest]
	fn prefix_is_slash_only() {
		assert_eq!(join_prefix_path("/", "/users"), "/users");
	}

	#[rstest]
	fn path_is_slash_only() {
		assert_eq!(join_prefix_path("/api", "/"), "/api/");
	}

	#[rstest]
	fn both_are_slash_only() {
		assert_eq!(join_prefix_path("/", "/"), "/");
	}

	// ========================================================================
	// Trailing slash preservation
	// ========================================================================

	#[rstest]
	fn trailing_slash_in_path_preserved() {
		assert_eq!(join_prefix_path("/api/", "/users/"), "/api/users/");
	}

	#[rstest]
	fn trailing_slash_in_prefix_only() {
		assert_eq!(join_prefix_path("/api/", ""), "/api/");
	}

	// ========================================================================
	// Route patterns with placeholders
	// ========================================================================

	#[rstest]
	#[case("/api/", "/<id>", "/api/<id>")]
	#[case("/users/", "/<user_id>/posts", "/users/<user_id>/posts")]
	#[case("/api/v1/", "/items/{slug}/", "/api/v1/items/{slug}/")]
	fn placeholders_preserved(#[case] prefix: &str, #[case] path: &str, #[case] expected: &str) {
		assert_eq!(join_prefix_path(prefix, path), expected);
	}

	// ========================================================================
	// Multi-level nesting (integration-style)
	// ========================================================================

	#[rstest]
	fn three_level_nesting() {
		let level1 = join_prefix_path("", "/api/");
		let level2 = join_prefix_path(&level1, "/v1/");
		let level3 = join_prefix_path(&level2, "/users");
		assert_eq!(level3, "/api/v1/users");
	}

	#[rstest]
	fn three_level_nesting_no_trailing_slashes() {
		let level1 = join_prefix_path("", "/api");
		let level2 = join_prefix_path(&level1, "/v1");
		let level3 = join_prefix_path(&level2, "/users");
		assert_eq!(level3, "/api/v1/users");
	}

	// ========================================================================
	// Edge cases
	// ========================================================================

	#[rstest]
	fn prefix_without_leading_slash() {
		// Unusual but should not panic
		assert_eq!(join_prefix_path("api/", "/users"), "api/users");
	}

	#[rstest]
	fn path_without_leading_slash_and_prefix_without_trailing() {
		// Direct concatenation — caller's responsibility to ensure correct format
		assert_eq!(join_prefix_path("api", "users"), "apiusers");
	}

	#[rstest]
	fn multiple_internal_slashes_not_collapsed() {
		// Only the boundary slash is collapsed; internal slashes are untouched
		assert_eq!(join_prefix_path("/api//v1/", "/users"), "/api//v1/users");
	}

	#[rstest]
	fn prefix_is_long_path() {
		assert_eq!(join_prefix_path("/a/b/c/d/e/f/", "/g"), "/a/b/c/d/e/f/g");
	}

	#[rstest]
	fn single_char_segments() {
		assert_eq!(join_prefix_path("/a/", "/b"), "/a/b");
	}
}
