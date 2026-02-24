//! Origin header validation for WebSocket handshake
//!
//! This module provides Origin header validation to prevent Cross-Site WebSocket
//! Hijacking (CSWSH) attacks. It validates the `Origin` header sent during the
//! WebSocket handshake against a configurable list of allowed origins.
//!
//! # Usage
//!
//! ```
//! use reinhardt_websockets::origin::{OriginValidationMiddleware, OriginPolicy};
//!
//! // Require Origin and only allow specific origins
//! let middleware = OriginValidationMiddleware::new(
//!     OriginPolicy::AllowList(vec![
//!         "https://example.com".to_string(),
//!         "https://app.example.com".to_string(),
//!     ]),
//! );
//! ```

use crate::connection::WebSocketConnection;
use crate::middleware::{
	ConnectionContext, ConnectionMiddleware, MiddlewareError, MiddlewareResult,
};
use async_trait::async_trait;
use std::sync::Arc;

/// Header name for Origin
const ORIGIN_HEADER: &str = "origin";

/// Policy for Origin header validation
#[derive(Debug, Clone)]
pub enum OriginPolicy {
	/// Allow only specific origins
	AllowList(Vec<String>),
	/// Allow all origins (disables validation, not recommended for production)
	AllowAll,
}

/// Configuration for Origin validation behavior
#[derive(Debug, Clone)]
pub struct OriginValidationConfig {
	/// The origin policy to apply
	pub policy: OriginPolicy,
	/// Whether to reject connections with a missing Origin header.
	/// Default: true (reject missing Origin)
	pub reject_missing_origin: bool,
}

impl Default for OriginValidationConfig {
	fn default() -> Self {
		Self {
			policy: OriginPolicy::AllowList(Vec::new()),
			reject_missing_origin: true,
		}
	}
}

impl OriginValidationConfig {
	/// Create a new config with the given policy
	pub fn new(policy: OriginPolicy) -> Self {
		Self {
			policy,
			reject_missing_origin: true,
		}
	}

	/// Set whether to reject connections with missing Origin header
	pub fn with_reject_missing_origin(mut self, reject: bool) -> Self {
		self.reject_missing_origin = reject;
		self
	}
}

/// Middleware that validates the Origin header during WebSocket handshake
///
/// This middleware checks the `Origin` header in the connection context headers
/// against the configured policy and rejects connections from unauthorized origins.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::origin::{OriginValidationMiddleware, OriginPolicy};
/// use reinhardt_websockets::middleware::{ConnectionMiddleware, ConnectionContext};
///
/// # tokio_test::block_on(async {
/// let middleware = OriginValidationMiddleware::new(
///     OriginPolicy::AllowList(vec!["https://example.com".to_string()]),
/// );
///
/// // Valid origin
/// let mut context = ConnectionContext::new("192.168.1.1".to_string())
///     .with_header("origin".to_string(), "https://example.com".to_string());
/// assert!(middleware.on_connect(&mut context).await.is_ok());
///
/// // Invalid origin
/// let mut context = ConnectionContext::new("192.168.1.1".to_string())
///     .with_header("origin".to_string(), "https://evil.com".to_string());
/// assert!(middleware.on_connect(&mut context).await.is_err());
/// # });
/// ```
pub struct OriginValidationMiddleware {
	config: OriginValidationConfig,
}

impl OriginValidationMiddleware {
	/// Create a new Origin validation middleware with the given policy
	pub fn new(policy: OriginPolicy) -> Self {
		Self {
			config: OriginValidationConfig::new(policy),
		}
	}

	/// Create a new Origin validation middleware with full configuration
	pub fn with_config(config: OriginValidationConfig) -> Self {
		Self { config }
	}
}

