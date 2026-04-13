//! Handler types for intercepting and responding to requests.

use std::cell::Cell;
use std::collections::HashMap;
use std::time::Duration;

use super::matcher::{UrlMatcher, extract_path};
use super::response::MockResponse;

/// An intercepted HTTP request extracted from JS.
#[derive(Debug, Clone)]
pub struct InterceptedRequest {
	/// The request URL.
	pub url: String,
	/// The HTTP method.
	pub method: String,
	/// Request headers.
	pub headers: HashMap<String, String>,
	/// Request body, if present.
	pub body: Option<String>,
}

/// HTTP method for handler matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
	Get,
	Post,
	Put,
	Delete,
	Patch,
}

impl Method {
	pub(crate) fn as_str(&self) -> &'static str {
		match self {
			Method::Get => "GET",
			Method::Post => "POST",
			Method::Put => "PUT",
			Method::Delete => "DELETE",
			Method::Patch => "PATCH",
		}
	}
}

/// Type-erased handler interface for heterogeneous storage.
pub(crate) trait ErasedHandler {
	fn matches(&self, req: &InterceptedRequest) -> bool;
	/// Build the response. Does NOT apply delay (caller handles that).
	fn respond(&self, req: &InterceptedRequest) -> Option<MockResponse>;
	fn is_consumed(&self) -> bool;
	/// Returns the delay duration if configured.
	fn delay(&self) -> Option<Duration>;
	/// Returns true if this is a network error handler.
	fn is_network_error(&self) -> bool;
}

/// Handler for REST endpoints.
pub struct RestHandler {
	method: Method,
	matcher: UrlMatcher,
	response_fn: Option<Box<dyn Fn(&InterceptedRequest) -> MockResponse>>,
	once: bool,
	consumed: Cell<bool>,
	delay: Option<Duration>,
	network_error: bool,
}

impl RestHandler {
	pub(crate) fn new(
		method: Method,
		matcher: UrlMatcher,
		response_fn: Box<dyn Fn(&InterceptedRequest) -> MockResponse>,
		once: bool,
		delay: Option<Duration>,
	) -> Self {
		Self {
			method,
			matcher,
			response_fn: Some(response_fn),
			once,
			consumed: Cell::new(false),
			delay,
			network_error: false,
		}
	}

	pub(crate) fn network_error(method: Method, matcher: UrlMatcher, once: bool) -> Self {
		Self {
			method,
			matcher,
			response_fn: None,
			once,
			consumed: Cell::new(false),
			delay: None,
			network_error: true,
		}
	}
}

impl ErasedHandler for RestHandler {
	fn matches(&self, req: &InterceptedRequest) -> bool {
		if self.consumed.get() {
			return false;
		}
		req.method == self.method.as_str() && self.matcher.matches(&req.url)
	}

	fn respond(&self, req: &InterceptedRequest) -> Option<MockResponse> {
		if self.once {
			self.consumed.set(true);
		}
		self.response_fn.as_ref().map(|f| f(req))
	}

	fn is_consumed(&self) -> bool {
		self.consumed.get()
	}

	fn delay(&self) -> Option<Duration> {
		self.delay
	}

	fn is_network_error(&self) -> bool {
		self.network_error
	}
}

use std::marker::PhantomData;

use reinhardt_pages::server_fn::MockableServerFn;
use reinhardt_pages::server_fn::ServerFnError;

use super::context::TestContext;

/// Type-safe handler for server functions.
pub(crate) struct ServerFnHandler<S: MockableServerFn> {
	response_fn: Box<dyn Fn(S::Args) -> Result<S::Response, ServerFnError>>,
	once: bool,
	consumed: Cell<bool>,
	delay: Option<Duration>,
	_marker: PhantomData<S>,
}

impl<S: MockableServerFn> ServerFnHandler<S> {
	pub(crate) fn new(
		response_fn: Box<dyn Fn(S::Args) -> Result<S::Response, ServerFnError>>,
		once: bool,
		delay: Option<Duration>,
	) -> Self {
		Self {
			response_fn,
			once,
			consumed: Cell::new(false),
			delay,
			_marker: PhantomData,
		}
	}
}

impl<S: MockableServerFn> ErasedHandler for ServerFnHandler<S> {
	fn matches(&self, req: &InterceptedRequest) -> bool {
		if self.consumed.get() {
			return false;
		}
		req.method == "POST" && extract_path(&req.url) == S::PATH
	}

	fn respond(&self, req: &InterceptedRequest) -> Option<MockResponse> {
		if self.once {
			self.consumed.set(true);
		}
		let body = req.body.as_deref().unwrap_or("{}");
		let args: S::Args = serde_json::from_str(body).ok()?;
		let result = (self.response_fn)(args);
		match result {
			Ok(response) => {
				let body = serde_json::to_string(&response).ok()?;
				Some(MockResponse {
					status: 200,
					headers: {
						let mut h = HashMap::new();
						h.insert("content-type".to_string(), "application/json".to_string());
						h
					},
					body,
				})
			}
			Err(err) => {
				let body = serde_json::to_string(&err).unwrap_or_default();
				Some(MockResponse {
					status: 500,
					headers: {
						let mut h = HashMap::new();
						h.insert("content-type".to_string(), "application/json".to_string());
						h
					},
					body,
				})
			}
		}
	}

