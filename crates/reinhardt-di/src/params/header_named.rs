//! Named header parameter extraction
//!
//! Provides compile-time header name specification using marker types.

use async_trait::async_trait;
use reinhardt_http::Request;
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::ops::Deref;

use super::{ParamContext, ParamError, ParamResult, extract::FromRequest};

/// Marker trait for header names.
pub trait HeaderName {
	/// The name of the HTTP header to extract.
	const NAME: &'static str;
}

/// Marker type for Authorization header
pub struct Authorization;
impl HeaderName for Authorization {
	const NAME: &'static str = "Authorization";
}

/// Marker type for Content-Type header
pub struct ContentType;
impl HeaderName for ContentType {
	const NAME: &'static str = "Content-Type";
}

/// Extract a value from request headers with compile-time name specification
///
/// Unlike `Header<T>` which requires runtime ParamContext configuration,
/// `HeaderNamed` specifies the header name at compile time using marker types.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_di::params::{HeaderNamed, Authorization, ContentType};
/// async fn handler(auth: HeaderNamed<Authorization, String>) {
///     println!("Authorization: {}", *auth);
/// }
///
/// async fn handler_optional(ct: HeaderNamed<ContentType, Option<String>>) {
///     if let Some(t) = ct.into_inner() {
///         println!("Content-Type: {}", t);
///     }
/// }
/// ```
pub struct HeaderNamed<N: HeaderName, T> {
	value: T,
	_phantom: PhantomData<N>,
}

impl<N: HeaderName, T> HeaderNamed<N, T> {
	/// Unwrap the HeaderNamed and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{HeaderNamed, ContentType};
	///
	/// let header = HeaderNamed::<ContentType, String>::new("application/json".to_string());
	/// let inner = header.into_inner();
	/// assert_eq!(inner, "application/json");
	/// ```
	pub fn into_inner(self) -> T {
		self.value
	}

	/// Create a new HeaderNamed with a value
	///
	/// This is useful for testing or manual construction.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{HeaderNamed, Authorization};
	///
	/// let header = HeaderNamed::<Authorization, String>::new("Bearer token123".to_string());
	/// assert_eq!(*header, "Bearer token123");
	/// ```
	pub fn new(value: T) -> Self {
		HeaderNamed {
			value,
			_phantom: PhantomData,
		}
	}

	/// Get the header name as a compile-time constant
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::{HeaderNamed, ContentType};
	///
	/// let header = HeaderNamed::<ContentType, String>::new("text/html".to_string());
	/// assert_eq!(HeaderNamed::<ContentType, String>::name(), "Content-Type");
	/// ```
	pub const fn name() -> &'static str {
		N::NAME
	}
}

impl<N: HeaderName, T> Deref for HeaderNamed<N, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<N: HeaderName, T: Debug> Debug for HeaderNamed<N, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("HeaderNamed")
			.field("name", &N::NAME)
			.field("value", &self.value)
			.finish()
	}
}

#[async_trait]
impl FromRequest for HeaderNamed<ContentType, String> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let value = req
			.headers
			.get("content-type")
			.and_then(|v| v.to_str().ok())
			.ok_or_else(|| ParamError::MissingParameter("Content-Type".to_string()))?;

		Ok(HeaderNamed::new(value.to_string()))
	}
}

#[async_trait]
impl FromRequest for HeaderNamed<Authorization, String> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let value = req
			.headers
			.get("authorization")
			.and_then(|v| v.to_str().ok())
			.ok_or_else(|| ParamError::MissingParameter("Authorization".to_string()))?;

		Ok(HeaderNamed::new(value.to_string()))
	}
}

#[async_trait]
impl FromRequest for HeaderNamed<ContentType, Option<String>> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let maybe = req
			.headers
			.get("content-type")
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		Ok(HeaderNamed::new(maybe))
	}
}

#[async_trait]
impl FromRequest for HeaderNamed<Authorization, Option<String>> {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		let maybe = req
			.headers
			.get("authorization")
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		Ok(HeaderNamed::new(maybe))
	}
}

// Implement WithValidation trait for HeaderNamed
#[cfg(feature = "validation")]
impl<N: HeaderName, T> super::validation::WithValidation for HeaderNamed<N, T> {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_header_named_new() {
		let header = HeaderNamed::<ContentType, String>::new("test_value".to_string());
		assert_eq!(*header, "test_value");
		assert_eq!(HeaderNamed::<ContentType, String>::name(), "Content-Type");
	}

	#[test]
	fn test_header_named_into_inner() {
		let header = HeaderNamed::<Authorization, String>::new("Mozilla/5.0".to_string());
		let value = header.into_inner();
		assert_eq!(value, "Mozilla/5.0");
	}

	#[test]
	fn test_header_named_deref() {
		let header = HeaderNamed::<ContentType, String>::new("application/json".to_string());
		assert_eq!(&*header, "application/json");
	}

	#[test]
	fn test_header_named_optional() {
		let header1 = HeaderNamed::<Authorization, Option<String>>::new(Some("abc123".to_string()));
		assert_eq!(*header1, Some("abc123".to_string()));

		let header2 = HeaderNamed::<Authorization, Option<String>>::new(None);
		assert_eq!(*header2, None);
	}
}
