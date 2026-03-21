//! Browsable API Middleware
//!
//! Provides middleware for automatically serving browsable HTML responses
//! when accessed from a web browser, similar to Django REST Framework.

use async_trait::async_trait;
use hyper::{Method, Uri};
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use std::sync::Arc;

use super::renderer::{ApiContext, BrowsableApiRenderer};

/// Middleware configuration for Browsable API
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BrowsableApiConfig {
	/// Enable browsable API (default: true)
	pub enabled: bool,
	/// Custom template name (optional)
	pub template_name: Option<String>,
	/// Custom CSS path (optional)
	pub custom_css: Option<String>,
}

impl Default for BrowsableApiConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			template_name: None,
			custom_css: None,
		}
	}
}

/// Middleware for serving browsable API HTML responses
///
/// This middleware automatically converts API responses to browsable HTML
/// when the request is from a web browser (based on Accept header).
pub struct BrowsableApiMiddleware {
	config: BrowsableApiConfig,
	renderer: BrowsableApiRenderer,
}

impl BrowsableApiMiddleware {
	/// Create a new BrowsableApiMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::browsable_api::middleware::BrowsableApiMiddleware;
	///
	/// let middleware = BrowsableApiMiddleware::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: BrowsableApiConfig::default(),
			renderer: BrowsableApiRenderer::new(),
		}
	}

	/// Create a new BrowsableApiMiddleware with custom configuration
	///
	/// # Arguments
	///
	/// * `config` - Custom configuration for the middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::browsable_api::middleware::{BrowsableApiMiddleware, BrowsableApiConfig};
	///
	/// let mut config = BrowsableApiConfig::default();
	/// config.enabled = true;
	/// config.template_name = Some("custom_api.tpl".to_string());
	/// config.custom_css = Some("/static/api.css".to_string());
	///
	/// let middleware = BrowsableApiMiddleware::with_config(config);
	/// ```
	pub fn with_config(config: BrowsableApiConfig) -> Self {
		Self {
			config,
			renderer: BrowsableApiRenderer::new(),
		}
	}

	/// Check if the request prefers HTML response
	fn prefers_html(request: &Request) -> bool {
		if let Some(accept) = request.headers.get("Accept")
			&& let Ok(accept_str) = accept.to_str()
		{
			// Check if Accept header contains text/html
			return accept_str.contains("text/html");
		}
		false
	}

	/// Check if the response is JSON
	fn is_json_response(response: &Response) -> bool {
		if let Some(content_type) = response.headers.get("content-type")
			&& let Ok(content_type_str) = content_type.to_str()
		{
			return content_type_str.contains("application/json");
		}
		false
	}

	/// Extract CSRF token from response Set-Cookie header.
	///
	/// Parses the Set-Cookie header to find the csrftoken cookie value,
	/// which is set by the CSRF middleware.
	fn extract_csrf_token(response: &Response) -> Option<String> {
		response
			.headers
			.get_all("set-cookie")
			.iter()
			.filter_map(|v| v.to_str().ok())
			.find_map(|cookie| {
				cookie.split(';').next().and_then(|kv| {
					let (name, value) = kv.trim().split_once('=')?;

					if name == "csrftoken" {
						Some(value.to_string())
					} else {
						None
					}
				})
			})
	}

	/// Convert JSON response to HTML with request info
	fn convert_to_html_with_info(
		&self,
		request_uri: &Uri,
		request_method: &Method,
		response: Response,
	) -> reinhardt_core::exception::Result<Response> {
		// Parse JSON response
		let json_body: serde_json::Value = serde_json::from_slice(&response.body).map_err(|e| {
			reinhardt_core::exception::Error::Other(anyhow::anyhow!("Failed to parse JSON: {}", e))
		})?;

		// Extract CSRF token from response cookies for form inclusion
		let csrf_token = Self::extract_csrf_token(&response);

		// Extract headers for display
		let headers: Vec<(String, String)> = response
			.headers
			.iter()
			.map(|(name, value)| {
				(
					name.to_string(),
					value.to_str().unwrap_or("<binary>").to_string(),
				)
			})
			.collect();

		// Build ApiContext
		let context = ApiContext {
			title: String::from("API Response"),
			description: None,
			endpoint: request_uri.path().to_string(),
			method: request_method.to_string().to_uppercase(),
			response_data: json_body,
			response_status: response.status.as_u16(),
			allowed_methods: vec!["GET".to_string()], // Default, should be extracted from response
			request_form: None,                       // Could be populated from OPTIONS response
			headers,
			csrf_token,
		};

		// Render HTML
		let html = self.renderer.render(&context).map_err(|e| {
			reinhardt_core::exception::Error::Other(anyhow::anyhow!("Failed to render HTML: {}", e))
		})?;

		// Create new response with HTML body, preserving Set-Cookie headers
		let mut html_response = Response::new(response.status)
			.with_body(html)
			.with_header("content-type", "text/html; charset=utf-8");

		// Copy Set-Cookie headers to HTML response so CSRF cookie is sent
		for value in response.headers.get_all("set-cookie").iter() {
			html_response.headers.append("set-cookie", value.clone());
		}

		Ok(html_response)
	}
}