	fn is_consumed(&self) -> bool {
		self.consumed.get()
	}

	fn delay(&self) -> Option<Duration> {
		self.delay
	}

	fn is_network_error(&self) -> bool {
		false
	}
}

/// Type-safe handler for server functions with DI test context.
pub(crate) struct ServerFnContextHandler<S: MockableServerFn> {
	response_fn: Box<dyn Fn(S::Args, &TestContext) -> Result<S::Response, ServerFnError>>,
	context: TestContext,
	once: bool,
	consumed: Cell<bool>,
	delay: Option<Duration>,
	_marker: PhantomData<S>,
}

impl<S: MockableServerFn> ServerFnContextHandler<S> {
	pub(crate) fn new(
		context: TestContext,
		response_fn: Box<dyn Fn(S::Args, &TestContext) -> Result<S::Response, ServerFnError>>,
		once: bool,
		delay: Option<Duration>,
	) -> Self {
		Self {
			response_fn,
			context,
			once,
			consumed: Cell::new(false),
			delay,
			_marker: PhantomData,
		}
	}
}

impl<S: MockableServerFn> ErasedHandler for ServerFnContextHandler<S> {
	fn matches(&self, req: &InterceptedRequest) -> bool {
		if self.consumed.get() {
			return false;
		}
		req.method == "POST" && extract_path(&req.url) == S::PATH
	}

	fn respond(&self, req: &InterceptedRequest) -> Option<MockResponse> {
		if self.once {
			self.consumed.set(true);
		}
		let body = req.body.as_deref().unwrap_or("{}");
		let args: S::Args = serde_json::from_str(body).ok()?;
		let result = (self.response_fn)(args, &self.context);
		match result {
			Ok(response) => {
				let body = serde_json::to_string(&response).ok()?;
				Some(MockResponse {
					status: 200,
					headers: {
						let mut h = HashMap::new();
						h.insert("content-type".to_string(), "application/json".to_string());
						h
					},
					body,
				})
			}
			Err(err) => {
				let body = serde_json::to_string(&err).unwrap_or_default();
				Some(MockResponse {
					status: 500,
					headers: {
						let mut h = HashMap::new();
						h.insert("content-type".to_string(), "application/json".to_string());
						h
					},
					body,
				})
			}
		}
	}

	fn is_consumed(&self) -> bool {
		self.consumed.get()
	}

	fn delay(&self) -> Option<Duration> {
		self.delay
	}

	fn is_network_error(&self) -> bool {
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	fn make_intercepted(url: &str, method: &str) -> InterceptedRequest {
		InterceptedRequest {
			url: url.to_string(),
			method: method.to_string(),
			headers: HashMap::new(),
			body: None,
		}
	}

	#[rstest]
	fn rest_handler_matches_method_and_url() {
		let handler = RestHandler::new(
			Method::Get,
			"/api/users".into(),
			Box::new(|_| MockResponse::empty()),
			false,
			None,
		);
		assert!(handler.matches(&make_intercepted("/api/users", "GET")));
		assert!(!handler.matches(&make_intercepted("/api/users", "POST")));
		assert!(!handler.matches(&make_intercepted("/api/posts", "GET")));
	}

	#[rstest]
	fn rest_handler_once_is_consumed_after_respond() {
		let handler = RestHandler::new(
			Method::Get,
			"/api/users".into(),
			Box::new(|_| MockResponse::empty()),
			true,
			None,
		);
		assert!(!handler.is_consumed());
		let _ = handler.respond(&make_intercepted("/api/users", "GET"));
		assert!(handler.is_consumed());
	}

	#[rstest]
	fn rest_handler_reusable_not_consumed() {
		let handler = RestHandler::new(
			Method::Get,
			"/api/users".into(),
			Box::new(|_| MockResponse::empty()),
			false,
			None,
		);
		let _ = handler.respond(&make_intercepted("/api/users", "GET"));
		let _ = handler.respond(&make_intercepted("/api/users", "GET"));
		assert!(!handler.is_consumed());
	}

	#[rstest]
	fn rest_handler_delay() {
		let handler = RestHandler::new(
			Method::Get,
			"/api/users".into(),
			Box::new(|_| MockResponse::empty()),
			false,
			Some(Duration::from_millis(100)),
		);
		assert_eq!(handler.delay(), Some(Duration::from_millis(100)));
	}

	#[rstest]
	fn network_error_handler() {
		let handler = RestHandler::network_error(Method::Get, "/api/fail".into(), false);
		assert!(handler.matches(&make_intercepted("/api/fail", "GET")));
		assert!(handler.is_network_error());
		assert!(
			handler
				.respond(&make_intercepted("/api/fail", "GET"))
				.is_none()
		);
	}
}
