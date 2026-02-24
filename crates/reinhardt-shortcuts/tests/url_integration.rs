//! URL type integration tests
//!
//! Tests URL validation, conversion, and various URL formats including
//! relative paths, absolute paths, query parameters, and error cases.

use reinhardt_shortcuts::{Url, UrlError};

/// Test: Valid relative URL paths
#[test]
fn test_valid_relative_urls() {
	// Simple relative paths
	let url = Url::new("/").unwrap();
	assert_eq!(url.as_str(), "/");

	let url = Url::new("/home").unwrap();
	assert_eq!(url.as_str(), "/home");

	let url = Url::new("/users/123").unwrap();
	assert_eq!(url.as_str(), "/users/123");

	// Nested paths
	let url = Url::new("/api/v1/users/profile").unwrap();
	assert_eq!(url.as_str(), "/api/v1/users/profile");
}

/// Test: Valid absolute URLs
#[test]
fn test_valid_absolute_urls() {
	// HTTP URLs
	let url = Url::new("http://example.com").unwrap();
	assert_eq!(url.as_str(), "http://example.com");

	// HTTPS URLs
	let url = Url::new("https://example.com/path").unwrap();
	assert_eq!(url.as_str(), "https://example.com/path");

	// With port
	let url = Url::new("http://localhost:8000").unwrap();
	assert_eq!(url.as_str(), "http://localhost:8000");

	// With subdomain
	let url = Url::new("https://api.example.com/v1/users").unwrap();
	assert_eq!(url.as_str(), "https://api.example.com/v1/users");
}

/// Test: URLs with query parameters
#[test]
fn test_urls_with_query_parameters() {
	// Single parameter
	let url = Url::new("/search?q=rust").unwrap();
	assert_eq!(url.as_str(), "/search?q=rust");

	// Multiple parameters
	let url = Url::new("/search?q=rust&page=2&limit=10").unwrap();
	assert_eq!(url.as_str(), "/search?q=rust&page=2&limit=10");

	// Absolute URL with query
	let url = Url::new("https://example.com/search?q=test").unwrap();
	assert_eq!(url.as_str(), "https://example.com/search?q=test");
}

/// Test: URLs with fragments
#[test]
fn test_urls_with_fragments() {
	// Fragment only
	let url = Url::new("/page#section").unwrap();
	assert_eq!(url.as_str(), "/page#section");

	// Query and fragment
	let url = Url::new("/page?id=123#section").unwrap();
	assert_eq!(url.as_str(), "/page?id=123#section");

	// Absolute URL with fragment
	let url = Url::new("https://example.com/docs#introduction").unwrap();
	assert_eq!(url.as_str(), "https://example.com/docs#introduction");
}

/// Test: Empty URL validation error
#[test]
fn test_empty_url_error() {
	let result = Url::new("");
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), UrlError::Empty);
}

/// Test: Whitespace-only URL validation error
#[test]
fn test_whitespace_only_url_error() {
	let result = Url::new("   ");
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), UrlError::Whitespace);

	let result = Url::new("\t\n  ");
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), UrlError::Whitespace);
}

/// Test: URL Display trait implementation
#[test]
fn test_url_display() {
	let url = Url::new("/home").unwrap();
	assert_eq!(format!("{}", url), "/home");

	let url = Url::new("https://example.com").unwrap();
	assert_eq!(format!("{}", url), "https://example.com");
}

/// Test: URL AsRef<str> implementation
#[test]
fn test_url_as_ref() {
	let url = Url::new("/page").unwrap();
	let url_ref: &str = url.as_ref();
	assert_eq!(url_ref, "/page");
}

/// Test: URL into_string conversion
#[test]
fn test_url_into_string() {
	let url = Url::new("/about").unwrap();
	let string: String = url.into_string();
	assert_eq!(string, "/about");
}

/// Test: URL TryFrom<String> conversion with validation
#[test]
fn test_url_try_from_string() {
	// Arrange
	let valid_string = String::from("/users");
	let empty_string = String::from("");
	let whitespace_string = String::from("   ");

	// Act & Assert - valid string succeeds
	let url = Url::try_from(valid_string).unwrap();
	assert_eq!(url.as_str(), "/users");

	// Act & Assert - empty string is rejected
	let result = Url::try_from(empty_string);
	assert_eq!(result.unwrap_err(), UrlError::Empty);

	// Act & Assert - whitespace-only string is rejected
	let result = Url::try_from(whitespace_string);
	assert_eq!(result.unwrap_err(), UrlError::Whitespace);
}

