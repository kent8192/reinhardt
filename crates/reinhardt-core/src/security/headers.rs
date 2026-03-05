//! Security headers middleware

use super::utils::generate_token;

/// Content Security Policy configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSecurityPolicy {
	pub default_src: Vec<String>,
	pub script_src: Vec<String>,
	pub style_src: Vec<String>,
	pub img_src: Vec<String>,
	pub connect_src: Vec<String>,
	pub font_src: Vec<String>,
	pub object_src: Vec<String>,
	pub media_src: Vec<String>,
	pub frame_src: Vec<String>,
	/// URI to send CSP violation reports to
	pub report_uri: Option<String>,
	/// Destination for CSP violation reports (Reporting API)
	pub report_to: Option<String>,
	/// Whether to automatically generate nonce values
	pub auto_nonce: bool,
}

impl Default for ContentSecurityPolicy {
	fn default() -> Self {
		Self {
			default_src: vec!["'self'".to_string()],
			script_src: vec!["'self'".to_string()],
			style_src: vec!["'self'".to_string()],
			img_src: vec!["'self'".to_string()],
			connect_src: vec!["'self'".to_string()],
			font_src: vec!["'self'".to_string()],
			object_src: vec!["'none'".to_string()],
			media_src: vec!["'self'".to_string()],
			frame_src: vec!["'self'".to_string()],
			report_uri: None,
			report_to: None,
			auto_nonce: false,
		}
	}
}

impl ContentSecurityPolicy {
	pub fn new() -> Self {
		Self::default()
	}

	/// Generate a nonce value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::ContentSecurityPolicy;
	///
	/// let csp = ContentSecurityPolicy::new();
	/// let nonce = csp.generate_nonce();
	/// assert_eq!(nonce.len(), 32);
	/// ```
	pub fn generate_nonce(&self) -> String {
		generate_token(32)
	}

	/// Set the report URI
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::ContentSecurityPolicy;
	///
	/// let csp = ContentSecurityPolicy::new()
	///     .with_report_uri("/csp-report");
	/// assert_eq!(csp.report_uri, Some("/csp-report".to_string()));
	/// ```
	pub fn with_report_uri(mut self, uri: &str) -> Self {
		self.report_uri = Some(uri.to_string());
		self
	}

	/// Set the Reporting API endpoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::ContentSecurityPolicy;
	///
	/// let csp = ContentSecurityPolicy::new()
	///     .with_report_to("csp-endpoint");
	/// assert_eq!(csp.report_to, Some("csp-endpoint".to_string()));
	/// ```
	pub fn with_report_to(mut self, endpoint: &str) -> Self {
		self.report_to = Some(endpoint.to_string());
		self
	}

	/// Enable automatic nonce generation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::ContentSecurityPolicy;
	///
	/// let csp = ContentSecurityPolicy::new()
	///     .with_auto_nonce(true);
	/// assert!(csp.auto_nonce);
	/// ```
	pub fn with_auto_nonce(mut self, enable: bool) -> Self {
		self.auto_nonce = enable;
		self
	}

	/// Generate header value with nonce
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::ContentSecurityPolicy;
	///
	/// let csp = ContentSecurityPolicy::new();
	/// let nonce = "abc123";
	/// let header = csp.to_header_value_with_nonce(Some(nonce));
	/// assert!(header.contains("'nonce-abc123'"));
	/// ```
	pub fn to_header_value_with_nonce(&self, nonce: Option<&str>) -> String {
		let mut parts = Vec::new();

		if !self.default_src.is_empty() {
			parts.push(format!("default-src {}", self.default_src.join(" ")));
		}

		if !self.script_src.is_empty() {
			let mut script_src = self.script_src.clone();
			if let Some(n) = nonce {
				script_src.push(format!("'nonce-{}'", n));
			}
			parts.push(format!("script-src {}", script_src.join(" ")));
		}

		if !self.style_src.is_empty() {
			let mut style_src = self.style_src.clone();
			if let Some(n) = nonce {
				style_src.push(format!("'nonce-{}'", n));
			}
			parts.push(format!("style-src {}", style_src.join(" ")));
		}

		if !self.img_src.is_empty() {
			parts.push(format!("img-src {}", self.img_src.join(" ")));
		}
		if !self.connect_src.is_empty() {
			parts.push(format!("connect-src {}", self.connect_src.join(" ")));
		}
		if !self.font_src.is_empty() {
			parts.push(format!("font-src {}", self.font_src.join(" ")));
		}
		if !self.object_src.is_empty() {
			parts.push(format!("object-src {}", self.object_src.join(" ")));
		}
		if !self.media_src.is_empty() {
			parts.push(format!("media-src {}", self.media_src.join(" ")));
		}
		if !self.frame_src.is_empty() {
			parts.push(format!("frame-src {}", self.frame_src.join(" ")));
		}

		if let Some(uri) = &self.report_uri {
			parts.push(format!("report-uri {}", uri));
		}

		if let Some(endpoint) = &self.report_to {
			parts.push(format!("report-to {}", endpoint));
		}

		parts.join("; ")
	}

