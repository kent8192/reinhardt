mod body;
mod methods;
mod params;

use crate::extensions::Extensions;
use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri, Version};
#[cfg(feature = "parsers")]
use reinhardt_core::parsers::parser::{ParsedData, Parser};
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Configuration for trusted proxy IPs.
///
/// Only proxy headers (X-Forwarded-For, X-Real-IP, X-Forwarded-Proto) from
/// these IP addresses will be trusted. By default, no proxies are trusted
/// and the actual connection information is used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies {
	/// Set of trusted proxy IP addresses.
	/// Only requests originating from these IPs will have their proxy headers honored.
	trusted_ips: HashSet<IpAddr>,
}

impl TrustedProxies {
	/// Create with no trusted proxies (default, most secure).
	pub fn none() -> Self {
		Self {
			trusted_ips: HashSet::new(),
		}
	}

	/// Create with a set of trusted proxy IPs.
	pub fn new(ips: impl IntoIterator<Item = IpAddr>) -> Self {
		Self {
			trusted_ips: ips.into_iter().collect(),
		}
	}

	/// Check if the given address is a trusted proxy.
	pub fn is_trusted(&self, addr: &IpAddr) -> bool {
		self.trusted_ips.contains(addr)
	}

	/// Check if any proxies are configured.
	pub fn has_trusted_proxies(&self) -> bool {
		!self.trusted_ips.is_empty()
	}
}

/// HTTP Request representation
pub struct Request {
	pub method: Method,
	pub uri: Uri,
	pub version: Version,
	pub headers: HeaderMap,
	body: Bytes,
	pub path_params: HashMap<String, String>,
	pub query_params: HashMap<String, String>,
	/// Indicates if this request came over HTTPS
	pub is_secure: bool,
	/// Remote address of the client (if available)
	pub remote_addr: Option<SocketAddr>,
	/// Parsers for request body
	#[cfg(feature = "parsers")]
	parsers: Vec<Box<dyn Parser>>,
	/// Cached parsed data (lazy parsing)
	#[cfg(feature = "parsers")]
	parsed_data: Arc<Mutex<Option<ParsedData>>>,
	/// Whether the body has been consumed
	body_consumed: Arc<AtomicBool>,
	/// Extensions for storing arbitrary typed data
	pub extensions: Extensions,
}

/// Builder for constructing `Request` instances.
///
/// Provides a fluent API for building HTTP requests with optional parameters.
///
/// # Examples
///
/// ```
/// use reinhardt_http::Request;
/// use hyper::Method;
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/users?page=1")
///     .build()
///     .unwrap();
///
/// assert_eq!(request.method, Method::GET);
/// assert_eq!(request.path(), "/api/users");
/// assert_eq!(request.query_params.get("page"), Some(&"1".to_string()));
/// ```
pub struct RequestBuilder {
	method: Method,
	uri: Option<Uri>,
	version: Version,
	headers: HeaderMap,
	body: Bytes,
	is_secure: bool,
	remote_addr: Option<SocketAddr>,
	path_params: HashMap<String, String>,
	/// Captured error from invalid URI
	uri_error: Option<String>,
	/// Captured error from invalid header value
	header_error: Option<String>,
	#[cfg(feature = "parsers")]
	parsers: Vec<Box<dyn Parser>>,
}

impl Default for RequestBuilder {
	fn default() -> Self {
		Self {
			method: Method::GET,
			uri: None,
			version: Version::HTTP_11,
			headers: HeaderMap::new(),
			body: Bytes::new(),
			is_secure: false,
			remote_addr: None,
			path_params: HashMap::new(),
			uri_error: None,
			header_error: None,
			#[cfg(feature = "parsers")]
			parsers: Vec::new(),
		}
	}
}

