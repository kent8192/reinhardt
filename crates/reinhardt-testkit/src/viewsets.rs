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
	pub fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
			login_required: false,
			required_permissions: Vec::new(),
		}
	}

	pub fn with_login_required(mut self, login_required: bool) -> Self {
		self.login_required = login_required;
		self
	}

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
