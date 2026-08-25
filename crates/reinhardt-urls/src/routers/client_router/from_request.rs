//! `FromRequest` integration for `ClientRouter::page` route handlers.
//!
//! Implements spec §4.3 of the Manouche DSL v2 design: a route page
//! function takes a single Props struct that implements [`FromRequest`].
//! The Props struct is constructed from a [`RouteContext`] (matched
//! path + captured path params + raw query string), with extractor
//! errors surfaced through [`ExtractError`].
//!
//! The same Props struct can also be used as a Component prop bag
//! (spec §4.3 last paragraph: "every page is a component").
//!
//! # Manual vs. derived
//!
//! `FromRequest` is implemented manually in v2. The
//! `#[derive(FromRequest)]` / `#[derive(PageProps)]` / `#[component]`
//! proc-macros that automate this boilerplate are deferred to
//! spec §10. See the tracking issue referenced from the PR.
//!
//! # Example
//!
//! ```no_run
//! use reinhardt_urls::routers::client_router::from_request::{
//!     ExtractError, FromRequest, PathParam, RouteContext,
//! };
//! use reinhardt_urls::routers::ClientRouter;
//!
//! struct UserPageProps {
//!     id: PathParam<i32>,
//! }
//!
//! impl FromRequest for UserPageProps {
//!     fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError> {
//!         Ok(Self {
//!             id: PathParam::extract(ctx, "id")?,
//!         })
//!     }
//! }
//!
//! fn user_page(props: UserPageProps) -> reinhardt_core::types::page::Page {
//!     reinhardt_core::types::page::Page::Text(
//!         format!("user {}", props.id.into_inner()).into(),
//!     )
//! }
//!
//! let _router = ClientRouter::new().page("user", "/users/{id}/", user_page);
//! ```

use std::collections::HashMap;
use std::str::FromStr;

/// The data a route handler receives from the router.
///
/// Wraps the data already produced by the client-router's pattern
/// matcher: the resolved path, the captured path parameters (by
/// name), and the raw query string (the portion after `?`, without
/// the leading `?`). Server-side rendering can construct the same
/// context shape.
#[derive(Debug, Clone)]
pub struct RouteContext {
	path: String,
	path_params: HashMap<String, String>,
	query: String,
}

impl RouteContext {
	/// Creates a new `RouteContext`.
	pub fn new(path: String, path_params: HashMap<String, String>, query: String) -> Self {
		Self {
			path,
			path_params,
			query,
		}
	}

	/// Returns the matched path (without the query string).
	pub fn path(&self) -> &str {
		&self.path
	}

	/// Returns the raw query string (without the leading `?`).
	pub fn query(&self) -> &str {
		&self.query
	}

	/// Looks up a named path parameter.
	///
	/// Returns `None` when the parameter was not captured by the route
	/// pattern. Returns an owned `String` so callers can decode /
	/// parse without borrowing the context.
	pub fn path_param(&self, name: &str) -> Option<String> {
		self.path_params.get(name).cloned()
	}

	/// Borrows the full path-parameter map.
	pub fn path_params(&self) -> &HashMap<String, String> {
		&self.path_params
	}
}

/// Errors surfaced when constructing a Props struct from a
/// [`RouteContext`].
///
/// Returned by [`FromRequest::from_request`] and the
/// `PathParam::extract` / `QueryParam::extract` helpers.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
	/// A required path parameter was not present in the match.
	#[error("missing path parameter `{0}`")]
	MissingPath(String),
	/// A required query parameter was not present in the query string.
	#[error("missing query parameter `{0}`")]
	MissingQuery(String),
	/// Parsing the raw string into the target type failed.
	#[error("failed to parse `{name}`: {source}")]
	Parse {
		/// The parameter name (path or query) being parsed.
		name: String,
		/// The underlying parse error.
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},
}

/// Trait implemented by Props structs used as route page handlers.
///
/// `ClientRouter::page<F, P>(pattern, handler)` constructs `P` via
/// `P::from_request(&ctx)` for every matched request, then calls
/// `handler(props)`.
pub trait FromRequest: Sized {
	/// Construct `Self` from the request context.
	///
	/// # Errors
	///
	/// Returns [`ExtractError`] when a required parameter is missing
	/// or fails to parse.
	fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError>;
}

/// Extractor for a single named path parameter, parsed via [`FromStr`].
///
/// Construct with [`PathParam::extract`] inside a `FromRequest` impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathParam<T>(T);

