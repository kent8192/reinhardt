//! Broken link detection middleware
//!
//! Detects and logs 404 errors that originate from internal links (same domain).
//! Useful for identifying broken links on your site before users encounter them.
//!
//! ## Email Notifications
//!
//! This middleware can send email notifications to managers when broken links are detected.
//! Managers are loaded from `Settings::managers` (via `REINHARDT_SETTINGS` environment variable),
//! or from the `BrokenLinkConfig::email_addresses` if settings are not available.

use async_trait::async_trait;
use hyper::StatusCode;
use hyper::header::{REFERER, USER_AGENT};
use regex::Regex;
use reinhardt_conf::settings;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use reinhardt_mail;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for broken link detection
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenLinkConfig {
	/// Enable or disable broken link detection
	pub enabled: bool,
	/// Email addresses to notify (if configured)
	pub email_addresses: Vec<String>,
	/// Path patterns to ignore (regex)
	pub ignored_paths: Vec<String>,
	/// User-Agent patterns to ignore (e.g., bots)
	pub ignored_user_agents: Vec<String>,
}

impl BrokenLinkConfig {
	/// Create a new default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::BrokenLinkConfig;
	///
	/// let config = BrokenLinkConfig::new();
	/// assert!(config.enabled);
	/// ```
	pub fn new() -> Self {
		Self {
			enabled: true,
			email_addresses: Vec::new(),
			ignored_paths: vec![
				// Common paths to ignore
				"/favicon.ico".to_string(),
				"/robots.txt".to_string(),
				"/.well-known/.*".to_string(),
			],
			ignored_user_agents: vec![
				// Common bots/crawlers to ignore
				"bot".to_string(),
				"crawler".to_string(),
				"spider".to_string(),
				"slurp".to_string(),
			],
		}
	}

	/// Disable broken link detection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::BrokenLinkConfig;
	///
	/// let config = BrokenLinkConfig::new().disabled();
	/// assert!(!config.enabled);
	/// ```
	pub fn disabled(mut self) -> Self {
		self.enabled = false;
		self
	}

	/// Add email addresses for notifications
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::BrokenLinkConfig;
	///
	/// let config = BrokenLinkConfig::new()
	///     .with_emails(vec!["admin@example.com".to_string()]);
	/// ```
	pub fn with_emails(mut self, emails: Vec<String>) -> Self {
		self.email_addresses = emails;
		self
	}

	/// Add additional paths to ignore
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::BrokenLinkConfig;
	///
	/// let config = BrokenLinkConfig::new()
	///     .with_ignored_paths(vec!["/admin/.*".to_string()]);
	/// ```
	pub fn with_ignored_paths(mut self, paths: Vec<String>) -> Self {
		self.ignored_paths.extend(paths);
		self
	}

	/// Add additional user agents to ignore
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::BrokenLinkConfig;
	///
	/// let config = BrokenLinkConfig::new()
	///     .with_ignored_user_agents(vec!["CustomBot".to_string()]);
	/// ```
	pub fn with_ignored_user_agents(mut self, user_agents: Vec<String>) -> Self {
		self.ignored_user_agents.extend(user_agents);
		self
	}
}

impl Default for BrokenLinkConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Middleware for detecting broken internal links
///
/// Logs 404 errors that originate from internal referrers (same domain).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{BrokenLinkEmailsMiddleware, BrokenLinkConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct NotFoundHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for NotFoundHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::NOT_FOUND))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = BrokenLinkConfig::new();
/// let middleware = BrokenLinkEmailsMiddleware::new(config);
/// let handler = Arc::new(NotFoundHandler);
///
/// let mut headers = HeaderMap::new();
/// headers.insert(hyper::header::REFERER, "http://example.com/page".parse().unwrap());
/// headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/missing")
///     .version(Version::HTTP_11)
///     .headers(headers)
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::NOT_FOUND);
/// # });
/// ```
pub struct BrokenLinkEmailsMiddleware {
	config: BrokenLinkConfig,
	ignored_path_regexes: Vec<Regex>,
	ignored_ua_regexes: Vec<Regex>,
}

