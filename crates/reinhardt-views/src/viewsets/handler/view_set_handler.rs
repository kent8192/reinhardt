//! `ViewSetHandler` — action mapping and dispatch from HTTP `Method` to a
//! ViewSet `Action`.
//!
//! This module is responsible for:
//!
//! - Mapping incoming HTTP methods to named viewset actions via `action_map`
//! - Extracting path parameters (DRF-style `kwargs`)
//! - Running viewset middleware before and after dispatch
//! - Producing a `405 Method Not Allowed` response with a populated `Allow`
//!   header when the request method is not in the mapping

use crate::{Action, ViewSet};
use async_trait::async_trait;
use hyper::Method;
use parking_lot::RwLock;
use reinhardt_http::{Handler, Request, Response, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Handler implementation that wraps a `ViewSet`.
pub struct ViewSetHandler<V: ViewSet> {
	viewset: Arc<V>,
	action_map: HashMap<Method, String>,
	// Allow dead_code: stored for DRF-compatible handler identification in URL reversing
	#[allow(dead_code)]
	name: Option<String>,
	// Allow dead_code: stored for DRF-compatible view suffix (e.g. "List", "Instance") in URL reversing
	#[allow(dead_code)]
	suffix: Option<String>,

	// Attributes set after as_view() is called
	// These mirror Django REST Framework's behavior
	args: RwLock<Option<Vec<String>>>,
	kwargs: RwLock<Option<HashMap<String, String>>>,
	has_handled_request: RwLock<bool>,
}

// parking_lot::RwLock does not use poisoning, so the locks in
// ViewSetHandler do not introduce unwind-safety hazards. The unwind
// safety of the handler still depends on the underlying ViewSet,
// so we only assert RefUnwindSafe when V is itself RefUnwindSafe.
impl<V: ViewSet + std::panic::RefUnwindSafe> std::panic::RefUnwindSafe for ViewSetHandler<V> {}

impl<V: ViewSet> ViewSetHandler<V> {
	/// Create a new `ViewSetHandler` with the given viewset and action mapping.
	pub fn new(
		viewset: Arc<V>,
		action_map: HashMap<Method, String>,
		name: Option<String>,
		suffix: Option<String>,
	) -> Self {
		Self {
			viewset,
			action_map,
			name,
			suffix,
			args: RwLock::new(None),
			kwargs: RwLock::new(None),
			has_handled_request: RwLock::new(false),
		}
	}

	/// Check if args attribute is set (for testing)
	pub fn has_args(&self) -> bool {
		self.args.read().is_some()
	}

	/// Check if kwargs attribute is set (for testing)
	pub fn has_kwargs(&self) -> bool {
		self.kwargs.read().is_some()
	}

	/// Check if request attribute is set (for testing)
	pub fn has_request(&self) -> bool {
		*self.has_handled_request.read()
	}

	/// Check if action_map is set (for testing)
	pub fn has_action_map(&self) -> bool {
		!self.action_map.is_empty()
	}
}

#[async_trait]
impl<V: ViewSet + 'static> Handler for ViewSetHandler<V> {
	async fn handle(&self, mut request: Request) -> Result<Response> {
		// Set attributes when handling request (DRF behavior)
		*self.has_handled_request.write() = true;
		*self.args.write() = Some(Vec::new());

		// Extract path parameters from URI
		let kwargs = extract_path_params(&request);
		*self.kwargs.write() = Some(kwargs);

		let middleware = self.viewset.get_middleware();

		// Process middleware before ViewSet
		if let Some(middleware) = &middleware
			&& let Some(response) = middleware.process_request(&mut request).await?
		{
			return middleware.process_response(&request, response).await;
		}

		let request_for_response = middleware
			.as_ref()
			.map(|_| request.clone_for_response())
			.transpose()?;

		// Resolve action from HTTP method and dispatch to the ViewSet.
		let response = match self.action_map.get(&request.method) {
			Some(action_name) => {
				let action = Action::from_name(action_name);
				self.viewset.dispatch(request, action).await?
			}
			None => {
				let allowed: Vec<String> = self.action_map.keys().map(|m| m.to_string()).collect();
				let mut response = Response::new(hyper::StatusCode::METHOD_NOT_ALLOWED);
				match allowed.join(", ").parse() {
					Ok(header_value) => {
						response.headers.insert(hyper::header::ALLOW, header_value);
					}
					Err(e) => {
						tracing::warn!(
							error = %e,
							"Failed to parse allowed methods as header value"
						);
					}
				}
				response
			}
		};

		if let (Some(middleware), Some(request)) = (middleware, request_for_response) {
			middleware.process_response(&request, response).await
		} else {
			Ok(response)
		}
	}
}

