//! # Reinhardt Security
//!
//! Security features for Reinhardt framework including CSRF protection,
//! XSS prevention, and security header management.
//!
//! ## Features
//!
//! - CSRF (Cross-Site Request Forgery) protection
//! - XSS (Cross-Site Scripting) prevention helpers
//! - Security headers middleware
//! - Content Security Policy (CSP)
//! - Clickjacking protection
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_core::security::{CsrfMiddleware, SecurityHeadersMiddleware};
//!
//! # struct App;
//! # impl App {
//! #     fn add_middleware<T>(&mut self, _middleware: T) {}
//! # }
//! # let mut app = App;
//! // Add CSRF protection
//! app.add_middleware(CsrfMiddleware::new());
//!
//! // Add security headers
//! app.add_middleware(SecurityHeadersMiddleware::default());
//! ```

pub mod bounds;
pub mod csrf;
pub mod headers;
pub mod hsts;
pub mod ip_filter;
pub mod redirect;
pub mod resource_limits;
pub mod utils;
pub mod xss;

pub use bounds::CheckedArithmeticError;
pub use csrf::{CsrfConfig, CsrfMiddleware, CsrfToken};
pub use headers::{ContentSecurityPolicy, SecurityHeadersConfig, SecurityHeadersMiddleware};
pub use hsts::{HstsConfig, HstsMiddleware};
pub use ip_filter::{IpFilterConfig, IpFilterMiddleware, IpFilterMode};
pub use redirect::{RedirectValidationError, is_safe_redirect, validate_redirect_url};
pub use resource_limits::{LimitExceeded, ResourceLimits};
pub use xss::{
	escape_css_selector, escape_html, escape_html_content, sanitize_html, strip_tags_safe,
	validate_css_selector,
};

use thiserror::Error;

/// Result type for security operations
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Security-related errors
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum SecurityError {
	/// CSRF token validation failed
	#[error("CSRF validation failed: {0}")]
	CsrfValidationFailed(String),

	/// Missing CSRF token
	#[error("Missing CSRF token")]
	MissingCsrfToken,

	/// Invalid security configuration
	#[error("Invalid security configuration: {0}")]
	InvalidConfiguration(String),

	/// XSS attempt detected
	#[error("Potential XSS detected: {0}")]
	XssDetected(String),
}

// Re-export CSRF types and constants
pub use csrf::{
	CSRF_SECRET_LENGTH, CSRF_SESSION_KEY, CSRF_TOKEN_LENGTH, REASON_BAD_ORIGIN, REASON_BAD_REFERER,
	REASON_CSRF_TOKEN_MISSING, REASON_INSECURE_REFERER, REASON_MALFORMED_REFERER,
	REASON_NO_REFERER, RejectRequest,
};

// Re-export CSRF utility functions
pub use csrf::{CsrfMeta, check_origin, check_referer, is_same_domain};

// Re-export HMAC-based CSRF functions (primary API)
pub use csrf::{
	check_token_hmac, generate_token_hmac, get_secret_bytes, get_token_hmac, verify_token_hmac,
};