impl BrokenLinkEmailsMiddleware {
	/// Create a new BrokenLinkEmailsMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{BrokenLinkEmailsMiddleware, BrokenLinkConfig};
	///
	/// let config = BrokenLinkConfig::new();
	/// let middleware = BrokenLinkEmailsMiddleware::new(config);
	/// ```
	pub fn new(config: BrokenLinkConfig) -> Self {
		let ignored_path_regexes = config
			.ignored_paths
			.iter()
			.filter_map(|p| Regex::new(p).ok())
			.collect();

		let ignored_ua_regexes = config
			.ignored_user_agents
			.iter()
			.filter_map(|ua| Regex::new(&format!("(?i){}", ua)).ok())
			.collect();

		Self {
			config,
			ignored_path_regexes,
			ignored_ua_regexes,
		}
	}

	/// Check if the path should be ignored
	fn is_ignored_path(&self, path: &str) -> bool {
		self.ignored_path_regexes.iter().any(|re| re.is_match(path))
	}

	/// Check if the user agent should be ignored
	fn is_ignored_user_agent(&self, user_agent: &str) -> bool {
		self.ignored_ua_regexes
			.iter()
			.any(|re| re.is_match(user_agent))
	}

	/// Extract domain from URL
	fn extract_domain(url: &str) -> Option<String> {
		if let Ok(parsed) = url::Url::parse(url) {
			parsed.host_str().map(|h| h.to_string())
		} else {
			None
		}
	}

	/// Check if the referrer is from the same domain (internal link)
	fn is_internal_referrer(&self, referer: &str, host: &str) -> bool {
		if let Some(referer_domain) = Self::extract_domain(referer) {
			// Normalize domains (remove www. prefix for comparison)
			let normalized_referer = referer_domain.trim_start_matches("www.");
			let normalized_host = host.trim_start_matches("www.");
			normalized_referer == normalized_host
		} else {
			false
		}
	}

	/// Log a broken link and send email notifications
	async fn log_broken_link(&self, path: &str, referer: &str) {
		// Log to standard logging system
		log::warn!("Broken link detected: {} (from: {})", path, referer);

		// Load Settings to get managers list
		// Try to read from default settings location
		// In production, this should be configured via environment or config file
		let managers = if let Ok(settings_json) = std::env::var("REINHARDT_SETTINGS") {
			// Attempt to parse settings from environment variable
			if let Ok(settings) = serde_json::from_str::<settings::Settings>(&settings_json) {
				settings.managers
			} else {
				Vec::new()
			}
		} else {
			// Fallback to config email_addresses if settings not available
			self.config
				.email_addresses
				.iter()
				.map(|email| settings::Contact::new("", email.clone()))
				.collect()
		};

		// Send email notifications to managers
		if !managers.is_empty() {
			let subject = format!("Broken link detected: {}", path);
			let body = format!(
				"A broken link was detected on your site:\n\n\
				 Broken URL: {}\n\
				 Referrer: {}\n\n\
				 Please check and fix this link.",
				path, referer
			);

			// Send to all managers asynchronously (non-blocking)
			for manager in &managers {
				let email = manager.email.clone();
				let subject_clone = subject.clone();
				let body_clone = body.clone();

				// Schedule email sending in a separate task to avoid blocking
				// Note: Uses default SMTP config (localhost:25). Configure via SmtpConfig for production.
				tokio::spawn(async move {
					let config = reinhardt_mail::SmtpConfig::default();
					let backend = match reinhardt_mail::SmtpBackend::new(config) {
						Ok(backend) => backend,
						Err(e) => {
							log::error!(
								"Failed to create SMTP backend for broken link email: {}",
								e
							);
							return;
						}
					};
					match reinhardt_mail::send_mail_with_backend(
						subject_clone,
						body_clone,
						"noreply@example.com", // Default sender
						vec![email.clone()],
						None,
						&backend,
					)
					.await
					{
						Ok(_) => {
							log::info!("Broken link email notification sent to: {}", email);
						}
						Err(e) => {
							log::error!("Failed to send broken link email to {}: {}", email, e);
						}
					}
				});
			}
		}
	}
}

