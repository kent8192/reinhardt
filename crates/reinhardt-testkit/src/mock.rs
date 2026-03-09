use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Call record for tracking function calls
#[derive(Debug, Clone)]
pub struct CallRecord {
	/// Arguments passed to the function call (serialized as JSON values).
	pub args: Vec<serde_json::Value>,
	/// Timestamp when the call was recorded.
	pub timestamp: std::time::Instant,
}

/// Mock function tracker
pub struct MockFunction<T> {
	calls: Arc<Mutex<Vec<CallRecord>>>,
	return_values: Arc<Mutex<VecDeque<T>>>,
	default_return: Option<T>,
}

impl<T: Clone> MockFunction<T> {
	/// Create a new mock function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// assert_eq!(mock.call_count().await, 0);
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
			return_values: Arc::new(Mutex::new(VecDeque::new())),
			default_return: None,
		}
	}
	/// Create a mock function with a default return value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::with_default(42);
	/// let result = mock.call(vec![]).await;
	/// assert_eq!(result, Some(42));
	/// # });
	/// ```
	pub fn with_default(default_return: T) -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
			return_values: Arc::new(Mutex::new(VecDeque::new())),
			default_return: Some(default_return),
		}
	}
	/// Queue a return value for the next call
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.returns(42).await;
	///
	/// let result = mock.call(vec![]).await;
	/// assert_eq!(result, Some(42));
	/// # });
	/// ```
	pub async fn returns(&self, value: T) {
		self.return_values.lock().await.push_back(value);
	}
	/// Queue multiple return values for sequential calls
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.returns_many(vec![1, 2, 3]).await;
	///
	/// assert_eq!(mock.call(vec![]).await, Some(1));
	/// assert_eq!(mock.call(vec![]).await, Some(2));
	/// assert_eq!(mock.call(vec![]).await, Some(3));
	/// # });
	/// ```
	pub async fn returns_many(&self, values: Vec<T>) {
		let mut queue = self.return_values.lock().await;
		for value in values {
			queue.push_back(value);
		}
	}
	/// Record a call and return the next queued value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.returns(42).await;
	///
	/// let result = mock.call(vec![json!("arg1"), json!(123)]).await;
	/// assert_eq!(result, Some(42));
	/// assert_eq!(mock.call_count().await, 1);
	/// # });
	/// ```
	pub async fn call(&self, args: Vec<serde_json::Value>) -> Option<T> {
		let record = CallRecord {
			args,
			timestamp: std::time::Instant::now(),
		};
		self.calls.lock().await.push(record);

		let mut queue = self.return_values.lock().await;
		queue.pop_front().or_else(|| self.default_return.clone())
	}
	/// Get the number of times the function has been called
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// assert_eq!(mock.call_count().await, 0);
	///
	/// mock.call(vec![]).await;
	/// assert_eq!(mock.call_count().await, 1);
	/// # });
	/// ```
	pub async fn call_count(&self) -> usize {
		self.calls.lock().await.len()
	}
	/// Check if the function has been called at least once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// assert!(!mock.was_called().await);
	///
	/// mock.call(vec![]).await;
	/// assert!(mock.was_called().await);
	/// # });
	/// ```
	pub async fn was_called(&self) -> bool {
		self.call_count().await > 0
	}
	/// Check if the function was called with specific arguments
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.call(vec![json!("test"), json!(42)]).await;
	///
	/// assert!(mock.was_called_with(vec![json!("test"), json!(42)]).await);
	/// assert!(!mock.was_called_with(vec![json!("other")]).await);
	/// # });
	/// ```
	pub async fn was_called_with(&self, args: Vec<serde_json::Value>) -> bool {
		let calls = self.calls.lock().await;
		calls.iter().any(|record| record.args == args)
	}
	/// Get all call records for inspection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.call(vec![json!("arg1")]).await;
	/// mock.call(vec![json!("arg2")]).await;
	///
	/// let calls = mock.get_calls().await;
	/// assert_eq!(calls.len(), 2);
	/// assert_eq!(calls[0].args, vec![json!("arg1")]);
	/// # });
	/// ```
	pub async fn get_calls(&self) -> Vec<CallRecord> {
		self.calls.lock().await.clone()
	}
	/// Reset the mock to its initial state
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.call(vec![]).await;
	/// assert_eq!(mock.call_count().await, 1);
	///
	/// mock.reset().await;
	/// assert_eq!(mock.call_count().await, 0);
	/// # });
	/// ```
	pub async fn reset(&self) {
		self.calls.lock().await.clear();
		self.return_values.lock().await.clear();
	}
	/// Get the arguments from the last function call
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::MockFunction;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let mock = MockFunction::<i32>::new();
	/// mock.call(vec![json!("first")]).await;
	/// mock.call(vec![json!("last")]).await;
	///
	/// let last_args = mock.last_call_args().await;
	/// assert_eq!(last_args, Some(vec![json!("last")]));
	/// # });
	/// ```
	pub async fn last_call_args(&self) -> Option<Vec<serde_json::Value>> {
		self.calls.lock().await.last().map(|r| r.args.clone())
	}
}

