//! Named cookie parameter extraction
//!
//! Provides compile-time cookie name specification using marker types.

use async_trait::async_trait;
use reinhardt_http::Request;
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::ops::Deref;

use super::{ParamContext, ParamError, ParamResult, extract::FromRequest};

/// Marker trait for cookie names
pub trait CookieName {
	const NAME: &'static str;
}

/// Marker type for sessionid cookie
pub struct SessionId;
impl CookieName for SessionId {
	const NAME: &'static str = "sessionid";
}

/// Marker type for csrftoken cookie
pub struct CsrfToken;
impl CookieName for CsrfToken {
	const NAME: &'static str = "csrftoken";
}

/// Extract a value from cookies with compile-time name specification
///
/// Unlike `Cookie<T>` which requires runtime ParamContext configuration,
/// `CookieNamed` specifies the cookie name at compile time using marker types.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_di::params::{CookieNamed, SessionId, CsrfToken};
/// async fn handler(session: CookieNamed<SessionId, String>) {
///     println!("Session ID: {}", *session);
/// }
///
/// async fn handler_optional(token: CookieNamed<CsrfToken, Option<String>>) {
///     if let Some(t) = token.into_inner() {
///         println!("CSRF Token: {}", t);
///     }
/// }
/// ```
pub struct CookieNamed<N: CookieName, T> {
	value: T,
	_phantom: PhantomData<N>,
}

impl<N: CookieName, T> CookieNamed<N, T> {
	/// Unwrap the CookieNamed and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{CookieNamed, SessionId};
	///
	/// let cookie = CookieNamed::<SessionId, String>::new("abc123".to_string());
	/// let inner = cookie.into_inner();
	/// assert_eq!(inner, "abc123");
	/// ```
	pub fn into_inner(self) -> T {
		self.value
	}

	/// Create a new CookieNamed with a value
	///
	/// This is useful for testing or manual construction.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{CookieNamed, SessionId};
	///
	/// let cookie = CookieNamed::<SessionId, String>::new("user_12345".to_string());
	/// assert_eq!(*cookie, "user_12345");
	/// ```
	pub fn new(value: T) -> Self {
		CookieNamed {
			value,
			_phantom: PhantomData,
		}
	}

	/// Get the cookie name as a compile-time constant
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{CookieNamed, CsrfToken};
	///
	/// let cookie = CookieNamed::<CsrfToken, String>::new("xyz789".to_string());
	/// assert_eq!(CookieNamed::<CsrfToken, String>::name(), "csrftoken");
	/// ```
	pub const fn name() -> &'static str {
		N::NAME
	}
}

impl<N: CookieName, T> Deref for CookieNamed<N, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<N: CookieName, T: Debug> Debug for CookieNamed<N, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("CookieNamed")
			.field("name", &N::NAME)
			.field("value", &self.value)
			.finish()
	}
}

use super::cookie_util::parse_cookies;

#[async_trait]
impl FromRequest for CookieNamed<SessionId, String> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		let value = cookies_map
			.get("sessionid")
			.ok_or_else(|| ParamError::MissingParameter("sessionid".to_string()))?;

		Ok(CookieNamed::new(value.to_string()))
	}
}

#[async_trait]
impl FromRequest for CookieNamed<CsrfToken, String> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		let value = cookies_map
			.get("csrftoken")
			.ok_or_else(|| ParamError::MissingParameter("csrftoken".to_string()))?;

		Ok(CookieNamed::new(value.to_string()))
	}
}

#[async_trait]
impl FromRequest for CookieNamed<SessionId, Option<String>> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		let maybe = cookies_map.get("sessionid").map(|s| s.to_string());

		Ok(CookieNamed::new(maybe))
	}
}

#[async_trait]
impl FromRequest for CookieNamed<CsrfToken, Option<String>> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		let maybe = cookies_map.get("csrftoken").map(|s| s.to_string());

		Ok(CookieNamed::new(maybe))
	}
}

// Implement WithValidation trait for CookieNamed
#[cfg(feature = "validation")]
impl<N: CookieName, T> super::validation::WithValidation for CookieNamed<N, T> {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cookie_named_new() {
		let cookie = CookieNamed::<SessionId, String>::new("12345".to_string());
		assert_eq!(*cookie, "12345");
		assert_eq!(CookieNamed::<SessionId, String>::name(), "sessionid");
	}

	#[test]
	fn test_cookie_named_into_inner() {
		let cookie = CookieNamed::<CsrfToken, String>::new("abc-def-ghi".to_string());
		let value = cookie.into_inner();
		assert_eq!(value, "abc-def-ghi");
	}

	#[test]
	fn test_cookie_named_deref() {
		let cookie = CookieNamed::<SessionId, String>::new("session123".to_string());
		assert_eq!(&*cookie, "session123");
	}

	#[test]
	fn test_cookie_named_optional() {
		let cookie1 = CookieNamed::<CsrfToken, Option<String>>::new(Some("dark".to_string()));
		assert_eq!(*cookie1, Some("dark".to_string()));

		let cookie2 = CookieNamed::<CsrfToken, Option<String>>::new(None);
		assert_eq!(*cookie2, None);
	}

	#[test]
	fn test_parse_cookies() {
		let cookies = parse_cookies("sessionid=abc123; csrftoken=xyz789; user=john");
		assert_eq!(cookies.get("sessionid"), Some(&"abc123".to_string()));
		assert_eq!(cookies.get("csrftoken"), Some(&"xyz789".to_string()));
		assert_eq!(cookies.get("user"), Some(&"john".to_string()));
	}

	#[test]
	fn test_parse_cookies_with_encoding() {
		let cookies = parse_cookies("name=value%20with%20spaces");
		assert_eq!(cookies.get("name"), Some(&"value with spaces".to_string()));
	}
}