impl Default for BrowsableApiMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for BrowsableApiMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// If disabled, just pass through
		if !self.config.enabled {
			return handler.handle(request).await;
		}

		let prefers_html = Self::prefers_html(&request);

		// Extract request info before moving request
		let request_uri = request.uri.clone();
		let request_method = request.method.clone();

		// Get response from handler
		let response = handler.handle(request).await?;

		// If client prefers HTML and response is JSON, convert to browsable HTML
		if prefers_html && Self::is_json_response(&response) {
			self.convert_to_html_with_info(&request_uri, &request_method, response)
		} else {
			Ok(response)
		}
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
			Ok(Response::new(StatusCode::OK)
				.with_body(Bytes::from(r#"{"data":"test"}"#))
				.with_header("content-type", "application/json"))
		}
	}

	#[tokio::test]
	async fn test_middleware_with_html_accept() {
		let middleware = BrowsableApiMiddleware::new();
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("Accept", "text/html".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Check that response was converted to HTML
		assert_eq!(
			response.headers.get("content-type").unwrap(),
			"text/html; charset=utf-8"
		);

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body.contains("<!DOCTYPE html>"), "Missing DOCTYPE");
		assert!(body.contains("API Response"), "Missing 'API Response'");
		// Tera autoescapes `/` as `&#x2F;` for XSS protection
		assert!(
			body.contains("&#x2F;api&#x2F;test"),
			"Missing '/api/test' (HTML-escaped)"
		);
		// The response data is rendered in pre-formatted JSON
		assert!(body.contains("data"), "Missing 'data' in body: {}", body);
		assert!(body.contains("test"), "Missing 'test' in body");
	}

	#[tokio::test]
	async fn test_middleware_with_json_accept() {
		let middleware = BrowsableApiMiddleware::new();
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("Accept", "application/json".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		// When requesting JSON, should return JSON unchanged
		assert_eq!(
			response.headers.get("content-type").unwrap(),
			"application/json"
		);
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, r#"{"data":"test"}"#);
	}

	#[tokio::test]
	async fn test_middleware_disabled() {
		let config = BrowsableApiConfig {
			enabled: false,
			template_name: None,
			custom_css: None,
		};
		let middleware = BrowsableApiMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("Accept", "text/html".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		// When disabled, should return JSON unchanged
		assert_eq!(
			response.headers.get("content-type").unwrap(),
			"application/json"
		);
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, r#"{"data":"test"}"#);
	}

	#[tokio::test]
	async fn test_middleware_default() {
		let middleware = BrowsableApiMiddleware::default();
		assert!(middleware.config.enabled);
		assert!(middleware.config.template_name.is_none());
		assert!(middleware.config.custom_css.is_none());
	}

	#[tokio::test]
	async fn test_middleware_with_custom_config() {
		let config = BrowsableApiConfig {
			enabled: true,
			template_name: Some("custom.html".to_string()),
			custom_css: Some("/custom.css".to_string()),
		};
		let middleware = BrowsableApiMiddleware::with_config(config.clone());
		assert!(middleware.config.enabled);
		assert_eq!(
			middleware.config.template_name,
			Some("custom.html".to_string())
		);
		assert_eq!(
			middleware.config.custom_css,
			Some("/custom.css".to_string())
		);
	}

	#[tokio::test]
	async fn test_prefers_html_with_html_accept() {
		let mut headers = HeaderMap::new();
		headers.insert("Accept", "text/html".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(BrowsableApiMiddleware::prefers_html(&request));
	}

	#[tokio::test]
	async fn test_prefers_html_with_json_accept() {
		let mut headers = HeaderMap::new();
		headers.insert("Accept", "application/json".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(!BrowsableApiMiddleware::prefers_html(&request));
	}

	#[tokio::test]
	async fn test_prefers_html_without_accept_header() {
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		assert!(!BrowsableApiMiddleware::prefers_html(&request));
	}
}
