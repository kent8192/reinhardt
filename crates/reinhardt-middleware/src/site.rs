//! Site middleware
//!
//! Enables multi-site support by identifying and setting the current site
//! based on the incoming request's host header.

use async_trait::async_trait;
use hyper::header::HeaderName;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Site information
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
	/// Site ID
	pub id: u64,
	/// Domain name
	pub domain: String,
	/// Site name
	pub name: String,
}

impl Site {
	/// Create a new site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::Site;
	///
	/// let site = Site::new(1, "example.com".to_string(), "Example Site".to_string());
	/// assert_eq!(site.id, 1);
	/// assert_eq!(site.domain, "example.com");
	/// ```
	pub fn new(id: u64, domain: String, name: String) -> Self {
		Self { id, domain, name }
	}
}

/// Site registry for managing multiple sites
#[derive(Debug, Default)]
pub struct SiteRegistry {
	/// Sites indexed by domain
	sites: RwLock<HashMap<String, Site>>,
	/// Default site
	default_site: RwLock<Option<Site>>,
}

impl SiteRegistry {
	/// Create a new site registry
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{Site, SiteRegistry};
	///
	/// let registry = SiteRegistry::new();
	/// let site = Site::new(1, "example.com".to_string(), "Example".to_string());
	/// registry.register(site);
	/// ```
	pub fn register(&self, site: Site) {
		let domain = site.domain.clone();
		self.sites
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.insert(domain, site);
	}

	/// Set default site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{Site, SiteRegistry};
	///
	/// let registry = SiteRegistry::new();
	/// let site = Site::new(1, "example.com".to_string(), "Example".to_string());
	/// registry.set_default(site);
	/// ```
	pub fn set_default(&self, site: Site) {
		*self.default_site.write().unwrap_or_else(|e| e.into_inner()) = Some(site);
	}

	/// Get site by domain
	pub fn get_by_domain(&self, domain: &str) -> Option<Site> {
		// Try exact match first
		if let Some(site) = self
			.sites
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(domain)
		{
			return Some(site.clone());
		}

		// Try without www prefix
		if let Some(without_www) = domain.strip_prefix("www.")
			&& let Some(site) = self
				.sites
				.read()
				.unwrap_or_else(|e| e.into_inner())
				.get(without_www)
		{
			return Some(site.clone());
		}

		// Not found
		None
	}

	/// Get default site
	pub fn default_site(&self) -> Option<Site> {
		self.default_site
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.clone()
	}

	/// Get all registered sites
	pub fn all(&self) -> Vec<Site> {
		self.sites
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.values()
			.cloned()
			.collect()
	}

	/// Clear all sites
	pub fn clear(&self) {
		self.sites
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		*self.default_site.write().unwrap_or_else(|e| e.into_inner()) = None;
	}
}

/// Header name for current site ID
pub const SITE_ID_HEADER: &str = "X-Site-ID";

/// Configuration for site middleware
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SiteConfig {
	/// Enable site middleware
	pub enabled: bool,
	/// Fallback to default site if no match
	pub fallback_enabled: bool,
}

impl SiteConfig {
	/// Create a new default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::SiteConfig;
	///
	/// let config = SiteConfig::new();
	/// assert!(config.enabled);
	/// ```
	pub fn new() -> Self {
		Self {
			enabled: true,
			fallback_enabled: true,
		}
	}

	/// Disable fallback to default site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::SiteConfig;
	///
	/// let config = SiteConfig::new().without_fallback();
	/// assert!(!config.fallback_enabled);
	/// ```
	pub fn without_fallback(mut self) -> Self {
		self.fallback_enabled = false;
		self
	}

	/// Disable site middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::SiteConfig;
	///
	/// let config = SiteConfig::new().disabled();
	/// assert!(!config.enabled);
	/// ```
	pub fn disabled(mut self) -> Self {
		self.enabled = false;
		self
	}
}

impl Default for SiteConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Middleware for multi-site support
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{Site, SiteConfig, SiteMiddleware};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = SiteConfig::new();
/// let middleware = SiteMiddleware::new(config);
///
/// // Register a site
/// let site = Site::new(1, "example.com".to_string(), "Example Site".to_string());
/// middleware.registry.register(site);
///
/// let handler = Arc::new(TestHandler);
/// let mut headers = HeaderMap::new();
/// headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/test")
///     .version(Version::HTTP_11)
///     .headers(headers)
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert!(response.headers.contains_key("X-Site-ID"));
/// # });
/// ```
pub struct SiteMiddleware {
	config: SiteConfig,
	pub registry: Arc<SiteRegistry>,
}

impl SiteMiddleware {
	/// Create a new SiteMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{SiteConfig, SiteMiddleware};
	///
	/// let config = SiteConfig::new();
	/// let middleware = SiteMiddleware::new(config);
	/// ```
	pub fn new(config: SiteConfig) -> Self {
		Self {
			config,
			registry: Arc::new(SiteRegistry::new()),
		}
	}

	/// Create a new SiteMiddleware with default configuration
	pub fn with_defaults() -> Self {
		Self::new(SiteConfig::default())
	}

	/// Get host from request
	fn get_host(&self, request: &Request) -> Option<String> {
		request
			.headers
			.get(hyper::header::HOST)
			.and_then(|h| h.to_str().ok())
			.map(|s| {
				// Remove port if present
				s.split(':').next().unwrap_or(s).to_string()
			})
	}
}

