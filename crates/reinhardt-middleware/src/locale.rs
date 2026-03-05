//! Locale detection middleware
//!
//! Detects and sets the user's preferred language/locale based on multiple sources:
//! - Accept-Language header
//! - Cookie value
//! - URL path prefix
//!
//! The detected locale is stored in a custom header for downstream handlers to use.

use async_trait::async_trait;
use hyper::header::{ACCEPT_LANGUAGE, COOKIE};
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Header name for passing detected locale to handlers
pub const LOCALE_HEADER: &str = "X-Locale";
/// Cookie name for locale preference
pub const LOCALE_COOKIE_NAME: &str = "django_language";

/// Locale middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleConfig {
	/// Default locale to use when none is detected
	pub default_locale: String,
	/// List of supported locales (e.g., ["en", "ja", "fr"])
	pub supported_locales: Vec<String>,
	/// Check URL path for locale prefix (e.g., /ja/page)
	pub check_url_path: bool,
	/// Cookie name for storing locale preference
	pub cookie_name: String,
}

impl LocaleConfig {
	/// Create a new LocaleConfig with default settings
	///
	/// Default configuration:
	/// - `default_locale`: "en"
	/// - `supported_locales`: ["en"]
	/// - `check_url_path`: false
	/// - `cookie_name`: "django_language"
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::locale::LocaleConfig;
	///
	/// let config = LocaleConfig::new();
	/// assert_eq!(config.default_locale, "en");
	/// ```
	pub fn new() -> Self {
		Self {
			default_locale: "en".to_string(),
			supported_locales: vec!["en".to_string()],
			check_url_path: false,
			cookie_name: LOCALE_COOKIE_NAME.to_string(),
		}
	}

	/// Create a new LocaleConfig with multiple supported locales
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::locale::LocaleConfig;
	///
	/// let config = LocaleConfig::with_locales(
	///     "en".to_string(),
	///     vec!["en".to_string(), "ja".to_string(), "fr".to_string()]
	/// );
	/// assert_eq!(config.supported_locales.len(), 3);
	/// ```
	pub fn with_locales(default: String, supported: Vec<String>) -> Self {
		Self {
			default_locale: default,
			supported_locales: supported,
			check_url_path: false,
			cookie_name: LOCALE_COOKIE_NAME.to_string(),
		}
	}
}

impl Default for LocaleConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Locale detection middleware
///
/// Detects the user's preferred locale from various sources and adds it
/// to the request headers for use by downstream handlers.
///
/// Detection order:
/// 1. URL path prefix (if enabled)
/// 2. Cookie value
/// 3. Accept-Language header
/// 4. Default locale
///
/// # Examples
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::sync::Arc;
/// use reinhardt_middleware::{LocaleMiddleware, locale::LocaleConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
///         // Access detected locale from header
///         let locale = request.headers.get("X-Locale")
///             .and_then(|h| h.to_str().ok().map(String::from))
///             .unwrap_or_else(|| "en".to_string());
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from(locale)))
///     }
/// }
///
/// let config = LocaleConfig::with_locales(
///     "en".to_string(),
///     vec!["en".to_string(), "ja".to_string(), "fr".to_string()]
/// );
///
/// let middleware = LocaleMiddleware::with_config(config);
/// let handler = Arc::new(TestHandler);
///
/// let mut headers = HeaderMap::new();
/// headers.insert(hyper::header::ACCEPT_LANGUAGE, "ja,en;q=0.9".parse().unwrap());
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/page")
///     .version(Version::HTTP_11)
///     .headers(headers)
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// let body = String::from_utf8(response.body.to_vec()).unwrap();
/// assert_eq!(body, "ja");
/// # Ok(())
/// # }
/// ```
pub struct LocaleMiddleware {
	config: LocaleConfig,
}

