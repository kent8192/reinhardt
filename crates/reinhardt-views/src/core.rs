//! Core view traits and types.

use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::{Request, Response};
use std::collections::HashMap;

/// Base trait for all generic views
#[async_trait]
pub trait View: Send + Sync {
	/// Dispatch the request and return a response.
	async fn dispatch(&self, request: Request) -> Result<Response>;

	/// Returns the list of HTTP methods allowed by this view
	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "OPTIONS"]
	}
}

/// Context data for template rendering
pub type Context = HashMap<String, serde_json::Value>;
