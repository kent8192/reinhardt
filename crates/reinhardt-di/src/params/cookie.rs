//! Cookie parameter extraction

use async_trait::async_trait;
use reinhardt_http::Request;
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{
	ParamContext, ParamError, ParamErrorContext, ParamResult, ParamType, extract::FromRequest,
};

/// Extract a value from cookies
pub struct Cookie<T>(pub T);

impl<T> Cookie<T> {
	/// Unwrap the Cookie and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::Cookie;
	///
	/// let cookie = Cookie(String::from("session_token_123"));
	/// let inner = cookie.into_inner();
	/// assert_eq!(inner, "session_token_123");
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for Cookie<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for Cookie<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

/// CookieStruct extracts multiple cookies into a struct
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_di::params::CookieStruct;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct MyCookies {
///     session_id: String,
///     user_id: Option<String>,
/// }
///
/// async fn handler(cookies: CookieStruct<MyCookies>) {
///     let session = &cookies.session_id;
/// }
/// ```
pub struct CookieStruct<T>(pub T);

impl<T> CookieStruct<T> {
	/// Unwrap the CookieStruct and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::CookieStruct;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct MyCookies {
	///     session_id: String,
	///     user_id: String,
	/// }
	///
	/// let cookies = CookieStruct(MyCookies {
	///     session_id: "abc123".to_string(),
	///     user_id: "user456".to_string(),
	/// });
	/// let inner = cookies.into_inner();
	/// assert_eq!(inner.session_id, "abc123");
	/// assert_eq!(inner.user_id, "user456");
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for CookieStruct<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for CookieStruct<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

// Note: No internal trait; use concrete impls for T and Option<T> via FromRequest

use super::cookie_util::parse_cookies;

#[async_trait]
impl<T> FromRequest for CookieStruct<T>
where
	T: DeserializeOwned + Send,
{
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		// Extract cookies from the Cookie header
		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);

		// Convert to JSON for deserialization
		let json_value = serde_json::to_value(&cookies_map).map_err(|e| {
			ParamError::InvalidParameter(Box::new(
				ParamErrorContext::new(ParamType::Cookie, e.to_string()).with_field("cookies"),
			))
		})?;

		serde_json::from_value(json_value)
			.map(CookieStruct)
			.map_err(|e| {
				ParamError::InvalidParameter(Box::new(
					ParamErrorContext::new(ParamType::Cookie, e.to_string()).with_field("cookies"),
				))
			})
	}
}

#[async_trait]
impl FromRequest for Cookie<String> {
	async fn from_request(req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
		let name = ctx.get_cookie_name::<String>().ok_or_else(|| {
			ParamError::MissingParameter(
				"Cookie name not specified in ParamContext for this type".to_string(),
			)
		})?;

		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		let value = cookies_map
			.get(name)
			.ok_or_else(|| ParamError::MissingParameter(name.to_string()))?;
		Ok(Cookie(value.to_string()))
	}
}

#[async_trait]
impl FromRequest for Cookie<Option<String>> {
	async fn from_request(req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
		let name = match ctx.get_cookie_name::<String>() {
			Some(n) => n,
			None => return Ok(Cookie(None)),
		};

		let cookie_header = req
			.headers
			.get(http::header::COOKIE)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("");

		let cookies_map = parse_cookies(cookie_header);
		Ok(Cookie(cookies_map.get(name).map(|s| s.to_string())))
	}
}

// Implement WithValidation trait for Cookie and CookieStruct
#[cfg(feature = "validation")]
impl<T> super::validation::WithValidation for Cookie<T> {}

#[cfg(feature = "validation")]
impl<T> super::validation::WithValidation for CookieStruct<T> {}