	pub fn to_header_value(&self) -> String {
		self.to_header_value_with_nonce(None)
	}
}

/// Security headers configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityHeadersConfig {
	pub x_content_type_options: bool,
	pub x_frame_options: Option<String>,
	pub x_xss_protection: bool,
	pub strict_transport_security: Option<String>,
	pub content_security_policy: Option<ContentSecurityPolicy>,
	pub referrer_policy: Option<String>,
	pub permissions_policy: Option<String>,
	/// Cross-Origin-Embedder-Policy
	pub cross_origin_embedder_policy: Option<String>,
	/// Cross-Origin-Opener-Policy
	pub cross_origin_opener_policy: Option<String>,
	/// Cross-Origin-Resource-Policy
	pub cross_origin_resource_policy: Option<String>,
}

impl Default for SecurityHeadersConfig {
	fn default() -> Self {
		Self {
			x_content_type_options: true,
			x_frame_options: Some("DENY".to_string()),
			x_xss_protection: true,
			strict_transport_security: Some("max-age=31536000; includeSubDomains".to_string()),
			content_security_policy: Some(ContentSecurityPolicy::default()),
			referrer_policy: Some("strict-origin-when-cross-origin".to_string()),
			permissions_policy: None,
			cross_origin_embedder_policy: Some("require-corp".to_string()),
			cross_origin_opener_policy: Some("same-origin".to_string()),
			cross_origin_resource_policy: Some("same-origin".to_string()),
		}
	}
}

impl SecurityHeadersConfig {
	/// Security headers configuration for production
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::SecurityHeadersConfig;
	///
	/// let config = SecurityHeadersConfig::production();
	/// assert!(config.x_content_type_options);
	/// assert_eq!(config.x_frame_options, Some("DENY".to_string()));
	/// ```
	pub fn production() -> Self {
		Self::default()
	}

	/// Security headers configuration for development (relaxed version)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::headers::SecurityHeadersConfig;
	///
	/// let config = SecurityHeadersConfig::development();
	/// assert_eq!(config.x_frame_options, Some("SAMEORIGIN".to_string()));
	/// ```
	pub fn development() -> Self {
		Self {
			x_content_type_options: true,
			x_frame_options: Some("SAMEORIGIN".to_string()),
			x_xss_protection: true,
			strict_transport_security: None, // To work with HTTP as well
			content_security_policy: None,   // Disabled during development
			referrer_policy: Some("no-referrer-when-downgrade".to_string()),
			permissions_policy: None,
			cross_origin_embedder_policy: None,
			cross_origin_opener_policy: None,
			cross_origin_resource_policy: None,
		}
	}
}

/// Security headers middleware
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityHeadersMiddleware {
	config: SecurityHeadersConfig,
}

impl SecurityHeadersMiddleware {
	pub fn new() -> Self {
		Self {
			config: SecurityHeadersConfig::default(),
		}
	}

	pub fn with_config(config: SecurityHeadersConfig) -> Self {
		Self { config }
	}

	pub fn config(&self) -> &SecurityHeadersConfig {
		&self.config
	}
}

impl Default for SecurityHeadersMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = SecurityHeadersConfig::default();
		assert!(config.x_content_type_options);
		assert_eq!(config.x_frame_options, Some("DENY".to_string()));
		assert!(config.x_xss_protection);
		assert!(config.strict_transport_security.is_some());
	}

	#[test]
	fn test_csp_to_header() {
		let csp = ContentSecurityPolicy::default();
		let header = csp.to_header_value();
		assert!(header.contains("default-src 'self'"));
		assert!(header.contains("script-src 'self'"));
	}

	#[test]
	fn test_csp_with_nonce() {
		let csp = ContentSecurityPolicy::new();
		let nonce = "abc123xyz";
		let header = csp.to_header_value_with_nonce(Some(nonce));
		assert!(header.contains("'nonce-abc123xyz'"));
	}

	#[test]
	fn test_csp_generate_nonce() {
		let csp = ContentSecurityPolicy::new();
		let nonce = csp.generate_nonce();
		assert_eq!(nonce.len(), 32);
	}

	#[test]
	fn test_csp_with_report_uri() {
		let csp = ContentSecurityPolicy::new().with_report_uri("/csp-report");
		let header = csp.to_header_value();
		assert!(header.contains("report-uri /csp-report"));
	}

	#[test]
	fn test_csp_with_report_to() {
		let csp = ContentSecurityPolicy::new().with_report_to("csp-endpoint");
		let header = csp.to_header_value();
		assert!(header.contains("report-to csp-endpoint"));
	}

	#[test]
	fn test_security_headers_production() {
		let config = SecurityHeadersConfig::production();
		assert!(config.x_content_type_options);
		assert_eq!(config.x_frame_options, Some("DENY".to_string()));
		assert!(config.cross_origin_embedder_policy.is_some());
	}

	#[test]
	fn test_security_headers_development() {
		let config = SecurityHeadersConfig::development();
		assert_eq!(config.x_frame_options, Some("SAMEORIGIN".to_string()));
		assert!(config.strict_transport_security.is_none());
		assert!(config.content_security_policy.is_none());
	}
}
