//! API Client fixtures for E2E testing
//!
//! Provides helper functions for creating APIClient instances
//! that connect to test servers.
//!
//! ## Overview
//!
//! This module provides helper functions for creating `APIClient` instances
//! configured to connect to test servers. Use `api_client_from_url` with
//! the server URL obtained from `TestServerGuard`.
//!
//! ## Usage Examples
//!
//! ### Using api_client_from_url with TestServerGuard
//!
//! ```rust,no_run
//! use reinhardt_testkit::fixtures::{test_server_guard, api_client_from_url, TestServerGuard};
//! use reinhardt_urls::routers::UnifiedRouter;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_api() {
//!     let router = UnifiedRouter::new();
//!     let server = test_server_guard(router).await;
//!     let client = api_client_from_url(&server.url);
//!     let response = client.get("/api/test").await.unwrap();
//!     assert_eq!(response.status_code(), 200);
//! }
//! ```
//!
//! ### Creating APIClient directly
//!
//! ```rust,no_run
//! use reinhardt_testkit::APIClient;
//!
//! # async fn example() {
//! let client = APIClient::with_base_url("http://localhost:8080");
//! let response = client.get("/api/test").await.unwrap();
//! # }
//! ```

use crate::client::APIClient;

/// Create an APIClient from a server URL string
///
/// This is a helper function for creating an `APIClient` when you already
/// have a server URL. Use this when you need more control over the server
/// setup or when working with existing test infrastructure.
///
/// # Arguments
///
/// * `url` - The base URL of the server to connect to (e.g., "http://localhost:8080")
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::api_client_from_url;
///
/// # async fn example() {
/// let client = api_client_from_url("http://localhost:8080");
/// let response = client.get("/api/users").await.unwrap();
/// # }
/// ```
///
/// # Usage with TestServerGuard
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::{test_server_guard, api_client_from_url, TestServerGuard};
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_custom_setup(#[future] test_server_guard: TestServerGuard) {
///     let server = test_server_guard.await;
///     let client = api_client_from_url(&server.url);
///     let response = client.get("/test").await.unwrap();
/// }
/// ```
pub fn api_client_from_url(url: &str) -> APIClient {
	APIClient::with_base_url(url)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_api_client_from_url() {
		let client = api_client_from_url("http://localhost:8080");
		assert_eq!(client.base_url(), "http://localhost:8080");
	}

	#[test]
	fn test_api_client_from_url_with_path() {
		let client = api_client_from_url("http://example.com:3000");
		assert_eq!(client.base_url(), "http://example.com:3000");
	}
}