impl RequestBuilder {
	/// Set the HTTP method.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/api/users")
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.method, Method::POST);
	/// ```
	pub fn method(mut self, method: Method) -> Self {
		self.method = method;
		self
	}

	/// Set the request URI.
	///
	/// Accepts either a `&str` or `Uri`. Query parameters will be automatically parsed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users?page=1&limit=10")
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.path(), "/api/users");
	/// assert_eq!(request.query_params.get("page"), Some(&"1".to_string()));
	/// assert_eq!(request.query_params.get("limit"), Some(&"10".to_string()));
	/// ```
	pub fn uri<T>(mut self, uri: T) -> Self
	where
		T: TryInto<Uri>,
		T::Error: std::fmt::Display,
	{
		match uri.try_into() {
			Ok(uri) => {
				self.uri = Some(uri);
			}
			Err(e) => {
				self.uri_error = Some(format!("Invalid URI: {}", e));
			}
		}
		self
	}

	/// Set the HTTP version.
	///
	/// Defaults to HTTP/1.1 if not specified.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version};
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users")
	///     .version(Version::HTTP_2)
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.version, Version::HTTP_2);
	/// ```
	pub fn version(mut self, version: Version) -> Self {
		self.version = version;
		self
	}

	/// Set the request headers.
	///
	/// Replaces all existing headers.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, HeaderMap, header};
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/api/users")
	///     .headers(headers.clone())
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.headers.get(header::CONTENT_TYPE).unwrap(), "application/json");
	/// ```
	pub fn headers(mut self, headers: HeaderMap) -> Self {
		self.headers = headers;
		self
	}

	/// Add a single header to the request.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, header};
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/api/users")
	///     .header(header::CONTENT_TYPE, "application/json")
	///     .header(header::AUTHORIZATION, "Bearer token123")
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.headers.get(header::CONTENT_TYPE).unwrap(), "application/json");
	/// assert_eq!(request.headers.get(header::AUTHORIZATION).unwrap(), "Bearer token123");
	/// ```
	pub fn header<K, V>(mut self, key: K, value: V) -> Self
	where
		K: hyper::header::IntoHeaderName,
		V: TryInto<hyper::header::HeaderValue>,
		V::Error: std::fmt::Display,
	{
		match value.try_into() {
			Ok(val) => {
				self.headers.insert(key, val);
			}
			Err(e) => {
				self.header_error = Some(format!("Invalid header value: {}", e));
			}
		}
		self
	}

	/// Set the request body.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	/// use bytes::Bytes;
	///
	/// let body = Bytes::from(r#"{"name":"Alice"}"#);
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/api/users")
	///     .body(body.clone())
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.body(), &body);
	/// ```
	pub fn body(mut self, body: Bytes) -> Self {
		self.body = body;
		self
	}

	/// Set whether the request is secure (HTTPS).
	///
	/// Defaults to `false` if not specified.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .secure(true)
	///     .build()
	///     .unwrap();
	///
	/// assert!(request.is_secure());
	/// assert_eq!(request.scheme(), "https");
	/// ```
	pub fn secure(mut self, is_secure: bool) -> Self {
		self.is_secure = is_secure;
		self
	}

	/// Set the remote address of the client.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
	///
	/// let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .remote_addr(addr)
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.remote_addr, Some(addr));
	/// ```
	pub fn remote_addr(mut self, addr: SocketAddr) -> Self {
		self.remote_addr = Some(addr);
		self
	}

	/// Add a parser to the request.
	///
	/// Parsers are used to parse the request body into specific formats.
	/// The parser will be boxed internally.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/api/users")
	///     .parser(JsonParser::new())
	///     .build()
	///     .unwrap();
	/// ```
	#[cfg(feature = "parsers")]
	pub fn parser<P: Parser + 'static>(mut self, parser: P) -> Self {
		self.parsers.push(Box::new(parser));
		self
	}

	/// Set path parameters (used for testing views without router).
	///
	/// This is primarily useful in test environments where you need to simulate
	/// path parameters that would normally be extracted by the router.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	/// use std::collections::HashMap;
	///
	/// let mut params = HashMap::new();
	/// params.insert("id".to_string(), "42".to_string());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users/42")
	///     .path_params(params)
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.path_params.get("id"), Some(&"42".to_string()));
	/// ```
	pub fn path_params(mut self, params: HashMap<String, String>) -> Self {
		self.path_params = params;
		self
	}

	/// Build the final `Request` instance.
	///
	/// Returns an error if the URI is missing.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users")
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.method, Method::GET);
	/// assert_eq!(request.path(), "/api/users");
	/// ```
	pub fn build(self) -> Result<Request, String> {
		// Report captured errors from builder methods
		if let Some(err) = self.uri_error {
			return Err(err);
		}
		if let Some(err) = self.header_error {
			return Err(err);
		}
		let uri = self.uri.ok_or_else(|| "URI is required".to_string())?;
		let query_params = Request::parse_query_params(&uri);

		Ok(Request {
			method: self.method,
			uri,
			version: self.version,
			headers: self.headers,
			body: self.body,
			path_params: self.path_params,
			query_params,
			is_secure: self.is_secure,
			remote_addr: self.remote_addr,
			#[cfg(feature = "parsers")]
			parsers: self.parsers,
			#[cfg(feature = "parsers")]
			parsed_data: Arc::new(Mutex::new(None)),
			body_consumed: Arc::new(AtomicBool::new(false)),
			extensions: Extensions::new(),
		})
	}
}

