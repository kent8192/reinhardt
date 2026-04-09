//! Response cookies for server function handlers.
//!
//! Allows server functions to set `Set-Cookie` headers on HTTP responses.
//! The server function router inserts a [`SharedResponseCookies`] jar into
//! request extensions before calling the handler. The handler adds cookies
//! via the shared jar, and the router extracts them after the handler returns.
//!
//! # How it works
//!
//! The server function router creates a [`SharedResponseCookies`] jar and
//! inserts it into the request's extensions before calling the handler.
//! Because clones of `SharedResponseCookies` share the same backing store,
//! cookies added by the handler are visible to the router after the handler
//! returns.
//!
//! # Usage in a handler
//!
//! The router inserts a [`SharedResponseCookies`] into the request's
//! extensions before calling the handler. The handler retrieves it and
//! adds cookies via [`SharedResponseCookies::add`].
//!
//! ```
//! use reinhardt_http::SharedResponseCookies;
//!
//! let jar = SharedResponseCookies::new();
//! let jar2 = jar.clone(); // clones share the same backing store
//! jar.add("session=abc123; Path=/; HttpOnly".to_string());
//! let cookies = jar2.take();
//! assert_eq!(cookies.cookies().len(), 1);
//! ```

use std::sync::{Arc, Mutex};

/// A collection of `Set-Cookie` header values to include in the HTTP response.
///
/// Server function handlers insert this into the request's
/// [`Extensions`](crate::Extensions) to communicate cookies back to the
/// response layer. The server function router wraps the handler call so
/// that any `ResponseCookies` added to the extensions are extracted
/// afterwards and applied as `Set-Cookie` headers on the HTTP response.
///
/// **Important:** `ResponseCookies` must be inserted into the request's
/// extensions — not held separately — for the cookies to reach the
/// response.
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

/// A shared, thread-safe cookie jar for passing response cookies between
/// the server function router wrapper and the handler.
///
/// Unlike [`ResponseCookies`], this type uses interior mutability via
/// `Arc<Mutex<>>` so that both the router wrapper and the handler can
/// read/write cookies through the same shared instance. The router inserts
/// a `SharedResponseCookies` into request extensions before calling the
/// handler; the handler adds cookies; the router reads them afterward.
///
/// # Example
///
/// ```
/// use reinhardt_http::SharedResponseCookies;
///
/// let jar = SharedResponseCookies::new();
/// let jar_clone = jar.clone();
///
/// jar.add("session=abc; Path=/; HttpOnly".to_string());
///
/// // The clone sees the same cookies
/// let cookies = jar_clone.take();
/// assert_eq!(cookies.cookies(), &["session=abc; Path=/; HttpOnly"]);
/// ```
#[derive(Clone, Default)]
pub struct SharedResponseCookies {
	inner: Arc<Mutex<ResponseCookies>>,
}

impl SharedResponseCookies {
	/// Creates a new empty shared cookie jar.
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(ResponseCookies::new())),
		}
	}

	/// Adds a `Set-Cookie` header value to the shared jar.
	pub fn add(&self, cookie: String) {
		self.inner
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.add(cookie);
	}

	/// Takes all cookies out of the jar, leaving it empty.
	pub fn take(&self) -> ResponseCookies {
		std::mem::take(&mut *self.inner.lock().unwrap_or_else(|e| e.into_inner()))
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

	#[rstest]
	fn test_shared_add_and_take() {
		// Arrange
		let jar = SharedResponseCookies::new();

		// Act
		jar.add("session=abc; Path=/".to_string());
		jar.add("csrf=xyz; SameSite=Strict".to_string());
		let cookies = jar.take();

		// Assert
		assert_eq!(cookies.cookies().len(), 2);
		assert_eq!(cookies.cookies()[0], "session=abc; Path=/");
		assert_eq!(cookies.cookies()[1], "csrf=xyz; SameSite=Strict");
	}

	#[rstest]
	fn test_shared_take_empties_jar() {
		// Arrange
		let jar = SharedResponseCookies::new();
		jar.add("session=abc; Path=/".to_string());

		// Act
		let first_take = jar.take();
		let second_take = jar.take();

		// Assert
		assert_eq!(first_take.cookies().len(), 1);
		assert!(second_take.cookies().is_empty());
	}

	#[rstest]
	fn test_shared_clone_shares_state() {
		// Arrange
		let jar = SharedResponseCookies::new();
		let jar_clone = jar.clone();

		// Act - add via clone, read via original
		jar_clone.add("session=abc; Path=/".to_string());
		let cookies = jar.take();

		// Assert
		assert_eq!(cookies.cookies().len(), 1);
		assert_eq!(cookies.cookies()[0], "session=abc; Path=/");
	}

	#[rstest]
	fn test_shared_default_is_empty() {
		// Arrange & Act
		let jar = SharedResponseCookies::default();
		let cookies = jar.take();

		// Assert
		assert!(cookies.cookies().is_empty());
	}
}