impl Default for BrokenLinkEmailsMiddleware {
	fn default() -> Self {
		Self::new(BrokenLinkConfig::default())
	}
}

#[async_trait]
impl Middleware for BrokenLinkEmailsMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Extract necessary information before moving request
		let path = request.uri.path().to_string();
		let referer = request
			.headers
			.get(REFERER)
			.and_then(|r| r.to_str().ok())
			.map(|s| s.to_string());
		let host = request
			.headers
			.get(hyper::header::HOST)
			.and_then(|h| h.to_str().ok())
			.map(|s| s.to_string());
		let user_agent = request
			.headers
			.get(USER_AGENT)
			.and_then(|ua| ua.to_str().ok())
			.map(|s| s.to_string());

		// Call the handler
		let response = handler.handle(request).await?;

		// Check if we should process this request/response
		if !self.config.enabled || response.status != StatusCode::NOT_FOUND {
			return Ok(response);
		}

		// Check if path should be ignored
		if self.is_ignored_path(&path) {
			return Ok(response);
		}

		// Check if user agent should be ignored
		if let Some(ua) = user_agent
			&& self.is_ignored_user_agent(&ua)
		{
			return Ok(response);
		}

		// Check if there's a referrer and host
		if let (Some(referer_str), Some(host_str)) = (referer, host) {
			// Only log if it's an internal referrer
			if self.is_internal_referrer(&referer_str, &host_str) {
				self.log_broken_link(&path, &referer_str).await;
			}
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct NotFoundHandler;

	#[async_trait]
	impl Handler for NotFoundHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::NOT_FOUND))
		}
	}

	struct OkHandler;

	#[async_trait]
	impl Handler for OkHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_internal_404_detected() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// In a real scenario, we'd check logs or email was sent
	}

	#[tokio::test]
	async fn test_external_404_ignored() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://external.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// External referrer should not trigger detection
	}

	#[tokio::test]
	async fn test_no_referrer_ignored() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// No referrer should not trigger detection
	}

	#[tokio::test]
	async fn test_ignored_path() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/favicon.ico")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// favicon.ico is in ignored paths
	}

	#[tokio::test]
	async fn test_ignored_user_agent() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
		headers.insert(USER_AGENT, "Googlebot/2.1".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// Bot user agents should be ignored
	}

	#[tokio::test]
	async fn test_200_response_ignored() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(OkHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/existing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		// 200 responses should not trigger detection
	}

	#[tokio::test]
	async fn test_www_subdomain_handling() {
		let config = BrokenLinkConfig::new();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://www.example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// www.example.com should be treated as same domain as example.com
	}

	#[tokio::test]
	async fn test_disabled_config() {
		let config = BrokenLinkConfig::new().disabled();
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// Disabled config should not trigger detection
	}

	#[tokio::test]
	async fn test_custom_ignored_paths() {
		let config = BrokenLinkConfig::new().with_ignored_paths(vec!["/admin/.*".to_string()]);
		let middleware = BrokenLinkEmailsMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REFERER, "http://example.com/page".parse().unwrap());
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/missing")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		// Custom ignored paths should work
	}

	#[tokio::test]
	async fn test_email_configuration() {
		let config = BrokenLinkConfig::new().with_emails(vec!["admin@example.com".to_string()]);
		let middleware = BrokenLinkEmailsMiddleware::new(config);

		assert_eq!(middleware.config.email_addresses.len(), 1);
		assert_eq!(middleware.config.email_addresses[0], "admin@example.com");
	}
}
