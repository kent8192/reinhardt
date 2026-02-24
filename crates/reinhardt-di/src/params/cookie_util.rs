//! Shared cookie parsing utilities
//!
//! Provides the `parse_cookies` function used by both `cookie` and `cookie_named` modules.

use std::collections::HashMap;

/// Parse a Cookie header string into a map of name-value pairs.
///
/// Cookie header format: `"name1=value1; name2=value2"`
///
/// Values are URL-decoded if they contain percent-encoded characters.
pub(crate) fn parse_cookies(cookie_header: &str) -> HashMap<String, String> {
	let mut result = HashMap::new();

	for cookie in cookie_header.split(';') {
		let cookie = cookie.trim();
		if let Some((name, value)) = cookie.split_once('=') {
			let name = name.trim().to_string();
			let value = value.trim();

			// URL-decode the value if it contains encoded characters
			let decoded_value = urlencoding::decode(value)
				.unwrap_or(std::borrow::Cow::Borrowed(value))
				.into_owned();

			result.insert(name, decoded_value);
		}
	}

	result
}