/// Extract path parameters from request.
///
/// Simple implementation — in production this would use the router's path
/// matching. If the path has a pattern like `/resource/123/`, `123` is
/// captured as the `id` parameter.
pub(crate) fn extract_path_params(request: &Request) -> HashMap<String, String> {
	let mut params = HashMap::new();

	// Simple extraction: if path has pattern like /resource/123/
	// extract "123" as the "id" parameter
	let path = request.uri.path();
	let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

	// If we have at least 2 segments, treat the second as an ID parameter.
	// Accept any non-empty segment (numeric, UUID, slug, etc.)
	if segments.len() >= 2 {
		params.insert("id".to_string(), segments[1].to_string());
	}

	params
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::viewsets::ViewSetMiddleware;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::Request;
	use rstest::rstest;
	use std::thread;

	fn build_request(uri: &str) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_parking_lot_rwlock_does_not_poison_after_panic() {
		// Arrange
		// parking_lot::RwLock does not poison, so after a thread panics
		// while holding the lock, subsequent access should succeed.
		let lock = RwLock::new(42);

		// Act - panic while holding write lock
		let lock_ref = &lock;
		let result = thread::scope(|s| {
			let handle = s.spawn(|| {
				let mut guard = lock_ref.write();
				*guard = 100;
				panic!("intentional panic while holding write lock");
			});
			let _ = handle.join(); // Thread panicked

			// Assert - lock is still usable (no poisoning)
			*lock_ref.read()
		});

		// parking_lot recovers the lock after panic
		assert!(result == 42 || result == 100);
	}

	#[rstest]
	fn test_rwlock_concurrent_read_access() {
		// Arrange
		let lock = RwLock::new(String::from("test_value"));

		// Act - multiple readers should not block each other
		let guard1 = lock.read();
		let guard2 = lock.read();

		// Assert
		assert_eq!(*guard1, "test_value");
		assert_eq!(*guard2, "test_value");
	}

	#[rstest]
	fn test_extract_path_params_numeric_segment_treated_as_id() {
		// Arrange
		let request = build_request("/resource/123/");

		// Act
		let params = extract_path_params(&request);

		// Assert
		assert_eq!(params.get("id"), Some(&"123".to_string()));
	}

	#[rstest]
	fn test_extract_path_params_non_numeric_segment_treated_as_id() {
		// Arrange
		let request = build_request("/resource/username/");

		// Act
		let params = extract_path_params(&request);

		// Assert
		assert_eq!(params.get("id"), Some(&"username".to_string()));
	}

	#[rstest]
	fn test_extract_path_params_slug_segment_treated_as_id() {
		// Arrange
		let request = build_request("/resource/my-slug/");

		// Act
		let params = extract_path_params(&request);

		// Assert
		assert_eq!(params.get("id"), Some(&"my-slug".to_string()));
	}

	#[rstest]
	fn test_extract_path_params_uuid_segment_treated_as_id() {
		// Arrange
		let request = build_request("/resource/550e8400-e29b-41d4-a716-446655440000/");

		// Act
		let params = extract_path_params(&request);

		// Assert
		assert_eq!(
			params.get("id"),
			Some(&"550e8400-e29b-41d4-a716-446655440000".to_string())
		);
	}

	#[rstest]
	fn test_extract_path_params_single_segment_no_id() {
		// Arrange
		let request = build_request("/resource/");

		// Act
		let params = extract_path_params(&request);

		// Assert
		assert_eq!(params.get("id"), None);
	}

	/// Minimal ViewSet implementation for testing ViewSetHandler
	struct MockViewSet;

	#[async_trait]
	impl ViewSet for MockViewSet {
		fn get_basename(&self) -> &str {
			"mock"
		}

		async fn dispatch(
			&self,
			_request: reinhardt_http::Request,
			_action: crate::Action,
		) -> reinhardt_http::Result<reinhardt_http::Response> {
			Ok(reinhardt_http::Response::ok())
		}
	}

	/// Helper to build a ViewSetHandler with a specific action_map
	fn build_handler(methods: Vec<Method>) -> ViewSetHandler<MockViewSet> {
		let mut action_map = HashMap::new();
		for method in methods {
			action_map.insert(method, "mock_action".to_string());
		}
		ViewSetHandler::new(Arc::new(MockViewSet), action_map, None, None)
	}

	/// Helper to build a minimal request with the given method
	fn build_method_request(method: Method) -> reinhardt_http::Request {
		reinhardt_http::Request::builder()
			.method(method)
			.uri("/mock/")
			.version(hyper::Version::HTTP_11)
			.headers(hyper::HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_unregistered_method_returns_405() {
		// Arrange
		let handler = build_handler(vec![Method::GET]);
		let request = build_method_request(Method::DELETE);

		// Act
		let response = Handler::handle(&handler, request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
	}

	#[rstest]
	#[tokio::test]
	async fn test_405_response_allow_header_contains_registered_methods() {
		// Arrange
		let handler = build_handler(vec![Method::GET, Method::POST]);
		let request = build_method_request(Method::DELETE);

		// Act
		let response = Handler::handle(&handler, request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
		let allow_header = response
			.headers
			.get(hyper::header::ALLOW)
			.expect("Allow header must be present");
		let allow_str = allow_header.to_str().unwrap();
		// Both registered methods must appear in the Allow header
		assert!(allow_str.contains("GET"), "Allow header must contain GET");
		assert!(allow_str.contains("POST"), "Allow header must contain POST");
	}

	#[rstest]
	#[tokio::test]
	async fn test_405_response_allow_header_comma_separated_format() {
		// Arrange
		let handler = build_handler(vec![Method::GET, Method::PUT]);
		let request = build_method_request(Method::PATCH);

		// Act
		let response = Handler::handle(&handler, request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
		let allow_header = response
			.headers
			.get(hyper::header::ALLOW)
			.expect("Allow header must be present");
		let allow_str = allow_header.to_str().unwrap();
		// Verify comma-separated format: each method is separated by ", "
		let methods: Vec<&str> = allow_str.split(", ").collect();
		assert_eq!(
			methods.len(),
			2,
			"Allow header must contain exactly 2 methods"
		);
		for method in &methods {
			assert!(
				*method == "GET" || *method == "PUT",
				"Unexpected method in Allow header: {}",
				method
			);
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_registered_method_does_not_return_405() {
		// Arrange
		let handler = build_handler(vec![Method::GET]);
		let request = build_method_request(Method::GET);

		// Act
		let response = Handler::handle(&handler, request).await.unwrap();

		// Assert
		assert_eq!(response.status, hyper::StatusCode::OK);
	}

	struct ResponseMiddleware;

	#[async_trait]
	impl ViewSetMiddleware for ResponseMiddleware {
		async fn process_request(&self, _request: &mut Request) -> Result<Option<Response>> {
			Ok(None)
		}

		async fn process_response(
			&self,
			_request: &Request,
			mut response: Response,
		) -> Result<Response> {
			response.headers.insert(
				"x-viewset-response",
				hyper::header::HeaderValue::from_static("processed"),
			);
			Ok(response)
		}
	}

	struct MiddlewareViewSet;

	#[async_trait]
	impl ViewSet for MiddlewareViewSet {
		fn get_basename(&self) -> &str {
			"middleware"
		}

		async fn dispatch(&self, _request: Request, _action: crate::Action) -> Result<Response> {
			Ok(Response::ok())
		}

		fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
			Some(Arc::new(ResponseMiddleware))
		}
	}

	#[tokio::test]
	async fn test_response_middleware_runs_after_dispatch() {
		let mut action_map = HashMap::new();
		action_map.insert(Method::GET, "list".to_string());
		let handler = ViewSetHandler::new(Arc::new(MiddlewareViewSet), action_map, None, None);

		let response = Handler::handle(&handler, build_method_request(Method::GET))
			.await
			.unwrap();

		assert_eq!(
			response.headers.get("x-viewset-response"),
			Some(&hyper::header::HeaderValue::from_static("processed"))
		);
	}
}
