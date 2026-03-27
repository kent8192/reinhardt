//! Response cookies for server function handlers.
//!
//! Allows server functions to set `Set-Cookie` headers on HTTP responses
//! by inserting a [`ResponseCookies`] value into the request extensions.
//! The server function router extracts these and applies them to the
//! outgoing HTTP response.
//!
//! # Example
//!
//! ```
//! use reinhardt_http::ResponseCookies;
//!
//! let mut cookies = ResponseCookies::new();
//! cookies.add("session=abc123; Path=/; HttpOnly".to_string());
//! assert_eq!(cookies.cookies().len(), 1);
//! ```

/// A collection of `Set-Cookie` header values to include in the HTTP response.
///
/// Server functions can insert this into the request's extensions to communicate
/// cookies to the response layer. The server function router checks for this
/// type in the request extensions and applies each cookie as a `Set-Cookie`
/// header on the HTTP response.
#[derive(Debug, Clone, Default)]
pub struct ResponseCookies {
	/// Cookie header values to include in the response
	cookies: Vec<String>,
}

impl ResponseCookies {
	/// Creates a new empty `ResponseCookies`.
	pub fn new() -> Self {
		Self {
			cookies: Vec::new(),
		}
	}

	/// Adds a `Set-Cookie` header value.
	pub fn add(&mut self, cookie: String) {
		self.cookies.push(cookie);
	}

	/// Returns the cookie header values.
	pub fn cookies(&self) -> &[String] {
		&self.cookies
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_new_response_cookies_is_empty() {
		// Arrange & Act
		let cookies = ResponseCookies::new();

		// Assert
		assert!(cookies.cookies().is_empty());
	}

	#[rstest]
	fn test_add_single_cookie() {
		// Arrange
		let mut cookies = ResponseCookies::new();

		// Act
		cookies.add("session=abc; Path=/".to_string());

		// Assert
		assert_eq!(cookies.cookies().len(), 1);
		assert_eq!(cookies.cookies()[0], "session=abc; Path=/");
	}

	#[rstest]
	fn test_add_multiple_cookies() {
		// Arrange
		let mut cookies = ResponseCookies::new();

		// Act
		cookies.add("session=abc; Path=/".to_string());
		cookies.add("csrf=xyz; SameSite=Strict".to_string());

		// Assert
		assert_eq!(cookies.cookies().len(), 2);
		assert_eq!(cookies.cookies()[0], "session=abc; Path=/");
		assert_eq!(cookies.cookies()[1], "csrf=xyz; SameSite=Strict");
	}

	#[rstest]
	fn test_default_is_empty() {
		// Arrange & Act
		let cookies = ResponseCookies::default();

		// Assert
		assert!(cookies.cookies().is_empty());
	}
}
