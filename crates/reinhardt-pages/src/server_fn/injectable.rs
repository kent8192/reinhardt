//! Injectable implementations for Server Functions
//!
//! This module provides Injectable trait implementations for types commonly
//! used in server function handlers, enabling automatic dependency injection.

use async_trait::async_trait;
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::Request;
use std::sync::Arc;

/// Wrapper for Request that can be injected into server function handlers.
///
/// This allows server functions to access the HTTP request via dependency injection
/// rather than receiving it as a direct parameter.
#[derive(Clone)]
pub struct ServerFnRequest(pub Arc<Request>);

impl ServerFnRequest {
	/// Returns a reference to the inner Request.
	pub fn inner(&self) -> &Request {
		&self.0
	}

	/// Consumes self and returns the inner Request.
	pub fn into_inner(self) -> Arc<Request> {
		self.0
	}
}

#[async_trait]
impl Injectable for ServerFnRequest {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Try to get Request from the injection context
		// The request should be set via ctx.with_request() when handling the HTTP request
		ctx.get_request::<Request>()
			.map(ServerFnRequest)
			.ok_or_else(|| DiError::NotFound("Request not found in InjectionContext. Ensure the server function handler is invoked with a properly configured InjectionContext containing the Request".to_string()))
	}
}

/// Wrapper for the request body that can be injected into server function handlers.
///
/// This extracts and provides the request body as a String for server function
/// argument deserialization.
#[derive(Debug, Clone)]
pub struct ServerFnBody(pub String);

impl ServerFnBody {
	/// Returns a reference to the body string.
	pub fn inner(&self) -> &str {
		&self.0
	}

	/// Consumes self and returns the body string.
	pub fn into_inner(self) -> String {
		self.0
	}
}

#[async_trait]
impl Injectable for ServerFnBody {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Get the request from context
		let request = ctx.get_request::<Request>().ok_or_else(|| {
			DiError::NotFound(
				"Cannot extract body: Request not found in InjectionContext".to_string(),
			)
		})?;

		// Read the body as string
		let body = request
			.read_body()
			.map_err(|e| DiError::ProviderError(format!("Failed to read request body: {}", e)))?;

		let body_string = String::from_utf8(body.to_vec()).map_err(|e| {
			DiError::ProviderError(format!("Request body is not valid UTF-8: {}", e))
		})?;

		Ok(ServerFnBody(body_string))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_server_fn_request_wrapper() {
		let request = Request::builder().uri("/test").build().unwrap();
		let wrapped = ServerFnRequest(Arc::new(request));
		assert_eq!(wrapped.inner().uri.path(), "/test");
	}

	#[test]
	fn test_server_fn_body_wrapper() {
		let body = ServerFnBody("test body".to_string());
		assert_eq!(body.inner(), "test body");
		assert_eq!(body.into_inner(), "test body".to_string());
	}
}
