//! Response cookies for server function handlers.
//!
//! Allows server functions to set `Set-Cookie` headers on HTTP responses.
//! The server function router inserts a [`SharedResponseCookies`] jar into
//! request extensions before calling the handler. The handler adds cookies
//! via the shared jar, and the router extracts them after the handler returns.
//!
//! # How it works
//!
//! [`Extensions`](crate::Extensions) uses `Arc<Mutex<HashMap>>` internally,
//! so cloning an `Extensions` value shares the same backing store. The
//! server function router clones the request's extensions *before* calling
//! the handler. Any [`ResponseCookies`] the handler inserts into
//! `request.extensions` are therefore visible through the cloned reference,
//! and the router can extract and apply them after the handler returns.
//!
//! # Usage in a handler
//!
//! Insert a [`ResponseCookies`] into the request's extensions inside your
//! server function handler. **Do not** construct `ResponseCookies`
//! separately and return it — it must be placed into the request's
//! extensions so the router can find it.
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
/// response layer. Because `Extensions` is backed by `Arc<Mutex<HashMap>>`,
/// cloning it shares the same underlying map. The server function router
/// exploits this: it clones the extensions before invoking the handler, then
/// extracts `ResponseCookies` from the clone afterwards. Each cookie is
/// applied as a `Set-Cookie` header on the HTTP response.
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