impl<T: Clone> Default for MockFunction<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Spy for tracking method calls with arguments
pub struct Spy<T> {
	inner: Option<T>,
	calls: Arc<Mutex<Vec<CallRecord>>>,
}

impl<T> Spy<T> {
	/// Create a new spy without wrapping any object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::Spy;
	///
	/// let spy = Spy::<String>::new();
	/// assert!(spy.inner().is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			inner: None,
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}
	/// Create a spy that wraps an existing object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::Spy;
	///
	/// let value = "test".to_string();
	/// let spy = Spy::wrap(value);
	/// assert!(spy.inner().is_some());
	/// ```
	pub fn wrap(inner: T) -> Self {
		Self {
			inner: Some(inner),
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}
	/// Record a method call with arguments
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::Spy;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let spy = Spy::<String>::new();
	/// spy.record_call(vec![json!("arg1"), json!(42)]).await;
	/// assert_eq!(spy.call_count().await, 1);
	/// # });
	/// ```
	pub async fn record_call(&self, args: Vec<serde_json::Value>) {
		let record = CallRecord {
			args,
			timestamp: std::time::Instant::now(),
		};
		self.calls.lock().await.push(record);
	}
	/// Get the total number of recorded calls.
	pub async fn call_count(&self) -> usize {
		self.calls.lock().await.len()
	}
	/// Check if the spy was called at least once.
	pub async fn was_called(&self) -> bool {
		self.call_count().await > 0
	}
	/// Check if the spy was called with specific arguments
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::Spy;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let spy = Spy::<String>::new();
	/// spy.record_call(vec![json!("test")]).await;
	///
	/// assert!(spy.was_called_with(vec![json!("test")]).await);
	/// assert!(!spy.was_called_with(vec![json!("other")]).await);
	/// # });
	/// ```
	pub async fn was_called_with(&self, args: Vec<serde_json::Value>) -> bool {
		let calls = self.calls.lock().await;
		calls.iter().any(|record| record.args == args)
	}
	/// Get all recorded call records.
	pub async fn get_calls(&self) -> Vec<CallRecord> {
		self.calls.lock().await.clone()
	}
	/// Get the arguments from the last recorded call, if any.
	pub async fn last_call_args(&self) -> Option<Vec<serde_json::Value>> {
		self.calls.lock().await.last().map(|r| r.args.clone())
	}
	/// Reset the spy by clearing all call records
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::mock::Spy;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let spy = Spy::<String>::new();
	/// spy.record_call(vec![json!("test")]).await;
	/// assert_eq!(spy.call_count().await, 1);
	///
	/// spy.reset().await;
	/// assert_eq!(spy.call_count().await, 0);
	/// # });
	/// ```
	pub async fn reset(&self) {
		self.calls.lock().await.clear();
	}
	/// Get a reference to the wrapped inner value, if any.
	pub fn inner(&self) -> Option<&T> {
		self.inner.as_ref()
	}
	/// Consume the spy and return the wrapped inner value, if any.
	pub fn into_inner(self) -> Option<T> {
		self.inner
	}
}

impl<T> Default for Spy<T> {
	fn default() -> Self {
		Self::new()
	}
}

// ============================================================================
// Handler Mocks
// ============================================================================