/// Test: URL TryFrom<&str> conversion with validation
#[test]
fn test_url_try_from_str() {
	// Arrange & Act & Assert - valid str succeeds
	let url = Url::try_from("/profile").unwrap();
	assert_eq!(url.as_str(), "/profile");

	// Act & Assert - empty str is rejected
	let result = Url::try_from("");
	assert_eq!(result.unwrap_err(), UrlError::Empty);

	// Act & Assert - whitespace-only str is rejected
	let result = Url::try_from("   ");
	assert_eq!(result.unwrap_err(), UrlError::Whitespace);
}

/// Test: TryFrom validates identically to Url::new
#[test]
fn test_try_from_matches_new_validation() {
	// Arrange
	let test_cases = vec![
		("/valid", true),
		("https://example.com", true),
		("", false),
		("   ", false),
		("\t\n", false),
	];

	for (input, should_succeed) in test_cases {
		// Act
		let new_result = Url::new(input);
		let try_from_result = Url::try_from(input);

		// Assert - both paths produce the same result
		assert_eq!(
			new_result.is_ok(),
			should_succeed,
			"Url::new for {:?}",
			input
		);
		assert_eq!(
			try_from_result.is_ok(),
			should_succeed,
			"TryFrom for {:?}",
			input
		);

		if let (Ok(new_url), Ok(try_url)) = (&new_result, &try_from_result) {
			assert_eq!(new_url, try_url);
		}
		if let (Err(new_err), Err(try_err)) = (&new_result, &try_from_result) {
			assert_eq!(new_err, try_err);
		}
	}
}

/// Test: URL cloning
#[test]
fn test_url_clone() {
	let url1 = Url::new("/original").unwrap();
	let url2 = url1.clone();

	assert_eq!(url1.as_str(), url2.as_str());
	assert_eq!(url1, url2);
}

/// Test: URL equality
#[test]
fn test_url_equality() {
	let url1 = Url::new("/same").unwrap();
	let url2 = Url::new("/same").unwrap();
	let url3 = Url::new("/different").unwrap();

	assert_eq!(url1, url2);
	assert_ne!(url1, url3);
}

/// Test: UTF-8 URLs (encoded and unencoded)
#[test]
fn test_utf8_urls() {
	// Unencoded UTF-8 (for internal representation)
	let url = Url::new("/search?q=日本語").unwrap();
	assert_eq!(url.as_str(), "/search?q=日本語");

	// Percent-encoded UTF-8 (typical for actual HTTP URLs)
	let url = Url::new("/search?q=%E6%97%A5%E6%9C%AC%E8%AA%9E").unwrap();
	assert_eq!(url.as_str(), "/search?q=%E6%97%A5%E6%9C%AC%E8%AA%9E");
}

/// Test: Special characters in URLs
#[test]
fn test_special_characters_in_urls() {
	// Spaces (should be percent-encoded in real URLs, but URL type doesn't enforce)
	let url = Url::new("/path with spaces").unwrap();
	assert_eq!(url.as_str(), "/path with spaces");

	// Percent-encoded special characters
	let url = Url::new("/path%20with%20encoded%20spaces").unwrap();
	assert_eq!(url.as_str(), "/path%20with%20encoded%20spaces");

	// Ampersands in query
	let url = Url::new("/page?a=1&b=2&c=3").unwrap();
	assert_eq!(url.as_str(), "/page?a=1&b=2&c=3");
}

/// Test: Very long URLs
#[test]
fn test_long_urls() {
	let long_path = format!("/very/long/path/{}", "segment/".repeat(100));
	let url = Url::new(&long_path).unwrap();
	assert_eq!(url.as_str(), long_path);

	let long_query = format!(
		"/search?{}",
		(0..50)
			.map(|i| format!("param{}=value{}", i, i))
			.collect::<Vec<_>>()
			.join("&")
	);
	let url = Url::new(&long_query).unwrap();
	assert_eq!(url.as_str(), long_query);
}

/// Test: Error message formatting
#[test]
fn test_error_message_formatting() {
	let error = UrlError::Empty;
	assert_eq!(error.to_string(), "URL cannot be empty");

	let error = UrlError::Whitespace;
	assert_eq!(error.to_string(), "URL cannot contain only whitespace");
}
