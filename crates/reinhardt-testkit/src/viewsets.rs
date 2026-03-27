//! Test ViewSet implementations for testing
//!
//! This module provides reusable ViewSet implementations for testing purposes.
//! These ViewSets are part of the reinhardt-test crate and can be used across
//! different test scenarios in the Reinhardt framework.

use async_trait::async_trait;
use reinhardt_http::{Request, Response, Result};
use reinhardt_views::viewsets::middleware::{CompositeMiddleware, ViewSetMiddleware};
use reinhardt_views::viewsets::{Action, ViewSet};
use std::sync::Arc;

/// Test ViewSet with configurable middleware support
#[derive(Debug, Clone)]
pub struct TestViewSet {
	basename: String,
	login_required: bool,
	required_permissions: Vec<String>,
}

impl TestViewSet {
	/// Create a new `TestViewSet` with the given basename.
	pub fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
			login_required: false,
			required_permissions: Vec::new(),
		}
	}

	/// Set whether this viewset requires login.
	pub fn with_login_required(mut self, login_required: bool) -> Self {
		self.login_required = login_required;
		self
	}

	/// Set the required permissions for this viewset.
	pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
		self.required_permissions = permissions;
		self
	}

	/// Convert ViewSet to Handler with action mapping
	pub fn as_view(self) -> reinhardt_views::viewsets::builder::ViewSetBuilder<Self> {
		reinhardt_views::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl ViewSet for TestViewSet {
	fn get_basename(&self) -> &str {
		&self.basename
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		// Simple test implementation that always returns success
		let mut response = Response::new(hyper::StatusCode::OK);
		response.body = "Test response".into();
		Ok(response)
	}

	fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
		if self.login_required || !self.required_permissions.is_empty() {
			let mut composite = CompositeMiddleware::new();

			if self.login_required {
				composite = composite.with_authentication(true);
			}

			if !self.required_permissions.is_empty() {
				composite = composite.with_permissions(self.required_permissions.clone());
			}

			Some(Arc::new(composite))
		} else {
			None
		}
	}

	fn requires_login(&self) -> bool {
		self.login_required
	}

	fn get_required_permissions(&self) -> Vec<String> {
		self.required_permissions.clone()
	}
}

/// Simple ViewSet for testing without middleware
#[derive(Debug, Clone)]
pub struct SimpleViewSet {
	basename: String,
}

impl SimpleViewSet {
	/// Create a new `SimpleViewSet` with the given basename.
	pub fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
		}
	}
}

#[async_trait]
impl ViewSet for SimpleViewSet {
	fn get_basename(&self) -> &str {
		&self.basename
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		let mut response = Response::new(hyper::StatusCode::OK);
		response.body = "Simple response".into();
		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::views::create_request;
	use hyper::Method;
	use rstest::rstest;

	// ========================================================================
	// TestViewSet tests
	// ========================================================================

	#[rstest]
	fn test_test_viewset_new_basename() {
		// Arrange & Act
		let viewset = TestViewSet::new("items");

		// Assert
		assert_eq!(viewset.get_basename(), "items");
	}

	#[rstest]
	fn test_test_viewset_default_no_login() {
		// Arrange & Act
		let viewset = TestViewSet::new("items");

		// Assert
		assert!(!viewset.requires_login());
	}

	#[rstest]
	fn test_test_viewset_with_login_required() {
		// Arrange & Act
		let viewset = TestViewSet::new("items").with_login_required(true);

		// Assert
		assert!(viewset.requires_login());
	}

	#[rstest]
	fn test_test_viewset_with_permissions() {
		// Arrange
		let perms = vec!["read".to_string(), "write".to_string()];

		// Act
		let viewset = TestViewSet::new("items").with_permissions(perms.clone());

		// Assert
		assert_eq!(viewset.get_required_permissions(), perms);
	}

	#[rstest]
	#[tokio::test]
	async fn test_test_viewset_dispatch_returns_ok() {
		// Arrange
		let viewset = TestViewSet::new("items");
		let request = create_request(Method::GET, "/api/items/", None, None, None);
		let action = Action::list();

		// Act
		let result = viewset.dispatch(request, action).await;

		// Assert
		assert!(result.is_ok());
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
	}

	#[rstest]
	fn test_test_viewset_no_middleware_by_default() {
		// Arrange & Act
		let viewset = TestViewSet::new("items");

		// Assert
		assert!(viewset.get_middleware().is_none());
	}

	#[rstest]
	fn test_test_viewset_middleware_with_login() {
		// Arrange & Act
		let viewset = TestViewSet::new("items").with_login_required(true);

		// Assert
		assert!(viewset.get_middleware().is_some());
	}

	#[rstest]
	fn test_test_viewset_middleware_with_permissions() {
		// Arrange
		let perms = vec!["admin".to_string()];

		// Act
		let viewset = TestViewSet::new("items").with_permissions(perms);

		// Assert
		assert!(viewset.get_middleware().is_some());
	}

	// ========================================================================
	// SimpleViewSet tests
	// ========================================================================

	#[rstest]
	fn test_simple_viewset_new() {
		// Arrange & Act
		let viewset = SimpleViewSet::new("posts");

		// Assert
		assert_eq!(viewset.get_basename(), "posts");
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_viewset_dispatch() {
		// Arrange
		let viewset = SimpleViewSet::new("posts");
		let request = create_request(Method::GET, "/api/posts/", None, None, None);
		let action = Action::list();

		// Act
		let result = viewset.dispatch(request, action).await;

		// Assert
		assert!(result.is_ok());
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
	}

	// ========================================================================
	// Edge case tests
	// ========================================================================

	#[rstest]
	fn test_test_viewset_empty_permissions_no_middleware() {
		// Arrange & Act
		let viewset = TestViewSet::new("items").with_permissions(vec![]);

		// Assert
		assert!(viewset.get_middleware().is_none());
	}

	#[rstest]
	fn test_test_viewset_get_required_permissions() {
		// Arrange
		let perms = vec!["read".to_string(), "write".to_string(), "admin".to_string()];
		let viewset = TestViewSet::new("items").with_permissions(perms.clone());

		// Act
		let result = viewset.get_required_permissions();

		// Assert
		assert_eq!(result, perms);
	}
}