/// Simple handler wrapper for testing
///
/// Provides a convenient way to create handlers from closures for testing purposes.
/// The handler function can be any closure that takes a `Request` and returns a
/// [`Result<Response>`].
///
/// # Examples
///
/// ## Basic usage
///
/// ```no_run
/// use reinhardt_testkit::mock::SimpleHandler;
/// use reinhardt_http::{Request, Response};
/// use reinhardt_http::Handler;
///
/// let handler = SimpleHandler::new(|req: Request| {
///     Ok(Response::ok().with_body("Hello, World!"))
/// });
///
/// // Use handler in tests
/// ```
///
/// ## With path-based routing
///
/// ```no_run
/// use reinhardt_testkit::mock::SimpleHandler;
/// use reinhardt_http::{Request, Response};
///
/// let handler = SimpleHandler::new(|req: Request| {
///     match req.path() {
///         "/" => Ok(Response::ok().with_body("Home")),
///         "/api" => Ok(Response::ok().with_body(r#"{"status": "ok"}"#)),
///         _ => Ok(Response::not_found().with_body("Not Found")),
///     }
/// });
/// ```
///
/// ## With custom logic
///
/// ```no_run
/// use reinhardt_testkit::mock::SimpleHandler;
/// use reinhardt_http::{Request, Response};
/// use std::sync::{Arc, Mutex};
///
/// let call_count = Arc::new(Mutex::new(0));
/// let call_count_clone = call_count.clone();
///
/// let handler = SimpleHandler::new(move |req: Request| {
///     let mut count = call_count_clone.lock().unwrap();
///     *count += 1;
///     Ok(Response::ok().with_body(format!("Call count: {}", *count)))
/// });
/// ```
pub struct SimpleHandler<F>
where
	F: Fn(reinhardt_http::Request) -> reinhardt_http::Result<reinhardt_http::Response>
		+ Send
		+ Sync
		+ 'static,
{
	handler_fn: F,
}

impl<F> SimpleHandler<F>
where
	F: Fn(reinhardt_http::Request) -> reinhardt_http::Result<reinhardt_http::Response>
		+ Send
		+ Sync
		+ 'static,
{
	/// Create a new SimpleHandler with the given handler function
	///
	/// # Arguments
	///
	/// * `handler_fn` - A closure that processes requests and returns responses
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_testkit::mock::SimpleHandler;
	/// use reinhardt_http::{Request, Response};
	///
	/// let handler = SimpleHandler::new(|req| {
	///     Ok(Response::ok().with_body("Success"))
	/// });
	/// ```
	pub fn new(handler_fn: F) -> Self {
		Self { handler_fn }
	}
}

#[async_trait::async_trait]
impl<F> reinhardt_http::Handler for SimpleHandler<F>
where
	F: Fn(reinhardt_http::Request) -> reinhardt_http::Result<reinhardt_http::Response>
		+ Send
		+ Sync
		+ 'static,
{
	async fn handle(
		&self,
		request: reinhardt_http::Request,
	) -> reinhardt_http::Result<reinhardt_http::Response> {
		(self.handler_fn)(request)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_mock_function() {
		let mock = MockFunction::<i32>::new();

		mock.returns(42).await;
		mock.returns(100).await;

		let result1 = mock.call(vec![serde_json::json!(1)]).await;
		assert_eq!(result1, Some(42));

		let result2 = mock.call(vec![serde_json::json!(2)]).await;
		assert_eq!(result2, Some(100));

		assert_eq!(mock.call_count().await, 2);
		assert!(mock.was_called().await);
	}

	#[tokio::test]
	async fn test_mock_default() {
		let mock = MockFunction::with_default(99);

		let result = mock.call(vec![]).await;
		assert_eq!(result, Some(99));
	}

	#[tokio::test]
	async fn test_spy() {
		use serde_json::json;

		let spy: Spy<String> = Spy::new();

		spy.record_call(vec![json!("arg1")]).await;
		spy.record_call(vec![json!("arg2")]).await;

		assert_eq!(spy.call_count().await, 2);
		assert!(spy.was_called().await);
		assert!(spy.was_called_with(vec![json!("arg1")]).await);
	}

	#[tokio::test]
	async fn test_mock_reset() {
		let mock = MockFunction::<i32>::new();
		mock.call(vec![]).await;
		assert_eq!(mock.call_count().await, 1);

		mock.reset().await;
		assert_eq!(mock.call_count().await, 0);
	}
}