impl Request {
	/// Create a new `RequestBuilder`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users")
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(request.method, Method::GET);
	/// ```
	pub fn builder() -> RequestBuilder {
		RequestBuilder::default()
	}

	/// Set the DI context for this request (used by routers with dependency injection)
	///
	/// This method stores the DI context in the request's extensions,
	/// allowing handlers to access dependency injection services.
	///
	/// The context will be wrapped in an Arc internally for efficient sharing.
	/// The DI context type is generic to avoid circular dependencies.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// # struct DummyDiContext;
	/// let mut request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	///
	/// let di_ctx = DummyDiContext;
	/// request.set_di_context(di_ctx);
	/// ```
	pub fn set_di_context<T: Send + Sync + 'static>(&mut self, ctx: T) {
		self.extensions.insert(Arc::new(ctx));
	}

	/// Get the DI context from this request
	///
	/// Returns `None` if no DI context was set.
	///
	/// The DI context type is generic to avoid circular dependencies.
	/// Returns a reference to the context.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_http::Request;
	/// use hyper::Method;
	///
	/// # struct DummyDiContext;
	/// let mut request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	///
	/// let di_ctx = DummyDiContext;
	/// request.set_di_context(di_ctx);
	///
	/// let ctx = request.get_di_context::<DummyDiContext>();
	/// assert!(ctx.is_some());
	/// ```
	pub fn get_di_context<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
		self.extensions.get::<Arc<T>>()
	}

	/// Extract Bearer token from Authorization header
	///
	/// Extracts JWT or other bearer tokens from the Authorization header.
	/// Returns `None` if the header is missing or not in "Bearer `<token>`" format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap, header};
	/// use bytes::Bytes;
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(
	///     header::AUTHORIZATION,
	///     "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".parse().unwrap()
	/// );
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let token = request.extract_bearer_token();
	/// assert_eq!(token, Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".to_string()));
	/// ```
	///
	/// # Missing or invalid header
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let token = request.extract_bearer_token();
	/// assert_eq!(token, None);
	/// ```
	pub fn extract_bearer_token(&self) -> Option<String> {
		self.headers
			.get(hyper::header::AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(|auth_str| auth_str.strip_prefix("Bearer ").map(|s| s.to_string()))
	}

	/// Get a specific header value from the request
	///
	/// Returns `None` if the header is missing or cannot be converted to a string.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap, header};
	/// use bytes::Bytes;
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(
	///     header::USER_AGENT,
	///     "Mozilla/5.0".parse().unwrap()
	/// );
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let user_agent = request.get_header("user-agent");
	/// assert_eq!(user_agent, Some("Mozilla/5.0".to_string()));
	/// ```
	///
	/// # Missing header
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let header = request.get_header("x-custom-header");
	/// assert_eq!(header, None);
	/// ```
	pub fn get_header(&self, name: &str) -> Option<String> {
		self.headers
			.get(name)
			.and_then(|value| value.to_str().ok())
			.map(|s| s.to_string())
	}

	/// Extract client IP address from the request
	///
	/// Only trusts proxy headers (X-Forwarded-For, X-Real-IP) when the request
	/// originates from a configured trusted proxy. Without trusted proxies,
	/// falls back to the actual connection address.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::{Request, TrustedProxies};
	/// use hyper::{Method, Version, HeaderMap, header};
	/// use bytes::Bytes;
	/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
	///
	/// let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
	/// let mut headers = HeaderMap::new();
	/// headers.insert(
	///     header::HeaderName::from_static("x-forwarded-for"),
	///     "203.0.113.1, 198.51.100.1".parse().unwrap()
	/// );
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .remote_addr(SocketAddr::new(proxy_ip, 8080))
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// // Configure trusted proxies to honor X-Forwarded-For
	/// request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));
	///
	/// let ip = request.get_client_ip();
	/// assert_eq!(ip, Some("203.0.113.1".parse().unwrap()));
	/// ```
	///
	/// # No trusted proxy, fallback to remote_addr
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
	///
	/// let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .remote_addr(addr)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let ip = request.get_client_ip();
	/// assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
	/// ```
	pub fn get_client_ip(&self) -> Option<std::net::IpAddr> {
		// Only trust proxy headers if the request comes from a configured trusted proxy
		if self.is_from_trusted_proxy() {
			// Try X-Forwarded-For header first (common in proxy setups)
			if let Some(forwarded) = self.get_header("x-forwarded-for") {
				// X-Forwarded-For can contain multiple IPs, take the first one
				if let Some(first_ip) = forwarded.split(',').next()
					&& let Ok(ip) = first_ip.trim().parse()
				{
					return Some(ip);
				}
			}

			// Try X-Real-IP header
			if let Some(real_ip) = self.get_header("x-real-ip")
				&& let Ok(ip) = real_ip.parse()
			{
				return Some(ip);
			}
		}

		// Fallback to remote_addr (actual connection info)
		self.remote_addr.map(|addr| addr.ip())
	}

	/// Check if the request originates from a trusted proxy.
	///
	/// Returns `true` only if trusted proxies are configured AND the
	/// remote address is in the trusted set.
	fn is_from_trusted_proxy(&self) -> bool {
		if let Some(trusted) = self.extensions.get::<TrustedProxies>()
			&& let Some(addr) = self.remote_addr
		{
			return trusted.is_trusted(&addr.ip());
		}
		false
	}

	/// Set trusted proxy configuration for this request.
	///
	/// This is typically called by the server/middleware layer to configure
	/// which proxy IPs are trusted for header forwarding.
	pub fn set_trusted_proxies(&self, proxies: TrustedProxies) {
		self.extensions.insert(proxies);
	}

	/// Validate Content-Type header
	///
	/// Checks if the Content-Type header matches the expected value.
	/// Returns an error if the header is missing or doesn't match.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap, header};
	/// use bytes::Bytes;
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(
	///     header::CONTENT_TYPE,
	///     "application/json".parse().unwrap()
	/// );
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// assert!(request.validate_content_type("application/json").is_ok());
	/// ```
	///
	/// # Content-Type mismatch
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap, header};
	/// use bytes::Bytes;
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(
	///     header::CONTENT_TYPE,
	///     "text/plain".parse().unwrap()
	/// );
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let result = request.validate_content_type("application/json");
	/// assert!(result.is_err());
	/// ```
	///
	/// # Missing Content-Type header
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let result = request.validate_content_type("application/json");
	/// assert!(result.is_err());
	/// ```
	pub fn validate_content_type(&self, expected: &str) -> crate::Result<()> {
		match self.get_header("content-type") {
			Some(content_type) if content_type.starts_with(expected) => Ok(()),
			Some(content_type) => Err(crate::Error::Http(format!(
				"Invalid Content-Type: expected '{}', got '{}'",
				expected, content_type
			))),
			None => Err(crate::Error::Http(
				"Missing Content-Type header".to_string(),
			)),
		}
	}

	/// Parse query parameters into typed struct
	///
	/// Deserializes query string parameters into the specified type `T`.
	/// Returns an error if deserialization fails.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct Pagination {
	///     page: u32,
	///     limit: u32,
	/// }
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users?page=2&limit=10")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let params: Pagination = request.query_as().unwrap();
	/// assert_eq!(params, Pagination { page: 2, limit: 10 });
	/// ```
	///
	/// # Type mismatch error
	///
	/// ```
	/// use reinhardt_http::Request;
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize)]
	/// struct Pagination {
	///     page: u32,
	///     limit: u32,
	/// }
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/users?page=invalid")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let result: Result<Pagination, _> = request.query_as();
	/// assert!(result.is_err());
	/// ```
	pub fn query_as<T: serde::de::DeserializeOwned>(&self) -> crate::Result<T> {
		// Convert HashMap<String, String> to Vec<(String, String)> for serde_urlencoded
		let params: Vec<(String, String)> = self
			.query_params
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect();

		let encoded = serde_urlencoded::to_string(&params)
			.map_err(|e| crate::Error::Http(format!("Failed to encode query parameters: {}", e)))?;
		serde_urlencoded::from_str(&encoded)
			.map_err(|e| crate::Error::Http(format!("Failed to parse query parameters: {}", e)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version, header};
	use rstest::rstest;

	#[rstest]
	fn test_extract_bearer_token() {
		let mut headers = HeaderMap::new();
		headers.insert(
			header::AUTHORIZATION,
			"Bearer test_token_123".parse().unwrap(),
		);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let token = request.extract_bearer_token();
		assert_eq!(token, Some("test_token_123".to_string()));
	}

	#[rstest]
	fn test_extract_bearer_token_missing() {
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let token = request.extract_bearer_token();
		assert_eq!(token, None);
	}

	#[rstest]
	fn test_get_header() {
		let mut headers = HeaderMap::new();
		headers.insert(header::USER_AGENT, "TestClient/1.0".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let user_agent = request.get_header("user-agent");
		assert_eq!(user_agent, Some("TestClient/1.0".to_string()));
	}

	#[rstest]
	fn test_get_header_missing() {
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let header = request.get_header("x-custom-header");
		assert_eq!(header, None);
	}

	#[rstest]
	fn test_get_client_ip_forwarded_for_with_trusted_proxy() {
		// Arrange
		let proxy_ip: std::net::IpAddr = "10.0.0.254".parse().unwrap();
		let mut headers = HeaderMap::new();
		headers.insert(
			header::HeaderName::from_static("x-forwarded-for"),
			"192.168.1.1, 10.0.0.1".parse().unwrap(),
		);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.remote_addr(std::net::SocketAddr::new(proxy_ip, 8080))
			.build()
			.unwrap();

		// Configure trusted proxies
		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

		// Act & Assert
		let ip = request.get_client_ip();
		assert_eq!(ip, Some("192.168.1.1".parse().unwrap()));
	}

	#[rstest]
	fn test_get_client_ip_forwarded_for_without_trusted_proxy() {
		// Arrange - proxy headers present but no trusted proxy configured
		let mut headers = HeaderMap::new();
		headers.insert(
			header::HeaderName::from_static("x-forwarded-for"),
			"192.168.1.1, 10.0.0.1".parse().unwrap(),
		);

		let remote_ip: std::net::IpAddr = "10.0.0.254".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.remote_addr(std::net::SocketAddr::new(remote_ip, 8080))
			.build()
			.unwrap();

		// Act - no trusted proxies, should use remote_addr
		let ip = request.get_client_ip();
		assert_eq!(ip, Some(remote_ip));
	}

	#[rstest]
	fn test_get_client_ip_real_ip_with_trusted_proxy() {
		// Arrange
		let proxy_ip: std::net::IpAddr = "10.0.0.254".parse().unwrap();
		let mut headers = HeaderMap::new();
		headers.insert(
			header::HeaderName::from_static("x-real-ip"),
			"203.0.113.5".parse().unwrap(),
		);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.remote_addr(std::net::SocketAddr::new(proxy_ip, 8080))
			.build()
			.unwrap();

		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

		// Act & Assert
		let ip = request.get_client_ip();
		assert_eq!(ip, Some("203.0.113.5".parse().unwrap()));
	}

	#[rstest]
	fn test_get_client_ip_none() {
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let ip = request.get_client_ip();
		assert_eq!(ip, None);
	}

	#[rstest]
	fn test_validate_content_type_valid() {
		let mut headers = HeaderMap::new();
		headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(request.validate_content_type("application/json").is_ok());
	}

	#[rstest]
	fn test_validate_content_type_invalid() {
		let mut headers = HeaderMap::new();
		headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(request.validate_content_type("application/json").is_err());
	}

	#[rstest]
	fn test_validate_content_type_missing() {
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(request.validate_content_type("application/json").is_err());
	}
}