impl LocaleMiddleware {
	/// Create a new LocaleMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::LocaleMiddleware;
	///
	/// let middleware = LocaleMiddleware::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: LocaleConfig::default(),
		}
	}

	/// Create a new LocaleMiddleware with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{LocaleMiddleware, locale::LocaleConfig};
	///
	/// let config = LocaleConfig::with_locales(
	///     "en".to_string(),
	///     vec!["en".to_string(), "ja".to_string()]
	/// );
	///
	/// let middleware = LocaleMiddleware::with_config(config);
	/// ```
	pub fn with_config(config: LocaleConfig) -> Self {
		Self { config }
	}

	/// Extract locale from URL path (e.g., /ja/page -> "ja")
	fn locale_from_path(&self, path: &str) -> Option<String> {
		if !self.config.check_url_path {
			return None;
		}

		// Path format: /locale/... (e.g., /ja/page)
		let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
		if parts.is_empty() {
			return None;
		}

		let potential_locale = parts[0];
		if self
			.config
			.supported_locales
			.contains(&potential_locale.to_string())
		{
			return Some(potential_locale.to_string());
		}

		None
	}

	/// Extract locale from cookie
	fn locale_from_cookie(&self, request: &Request) -> Option<String> {
		let cookie_header = request.headers.get(COOKIE)?.to_str().ok()?;

		// Parse cookies: "name1=value1; name2=value2"
		for cookie in cookie_header.split(';') {
			let cookie = cookie.trim();
			if let Some((name, value)) = cookie.split_once('=')
				&& name == self.config.cookie_name
			{
				let locale = value.to_string();
				if self.config.supported_locales.contains(&locale) {
					return Some(locale);
				}
			}
		}

		None
	}

	/// Extract locale from Accept-Language header
	fn locale_from_accept_language(&self, request: &Request) -> Option<String> {
		let accept_lang = request.headers.get(ACCEPT_LANGUAGE)?.to_str().ok()?;

		// Parse Accept-Language: "ja,en-US;q=0.9,en;q=0.8"
		let mut languages: Vec<(String, f32)> = Vec::new();

		for lang_spec in accept_lang.split(',') {
			let lang_spec = lang_spec.trim();
			let (lang, quality) = if let Some((l, q)) = lang_spec.split_once(";q=") {
				(l.trim(), q.parse::<f32>().unwrap_or(1.0))
			} else {
				(lang_spec, 1.0)
			};

			// Extract base language code (ja-JP -> ja, en-US -> en)
			let base_lang = lang.split('-').next().unwrap_or(lang).to_string();
			languages.push((base_lang, quality));
		}

		// Sort by quality score (descending)
		languages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

		// Find first supported language
		languages
			.into_iter()
			.map(|(lang, _)| lang)
			.find(|lang| self.config.supported_locales.contains(lang))
	}

	/// Detect locale from all available sources
	fn detect_locale(&self, request: &Request) -> String {
		// Priority: URL path > Cookie > Accept-Language > Default
		self.locale_from_path(request.uri.path())
			.or_else(|| self.locale_from_cookie(request))
			.or_else(|| self.locale_from_accept_language(request))
			.unwrap_or_else(|| self.config.default_locale.clone())
	}
}

impl Default for LocaleMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for LocaleMiddleware {
	async fn process(&self, mut request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Detect locale
		let locale = self.detect_locale(&request);

		// Add locale to request headers for downstream handlers
		request
			.headers
			.insert(LOCALE_HEADER, locale.parse().unwrap());

		// Process request with handler
		handler.handle(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			// Echo back the detected locale
			let locale = request
				.headers
				.get(LOCALE_HEADER)
				.and_then(|h| h.to_str().ok())
				.unwrap_or("unknown")
				.to_string();
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from(locale)))
		}
	}

	#[tokio::test]
	async fn test_default_locale() {
		let config = LocaleConfig::new();
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "en");
	}

	#[tokio::test]
	async fn test_accept_language_detection() {
		let config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_LANGUAGE, "ja,en;q=0.9".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "ja");
	}

	#[tokio::test]
	async fn test_accept_language_with_quality() {
		let config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(
			ACCEPT_LANGUAGE,
			"fr;q=0.7,ja;q=0.9,en;q=0.8".parse().unwrap(),
		);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "ja"); // Highest quality score
	}

	#[tokio::test]
	async fn test_cookie_detection() {
		let config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(COOKIE, "django_language=fr; other=value".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "fr");
	}

	#[tokio::test]
	async fn test_cookie_overrides_accept_language() {
		let config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_LANGUAGE, "ja".parse().unwrap());
		headers.insert(COOKIE, "django_language=fr".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "fr"); // Cookie takes precedence
	}

	#[tokio::test]
	async fn test_url_path_detection() {
		let mut config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		config.check_url_path = true;

		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/ja/page/subpage")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "ja");
	}

	#[tokio::test]
	async fn test_url_path_overrides_all() {
		let mut config = LocaleConfig::with_locales(
			"en".to_string(),
			vec!["en".to_string(), "ja".to_string(), "fr".to_string()],
		);
		config.check_url_path = true;

		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_LANGUAGE, "ja".parse().unwrap());
		headers.insert(COOKIE, "django_language=fr".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/en/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "en"); // URL path takes highest precedence
	}

	#[tokio::test]
	async fn test_unsupported_locale_fallback() {
		let config =
			LocaleConfig::with_locales("en".to_string(), vec!["en".to_string(), "ja".to_string()]);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_LANGUAGE, "de,fr;q=0.9".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "en"); // Falls back to default
	}

	#[tokio::test]
	async fn test_accept_language_with_region() {
		let config =
			LocaleConfig::with_locales("en".to_string(), vec!["en".to_string(), "ja".to_string()]);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_LANGUAGE, "ja-JP,en-US;q=0.9".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "ja"); // Extracts base language code
	}

	#[tokio::test]
	async fn test_invalid_cookie_value() {
		let config =
			LocaleConfig::with_locales("en".to_string(), vec!["en".to_string(), "ja".to_string()]);
		let middleware = LocaleMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(COOKIE, "django_language=invalid".parse().unwrap());
		headers.insert(ACCEPT_LANGUAGE, "ja".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "ja"); // Falls back to Accept-Language
	}
}
