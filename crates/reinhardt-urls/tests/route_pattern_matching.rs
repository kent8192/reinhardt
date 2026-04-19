//! Integration tests for URL route pattern matching.
//!
//! Tests cover PathPattern creation, type_spec_to_regex behavior (via PathPattern),
//! validate_path_param security checks, parameter extraction, and URL reversal.

use reinhardt_urls::routers::{MatchingMode, PathMatcher, PathPattern};
use rstest::rstest;
use std::collections::HashMap;

// ===================================================================
// PathPattern::new - basic creation tests
// ===================================================================

#[rstest]
fn path_pattern_simple_static() {
	// Arrange
	let pattern = PathPattern::new("/users/").unwrap();

	// Act
	let raw_pattern = pattern.pattern();
	let param_names = pattern.param_names();

	// Assert
	assert_eq!(raw_pattern, "/users/");
	assert!(param_names.is_empty());
}

#[rstest]
fn path_pattern_with_simple_param() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();

	// Act
	let names = pattern.param_names();

	// Assert
	assert_eq!(names, &["id"]);
}

#[rstest]
fn path_pattern_multiple_params() {
	// Arrange
	let pattern = PathPattern::new("/users/{user_id}/posts/{post_id}/").unwrap();

	// Act
	let names = pattern.param_names();

	// Assert
	assert_eq!(names, &["user_id", "post_id"]);
}