impl<T> PathParam<T> {
	/// Unwraps the inner value.
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> AsRef<T> for PathParam<T> {
	fn as_ref(&self) -> &T {
		&self.0
	}
}

impl<T> std::ops::Deref for PathParam<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T: FromStr> PathParam<T>
where
	T::Err: std::error::Error + Send + Sync + 'static,
{
	/// Extracts the path parameter `name` from `ctx` and parses it as `T`.
	///
	/// # Errors
	///
	/// - [`ExtractError::MissingPath`] when `name` is not in the match
	/// - [`ExtractError::Parse`] when `<T as FromStr>::from_str` fails
	pub fn extract(ctx: &RouteContext, name: &str) -> Result<Self, ExtractError> {
		let raw = ctx
			.path_param(name)
			.ok_or_else(|| ExtractError::MissingPath(name.to_string()))?;
		let parsed = T::from_str(&raw).map_err(|e| ExtractError::Parse {
			name: name.to_string(),
			source: Box::new(e),
		})?;
		Ok(PathParam(parsed))
	}
}

/// Extractor for a single named query parameter, parsed via [`FromStr`].
///
/// Construct with [`QueryParam::extract`] inside a `FromRequest` impl.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryParam<T>(T);

impl<T> QueryParam<T> {
	/// Unwraps the inner value.
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> AsRef<T> for QueryParam<T> {
	fn as_ref(&self) -> &T {
		&self.0
	}
}

impl<T> std::ops::Deref for QueryParam<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T: FromStr> QueryParam<T>
where
	T::Err: std::error::Error + Send + Sync + 'static,
{
	/// Extracts the query parameter `name` from `ctx` and parses it as `T`.
	///
	/// # Errors
	///
	/// - [`ExtractError::MissingQuery`] when `name` is not in the query string
	/// - [`ExtractError::Parse`] when `<T as FromStr>::from_str` fails
	pub fn extract(ctx: &RouteContext, name: &str) -> Result<Self, ExtractError> {
		let raw = parse_query(ctx.query(), name)
			.ok_or_else(|| ExtractError::MissingQuery(name.to_string()))?;
		let parsed = T::from_str(&raw).map_err(|e| ExtractError::Parse {
			name: name.to_string(),
			source: Box::new(e),
		})?;
		Ok(QueryParam(parsed))
	}
}

/// Extractor for an optional named query parameter, parsed via [`FromStr`]
/// when present.
///
/// Construct with [`OptionalQueryParam::extract`] inside a [`FromRequest`]
/// implementation. A missing key produces `None`; a present value that fails
/// to parse still produces [`ExtractError::Parse`].
///
/// ```ignore
/// use reinhardt_urls::routers::client_router::from_request::{
///     ExtractError, FromRequest, OptionalQueryParam, RouteContext,
/// };
///
/// struct DeploymentRequest {
///     logs: OptionalQueryParam<i64>,
/// }
///
/// impl FromRequest for DeploymentRequest {
///     fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError> {
///         Ok(Self {
///             logs: OptionalQueryParam::extract(ctx, "logs")?,
///         })
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OptionalQueryParam<T>(Option<T>);

impl<T> OptionalQueryParam<T> {
	/// Unwraps the optional inner value.
	pub fn into_inner(self) -> Option<T> {
		self.0
	}
}

impl<T> AsRef<Option<T>> for OptionalQueryParam<T> {
	fn as_ref(&self) -> &Option<T> {
		&self.0
	}
}

impl<T> std::ops::Deref for OptionalQueryParam<T> {
	type Target = Option<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: FromStr> OptionalQueryParam<T>
where
	T::Err: std::error::Error + Send + Sync + 'static,
{
	/// Extracts and parses query parameter `name` when it is present.
	///
	/// # Errors
	///
	/// Returns [`ExtractError::Parse`] when a present value fails to parse.
	pub fn extract(ctx: &RouteContext, name: &str) -> Result<Self, ExtractError> {
		let Some(raw) = parse_query(ctx.query(), name) else {
			return Ok(Self(None));
		};
		let parsed = T::from_str(&raw).map_err(|source| ExtractError::Parse {
			name: name.to_string(),
			source: Box::new(source),
		})?;
		Ok(Self(Some(parsed)))
	}
}

/// Minimal query-string parser: scans `k=v&k=v` pairs, returning
/// the percent-decoded value for the first match.
fn parse_query(query: &str, key: &str) -> Option<String> {
	for pair in query.split('&').filter(|p| !p.is_empty()) {
		let mut it = pair.splitn(2, '=');
		let k = it.next()?;
		let v = it.next().unwrap_or("");
		if k == key {
			return Some(url_decode(v));
		}
	}
	None
}

/// Minimal percent-decoder + `+` → space. Sufficient for typical
/// `application/x-www-form-urlencoded` query strings. Replace with
/// the `percent-encoding` crate if more robust handling becomes
/// necessary.
fn url_decode(s: &str) -> String {
	let bytes = s.as_bytes();
	let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
	let mut i = 0;
	while i < bytes.len() {
		match bytes[i] {
			b'+' => {
				out.push(b' ');
				i += 1;
			}
			b'%' if i + 2 < bytes.len() => {
				if let (Some(h), Some(l)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
					out.push(h * 16 + l);
					i += 3;
				} else {
					// Invalid escape — emit the `%` and continue scanning
					// from the next byte so trailing characters are
					// preserved literally.
					out.push(b'%');
					i += 1;
				}
			}
			b'%' => {
				// Trailing `%` with fewer than 2 hex digits — emit literal.
				out.push(b'%');
				i += 1;
			}
			b => {
				out.push(b);
				i += 1;
			}
		}
	}
	String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned())
}

fn hex_digit(b: u8) -> Option<u8> {
	match b {
		b'0'..=b'9' => Some(b - b'0'),
		b'a'..=b'f' => Some(b - b'a' + 10),
		b'A'..=b'F' => Some(b - b'A' + 10),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_query_finds_first_match() {
		assert_eq!(parse_query("a=1&b=2", "a"), Some("1".to_string()));
		assert_eq!(parse_query("a=1&b=2", "b"), Some("2".to_string()));
		assert_eq!(parse_query("a=1&b=2", "c"), None);
	}

	#[test]
	fn parse_query_handles_empty_value() {
		assert_eq!(parse_query("a=", "a"), Some("".to_string()));
	}

	#[test]
	fn url_decode_handles_percent_encoding() {
		assert_eq!(url_decode("hello%20world"), "hello world");
		assert_eq!(url_decode("a+b"), "a b");
		assert_eq!(url_decode("plain"), "plain");
	}

	#[test]
	fn url_decode_passes_through_invalid_percent() {
		// Stray % with non-hex following — keep the % literal.
		assert_eq!(url_decode("a%ZZ"), "a%ZZ");
	}
}