impl Default for SiteMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for SiteMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Skip if disabled
		if !self.config.enabled {
			return handler.handle(request).await;
		}

		// Get host from request
		let host = match self.get_host(&request) {
			Some(h) => h,
			None => {
				// No host header, use default site if fallback enabled
				if self.config.fallback_enabled {
					let default_site = self
						.registry
						.default_site
						.read()
						.unwrap_or_else(|e| e.into_inner())
						.clone();
					if let Some(site) = default_site {
						let mut response = handler.handle(request).await?;
						let header_name: HeaderName = SITE_ID_HEADER.parse().unwrap();
						response
							.headers
							.insert(header_name, site.id.to_string().parse().unwrap());
						return Ok(response);
					}
				}
				return handler.handle(request).await;
			}
		};

		// Get site for this host
		let mut site = self.registry.get_by_domain(&host);

		// If no site found, try default site if fallback enabled
		if site.is_none() && self.config.fallback_enabled {
			site = self.registry.default_site();
		}

		// Call handler
		let mut response = handler.handle(request).await?;

		// Add site ID to response headers if site was found
		if let Some(site) = site {
			let header_name: HeaderName = SITE_ID_HEADER.parse().unwrap();
			response
				.headers
				.insert(header_name, site.id.to_string().parse().unwrap());
		}

		Ok(response)
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
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_site_detection() {
		let config = SiteConfig::new();
		let middleware = SiteMiddleware::new(config);

		// Register a site
		let site = Site::new(1, "example.com".to_string(), "Example Site".to_string());
		middleware.registry.register(site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should have site ID header
		assert!(response.headers.contains_key(SITE_ID_HEADER));
		assert_eq!(response.headers.get(SITE_ID_HEADER).unwrap(), "1");
	}

	#[tokio::test]
	async fn test_www_subdomain_handling() {
		let config = SiteConfig::new();
		let middleware = SiteMiddleware::new(config);

		// Register site without www
		let site = Site::new(1, "example.com".to_string(), "Example".to_string());
		middleware.registry.register(site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "www.example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should match site despite www prefix
		assert_eq!(response.headers.get(SITE_ID_HEADER).unwrap(), "1");
	}

	#[tokio::test]
	async fn test_default_site_fallback() {
		let config = SiteConfig::new();
		let middleware = SiteMiddleware::new(config);

		// Set default site
		let default_site = Site::new(99, "default.com".to_string(), "Default".to_string());
		middleware.registry.set_default(default_site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "unknown.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should use default site
		assert_eq!(response.headers.get(SITE_ID_HEADER).unwrap(), "99");
	}

	#[tokio::test]
	async fn test_no_fallback() {
		let config = SiteConfig::new().without_fallback();
		let middleware = SiteMiddleware::new(config);

		// Set default site
		let default_site = Site::new(99, "default.com".to_string(), "Default".to_string());
		middleware.registry.set_default(default_site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "unknown.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should NOT use default site when fallback disabled
		assert!(!response.headers.contains_key(SITE_ID_HEADER));
	}

	#[tokio::test]
	async fn test_multiple_sites() {
		let config = SiteConfig::new();
		let middleware = Arc::new(SiteMiddleware::new(config));

		// Register multiple sites
		middleware
			.registry
			.register(Site::new(1, "site1.com".to_string(), "Site 1".to_string()));
		middleware
			.registry
			.register(Site::new(2, "site2.com".to_string(), "Site 2".to_string()));

		let handler = Arc::new(TestHandler);

		// Test site 1
		let mut headers1 = HeaderMap::new();
		headers1.insert(hyper::header::HOST, "site1.com".parse().unwrap());
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers1)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.headers.get(SITE_ID_HEADER).unwrap(), "1");

		// Test site 2
		let mut headers2 = HeaderMap::new();
		headers2.insert(hyper::header::HOST, "site2.com".parse().unwrap());
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers2)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();
		assert_eq!(response2.headers.get(SITE_ID_HEADER).unwrap(), "2");
	}

	#[tokio::test]
	async fn test_disabled_middleware() {
		let config = SiteConfig::new().disabled();
		let middleware = SiteMiddleware::new(config);

		let site = Site::new(1, "example.com".to_string(), "Example".to_string());
		middleware.registry.register(site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not add site ID when disabled
		assert!(!response.headers.contains_key(SITE_ID_HEADER));
	}

	#[tokio::test]
	async fn test_port_handling() {
		let config = SiteConfig::new();
		let middleware = SiteMiddleware::new(config);

		let site = Site::new(1, "example.com".to_string(), "Example".to_string());
		middleware.registry.register(site);

		let handler = Arc::new(TestHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com:8080".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should match site despite port in host header
		assert_eq!(response.headers.get(SITE_ID_HEADER).unwrap(), "1");
	}

	#[tokio::test]
	async fn test_no_host_header() {
		let config = SiteConfig::new();
		let middleware = SiteMiddleware::new(config);

		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not crash without host header
		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_all_sites() {
		let registry = SiteRegistry::new();

		registry.register(Site::new(1, "site1.com".to_string(), "Site 1".to_string()));
		registry.register(Site::new(2, "site2.com".to_string(), "Site 2".to_string()));

		let sites = registry.all();
		assert_eq!(sites.len(), 2);
	}

	#[tokio::test]
	async fn test_clear_registry() {
		let registry = SiteRegistry::new();

		registry.register(Site::new(1, "site1.com".to_string(), "Site 1".to_string()));
		registry.set_default(Site::new(
			99,
			"default.com".to_string(),
			"Default".to_string(),
		));

		registry.clear();

		assert_eq!(registry.all().len(), 0);
		assert!(registry.default_site.read().unwrap().is_none());
	}

	#[tokio::test]
	async fn test_default_middleware() {
		let middleware = SiteMiddleware::default();
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);
	}
}