#[rstest]
fn path_pattern_empty_param_name_is_error() {
	// Arrange
	let pattern = "/users/{}/";

	// Act
	let result = PathPattern::new(pattern);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn path_pattern_too_long_is_error() {
	// Arrange
	let long_pattern = "/".to_string() + &"a".repeat(1025);

	// Act
	let result = PathPattern::new(long_pattern);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn path_pattern_too_many_segments_is_error() {
	// Arrange - 33 slashes creates 34 segments, exceeding the limit of 32
	let pattern = "/".to_string() + &"a/".repeat(33);

	// Act
	let result = PathPattern::new(pattern);

	// Assert
	assert!(result.is_err());
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - int type
// ===================================================================

#[rstest]
#[case("123", true)]
#[case("0", true)]
#[case("9999", true)]
#[case("-1", false)]
#[case("abc", false)]
fn type_int_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<int:id>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"int type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - str type
// ===================================================================

#[rstest]
#[case("hello", true)]
#[case("hello-world", true)]
#[case("123", true)]
#[case("hello/world", false)]
fn type_str_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<str:name>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"str type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - uuid type
// ===================================================================

#[rstest]
#[case("550e8400-e29b-41d4-a716-446655440000", true)]
// NOTE: Uppercase UUIDs accepted by type_spec_to_regex but rejected by UuidConverter (see #1818)
#[case("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE", true)]
#[case("not-a-uuid", false)]
#[case("550e8400e29b41d4a716446655440000", false)]
fn type_uuid_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<uuid:id>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"uuid type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - slug type
// ===================================================================

#[rstest]
#[case("hello-world", true)]
#[case("hello", true)]
#[case("hello-world-123", true)]
#[case("HELLO", false)]
#[case("hello world", false)]
#[case("-leading-dash", false)]
fn type_slug_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<slug:name>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"slug type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - bool type
// ===================================================================

#[rstest]
#[case("true", true)]
#[case("false", true)]
#[case("1", true)]
#[case("0", true)]
#[case("yes", false)]
#[case("no", false)]
#[case("True", false)]
fn type_bool_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<bool:flag>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"bool type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - date type
// ===================================================================

#[rstest]
#[case("2024-01-15", true)]
#[case("1999-12-31", true)]
#[case("2024-1-5", false)]
#[case("20240115", false)]
#[case("2024/01/15", false)]
fn type_date_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<date:d>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"date type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - email type
// ===================================================================

#[rstest]
#[case("user@example.com", true)]
#[case("user.name+tag@sub.domain.org", true)]
#[case("not-an-email", false)]
#[case("@example.com", false)]
#[case("user@", false)]
fn type_email_matching(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/items/{<email:addr>}/".to_string()).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"email type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - signed integers
// ===================================================================

#[rstest]
#[case("i8", "42", true)]
#[case("i8", "-42", true)]
#[case("i16", "1000", true)]
#[case("i16", "-1000", true)]
#[case("i32", "99999", true)]
#[case("i32", "-99999", true)]
#[case("i64", "123456789", true)]
#[case("i64", "-123456789", true)]
#[case("i8", "abc", false)]
fn type_signed_integers_matching(
	#[case] type_name: &str,
	#[case] value: &str,
	#[case] should_match: bool,
) {
	// Arrange
	let pattern = PathPattern::new(format!("/items/{{<{}:n>}}/", type_name)).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"{} type for value '{}' should_match={}",
		type_name, value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - unsigned integers
// ===================================================================

#[rstest]
#[case("u8", "255", true)]
#[case("u8", "0", true)]
#[case("u16", "65535", true)]
#[case("u32", "4294967295", true)]
#[case("u64", "18446744073709551615", true)]
#[case("u8", "-1", false)]
#[case("u16", "abc", false)]
fn type_unsigned_integers_matching(
	#[case] type_name: &str,
	#[case] value: &str,
	#[case] should_match: bool,
) {
	// Arrange
	let pattern = PathPattern::new(format!("/items/{{<{}:n>}}/", type_name)).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"{} type for value '{}' should_match={}",
		type_name, value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - floating point
// ===================================================================

#[rstest]
#[case("f32", "3.14", true)]
#[case("f32", "-2.5", true)]
#[case("f32", "42", true)]
#[case("f64", "3.141592653589793", true)]
#[case("f64", "-0.001", true)]
#[case("f32", "abc", false)]
#[case("f64", "1.2.3", false)]
fn type_float_matching(#[case] type_name: &str, #[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new(format!("/items/{{<{}:n>}}/", type_name)).unwrap();
	let path = format!("/items/{}/", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"{} type for value '{}' should_match={}",
		type_name, value, should_match
	);
}

// ===================================================================
// type_spec_to_regex via PathPattern matching - path type
// ===================================================================

#[rstest]
#[case("a/b/c.txt", true)]
#[case("single", true)]
#[case("deep/nested/file.json", true)]
fn type_path_matching_valid(#[case] value: &str, #[case] should_match: bool) {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();
	let path = format!("/files/{}", value);

	// Act
	let matched = pattern.is_match(&path);

	// Assert
	assert_eq!(
		matched, should_match,
		"path type for value '{}' should_match={}",
		value, should_match
	);
}

// ===================================================================
// validate_path_param security tests via extract_params
// ===================================================================

#[rstest]
#[case("../etc/passwd")]
#[case("a/../etc/passwd")]
#[case("a/..")]
fn path_traversal_dot_dot_rejected(#[case] dangerous_value: &str) {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();
	let path = format!("/files/{}", dangerous_value);

	// Act
	let params = pattern.extract_params(&path);

	// Assert
	assert!(
		params.is_none(),
		"directory traversal '{}' should be rejected",
		dangerous_value
	);
}

#[rstest]
#[case("%2e%2e/etc/passwd")]
#[case("%2E%2E/secret")]
#[case("a/%2f/b")]
#[case("a/%2F/b")]
fn path_traversal_encoded_rejected(#[case] dangerous_value: &str) {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();
	let path = format!("/files/{}", dangerous_value);

	// Act
	let params = pattern.extract_params(&path);

	// Assert
	assert!(
		params.is_none(),
		"encoded traversal '{}' should be rejected",
		dangerous_value
	);
}

#[rstest]
#[case("%5c..%5c")]
#[case("a%5cpasswd")]
fn path_traversal_backslash_encoded_rejected(#[case] dangerous_value: &str) {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();
	let path = format!("/files/{}", dangerous_value);

	// Act
	let params = pattern.extract_params(&path);

	// Assert
	assert!(
		params.is_none(),
		"backslash encoded '{}' should be rejected",
		dangerous_value
	);
}

#[rstest]
fn path_param_with_null_byte_rejected() {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

	// Act - null bytes cannot be embedded in literal strings easily, test via %00
	let path = "/files/a%00b";
	let params = pattern.extract_params(path);

	// Assert
	match params {
		Some(params) => {
			let filepath = params.get("filepath").unwrap();
			assert!(
				!filepath.contains("%00"),
				"null byte encoded path should be rejected"
			);
		}
		// Path rejection at routing level is acceptable
		None => {}
	}
}

// ===================================================================
// extract_params - correct parameter extraction
// ===================================================================

#[rstest]
fn extract_params_single_param() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();

	// Act
	let params = pattern.extract_params("/users/123/").unwrap();

	// Assert
	assert_eq!(params.get("id"), Some(&"123".to_string()));
}

#[rstest]
fn extract_params_multiple_params() {
	// Arrange
	let pattern = PathPattern::new("/users/{user_id}/posts/{post_id}/").unwrap();

	// Act
	let params = pattern.extract_params("/users/42/posts/99/").unwrap();

	// Assert
	assert_eq!(params.get("user_id"), Some(&"42".to_string()));
	assert_eq!(params.get("post_id"), Some(&"99".to_string()));
}

#[rstest]
fn extract_params_typed_int_param() {
	// Arrange
	let pattern = PathPattern::new("/users/{<int:id>}/").unwrap();

	// Act
	let params = pattern.extract_params("/users/42/").unwrap();

	// Assert
	assert_eq!(params.get("id"), Some(&"42".to_string()));
}

#[rstest]
fn extract_params_no_match_returns_none() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();

	// Act
	let params = pattern.extract_params("/posts/123/");

	// Assert
	assert!(params.is_none());
}

#[rstest]
fn extract_params_path_type_valid() {
	// Arrange
	let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

	// Act
	let params = pattern.extract_params("/files/a/b/c.txt").unwrap();

	// Assert
	assert_eq!(params.get("filepath"), Some(&"a/b/c.txt".to_string()));
}

// ===================================================================
// PathPattern::reverse - URL reversal
// ===================================================================

#[rstest]
fn reverse_no_params() {
	// Arrange
	let pattern = PathPattern::new("/users/").unwrap();
	let params = HashMap::new();

	// Act
	let url = pattern.reverse(&params).unwrap();

	// Assert
	assert_eq!(url, "/users/");
}

#[rstest]
fn reverse_single_param() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	let mut params = HashMap::new();
	params.insert("id".to_string(), "123".to_string());

	// Act
	let url = pattern.reverse(&params).unwrap();

	// Assert
	assert_eq!(url, "/users/123/");
}

#[rstest]
fn reverse_multiple_params() {
	// Arrange
	let pattern = PathPattern::new("/users/{user_id}/posts/{post_id}/").unwrap();
	let mut params = HashMap::new();
	params.insert("user_id".to_string(), "42".to_string());
	params.insert("post_id".to_string(), "99".to_string());

	// Act
	let url = pattern.reverse(&params).unwrap();

	// Assert
	assert_eq!(url, "/users/42/posts/99/");
}

#[rstest]
fn reverse_missing_param_returns_error() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	let params = HashMap::new();

	// Act
	let result = pattern.reverse(&params);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn reverse_param_with_slash_rejected() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	let mut params = HashMap::new();
	params.insert("id".to_string(), "12/3".to_string());

	// Act
	let result = pattern.reverse(&params);

	// Assert
	assert!(
		result.is_err(),
		"param value containing '/' should be rejected for reversal"
	);
}

#[rstest]
fn reverse_param_with_traversal_rejected() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	let mut params = HashMap::new();
	params.insert("id".to_string(), "..".to_string());

	// Act
	let result = pattern.reverse(&params);

	// Assert
	assert!(
		result.is_err(),
		"param value '..' should be rejected for reversal"
	);
}

// ===================================================================
// PathMatcher - linear mode
// ===================================================================

#[rstest]
fn path_matcher_linear_no_match() {
	// Arrange
	let matcher = PathMatcher::new();

	// Act
	let result = matcher.match_path("/users/");

	// Assert
	assert!(result.is_none());
}

#[rstest]
fn path_matcher_linear_static_match() {
	// Arrange
	let mut matcher = PathMatcher::new();
	let pattern = PathPattern::new("/users/").unwrap();
	matcher.add_pattern(pattern, "users_list".to_string());

	// Act
	let result = matcher.match_path("/users/");

	// Assert
	assert!(result.is_some());
	let (handler_id, params) = result.unwrap();
	assert_eq!(handler_id, "users_list");
	assert!(params.is_empty());
}

#[rstest]
fn path_matcher_linear_param_match() {
	// Arrange
	let mut matcher = PathMatcher::new();
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	matcher.add_pattern(pattern, "users_detail".to_string());

	// Act
	let result = matcher.match_path("/users/42/");

	// Assert
	assert!(result.is_some());
	let (handler_id, params) = result.unwrap();
	assert_eq!(handler_id, "users_detail");
	assert_eq!(params.get("id"), Some(&"42".to_string()));
}

#[rstest]
fn path_matcher_linear_first_match_wins() {
	// Arrange
	let mut matcher = PathMatcher::new();
	let pattern1 = PathPattern::new("/items/{id}/").unwrap();
	let pattern2 = PathPattern::new("/items/{<int:id>}/").unwrap();
	matcher.add_pattern(pattern1, "first".to_string());
	matcher.add_pattern(pattern2, "second".to_string());

	// Act
	let result = matcher.match_path("/items/123/");

	// Assert
	assert!(result.is_some());
	let (handler_id, _) = result.unwrap();
	assert_eq!(handler_id, "first");
}

// ===================================================================
// PathMatcher - radix tree mode
// ===================================================================

#[rstest]
fn path_matcher_radix_mode() {
	// Arrange
	let mut matcher = PathMatcher::with_mode(MatchingMode::RadixTree);
	let pattern = PathPattern::new("/users/{id}/").unwrap();
	matcher.add_pattern(pattern, "users_detail".to_string());

	// Act
	let result = matcher.match_path("/users/42/");

	// Assert
	assert!(result.is_some());
	let (handler_id, params) = result.unwrap();
	assert_eq!(handler_id, "users_detail");
	assert_eq!(params.get("id"), Some(&"42".to_string()));
}

#[rstest]
fn path_matcher_enable_radix_tree() {
	// Arrange
	let mut matcher = PathMatcher::new();
	assert_eq!(matcher.mode(), MatchingMode::Linear);

	let pattern = PathPattern::new("/users/").unwrap();
	matcher.add_pattern(pattern, "users_list".to_string());
	matcher.enable_radix_tree();

	// Act
	let result = matcher.match_path("/users/");

	// Assert
	assert_eq!(matcher.mode(), MatchingMode::RadixTree);
	assert!(result.is_some());
}

// ===================================================================
// PathPattern::is_match
// ===================================================================

#[rstest]
fn is_match_true_for_matching_path() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();

	// Act
	let matched = pattern.is_match("/users/123/");

	// Assert
	assert!(matched);
}