/// Shared Origin validation function for use across WebSocket entry points
///
/// This function can be called directly from any WebSocket connection handler
/// (including pages integration) to validate the Origin header.
///
/// # Arguments
///
/// * `origin` - The Origin header value, or `None` if not present
/// * `config` - The Origin validation configuration
///
/// # Returns
///
/// Returns `Ok(())` if the Origin is valid, or an error if rejected.
pub fn validate_origin(
	origin: Option<&str>,
	config: &OriginValidationConfig,
) -> MiddlewareResult<()> {
	match origin {
		Some(origin_value) => {
			let origin_value = origin_value.trim();
			if origin_value.is_empty() {
				if config.reject_missing_origin {
					return Err(MiddlewareError::ConnectionRejected(
						"Empty Origin header".to_string(),
					));
				}
				return Ok(());
			}

			match &config.policy {
				OriginPolicy::AllowAll => Ok(()),
				OriginPolicy::AllowList(allowed) => {
					let normalized = origin_value.to_lowercase();
					let normalized = normalized.trim_end_matches('/');
					let is_allowed = allowed.iter().any(|allowed_origin| {
						let allowed_normalized = allowed_origin.trim().to_lowercase();
						let allowed_normalized = allowed_normalized.trim_end_matches('/');
						normalized == allowed_normalized
					});

					if is_allowed {
						Ok(())
					} else {
						Err(MiddlewareError::ConnectionRejected(format!(
							"Origin not allowed: {}",
							origin_value
						)))
					}
				}
			}
		}
		None => {
			if config.reject_missing_origin {
				Err(MiddlewareError::ConnectionRejected(
					"Missing Origin header".to_string(),
				))
			} else {
				Ok(())
			}
		}
	}
}

#[async_trait]
impl ConnectionMiddleware for OriginValidationMiddleware {
	async fn on_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()> {
		let origin = context.headers.get(ORIGIN_HEADER).cloned();
		validate_origin(origin.as_deref(), &self.config)
	}

	async fn on_disconnect(&self, _connection: &Arc<WebSocketConnection>) -> MiddlewareResult<()> {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// -- OriginValidationMiddleware tests --

	#[rstest]
	#[tokio::test]
	async fn test_allow_list_accepts_valid_origin() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://example.com".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_allow_list_rejects_invalid_origin() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://evil.com".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(msg) if msg.contains("Origin not allowed")
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rejects_missing_origin_by_default() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(msg) if msg.contains("Missing Origin header")
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_allows_missing_origin_when_configured() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]))
		.with_reject_missing_origin(false);
		let middleware = OriginValidationMiddleware::with_config(config);
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_allow_all_policy_accepts_any_origin() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowAll);
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://any-origin.com".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_case_insensitive_origin_comparison() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://Example.COM".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://example.com".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_trailing_slash_normalization() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://example.com/".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_multiple_allowed_origins() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://app.example.com".to_string(),
			"https://admin.example.com".to_string(),
			"http://localhost:3000".to_string(),
		]));

		// Act & Assert - each allowed origin should be accepted
		for origin in [
			"https://app.example.com",
			"https://admin.example.com",
			"http://localhost:3000",
		] {
			let mut context = ConnectionContext::new("192.168.1.1".to_string())
				.with_header("origin".to_string(), origin.to_string());
			assert!(
				middleware.on_connect(&mut context).await.is_ok(),
				"Expected origin '{}' to be accepted",
				origin
			);
		}

		// Unlisted origin should be rejected
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://evil.com".to_string());
		assert!(middleware.on_connect(&mut context).await.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_allow_list_rejects_all_origins() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "https://example.com".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_origin_header_rejected_when_required() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));
		let mut context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("origin".to_string(), "".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(msg) if msg.contains("Empty Origin header")
		));
	}

	// -- Shared validate_origin function tests --

	#[rstest]
	fn test_validate_origin_function_valid() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));

		// Act
		let result = validate_origin(Some("https://example.com"), &config);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_origin_function_missing_rejected() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));

		// Act
		let result = validate_origin(None, &config);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_origin_function_missing_allowed() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]))
		.with_reject_missing_origin(false);

		// Act
		let result = validate_origin(None, &config);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_origin_function_invalid() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowList(vec![
			"https://example.com".to_string(),
		]));

		// Act
		let result = validate_origin(Some("https://evil.com"), &config);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_origin_function_allow_all() {
		// Arrange
		let config = OriginValidationConfig::new(OriginPolicy::AllowAll);

		// Act
		let result = validate_origin(Some("https://any.com"), &config);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_on_disconnect_always_succeeds() {
		// Arrange
		let middleware = OriginValidationMiddleware::new(OriginPolicy::AllowList(vec![]));
		let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act
		let result = middleware.on_disconnect(&conn).await;

		// Assert
		assert!(result.is_ok());
	}
}