#[rstest]
fn is_match_false_for_non_matching_path() {
	// Arrange
	let pattern = PathPattern::new("/users/{id}/").unwrap();

	// Act
	let match_no_id = pattern.is_match("/users/");
	let match_wrong_prefix = pattern.is_match("/posts/123/");

	// Assert
	assert!(!match_no_id);
	assert!(!match_wrong_prefix);
}

#[rstest]
fn is_match_typed_uuid_pattern() {
	// Arrange
	let pattern = PathPattern::new("/users/{<uuid:id>}/").unwrap();

	// Act
	let match_valid = pattern.is_match("/users/550e8400-e29b-41d4-a716-446655440000/");
	let match_invalid = pattern.is_match("/users/not-a-uuid/");

	// Assert
	assert!(match_valid);
	assert!(!match_invalid);
}

// ===================================================================
// Typed parameter name parsing
// ===================================================================

#[rstest]
fn typed_param_name_extracted_correctly() {
	// Arrange
	let pattern = PathPattern::new("/users/{<int:user_id>}/posts/{<uuid:post_uuid>}/").unwrap();

	// Act
	let names = pattern.param_names();

	// Assert
	assert_eq!(names, &["user_id", "post_uuid"]);
}

#[rstest]
fn invalid_typed_param_no_colon_is_error() {
	// Arrange
	let pattern = "/users/{<intid>}/";

	// Act
	let result = PathPattern::new(pattern);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn invalid_typed_param_empty_name_is_error() {
	// Arrange
	let pattern = "/users/{<int:>}/";

	// Act
	let result = PathPattern::new(pattern);

	// Assert
	assert!(result.is_err());
}

// ===================================================================
// Default (unknown) type spec falls back to str-like matching
// ===================================================================

#[rstest]
fn unknown_type_spec_falls_back_to_str() {
	// Arrange
	let pattern = PathPattern::new("/items/{<unknowntype:val>}/").unwrap();

	// Act - unknown type uses [^/]+ (same as str)
	let match_single = pattern.is_match("/items/hello/");
	let match_nested = pattern.is_match("/items/hello/world/");

	// Assert
	assert!(match_single);
	assert!(!match_nested);
}
