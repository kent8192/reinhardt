//! Request recording and query/assertion API.

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use super::matcher::UrlMatcher;

/// A recorded intercepted request.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
	/// The request URL.
	pub url: String,
	/// The HTTP method (GET, POST, etc.).
	pub method: String,
	/// Request headers.
	pub headers: HashMap<String, String>,
	/// Request body, if any.
	pub body: Option<String>,
	/// Timestamp (milliseconds since epoch).
	pub timestamp: f64,
}

/// Internal store for recorded requests.
pub(crate) struct RequestRecorder {
	calls: Vec<RecordedRequest>,
}

impl RequestRecorder {
	pub(crate) fn new() -> Self {
		Self { calls: Vec::new() }
	}

	pub(crate) fn record(&mut self, request: RecordedRequest) {
		self.calls.push(request);
	}

	pub(crate) fn all(&self) -> &[RecordedRequest] {
		&self.calls
	}

	pub(crate) fn clear(&mut self) {
		self.calls.clear();
	}
}

/// Filtered query over recorded requests.
pub struct CallQuery<'a> {
	recorder: &'a RefCell<RequestRecorder>,
	matcher: UrlMatcher,
}

impl<'a> CallQuery<'a> {
	pub(crate) fn new(
		recorder: &'a RefCell<RequestRecorder>,
		pattern: impl Into<UrlMatcher>,
	) -> Self {
		Self {
			recorder,
			matcher: pattern.into(),
		}
	}

	fn filtered(&self) -> Vec<RecordedRequest> {
		self.recorder
			.borrow()
			.all()
			.iter()
			.filter(|r| self.matcher.matches(&r.url))
			.cloned()
			.collect()
	}

	/// Number of matching calls.
	pub fn count(&self) -> usize {
		self.filtered().len()
	}

	/// First matching call.
	pub fn first(&self) -> Option<RecordedRequest> {
		self.filtered().into_iter().next()
	}

	/// Last matching call.
	pub fn last(&self) -> Option<RecordedRequest> {
		self.filtered().into_iter().last()
	}

	/// All matching calls.
	pub fn all(&self) -> Vec<RecordedRequest> {
		self.filtered()
	}

	/// Nth matching call (0-indexed).
	pub fn nth(&self, n: usize) -> Option<RecordedRequest> {
		self.filtered().into_iter().nth(n)
	}

	/// Assert at least one matching call was recorded.
	pub fn assert_called(&self) {
		assert!(
			self.count() > 0,
			"Expected at least one call matching {:?}, but found none",
			self.matcher
		);
	}

	/// Assert no matching calls were recorded.
	pub fn assert_not_called(&self) {
		let count = self.count();
		assert!(
			count == 0,
			"Expected no calls matching {:?}, but found {}",
			self.matcher,
			count
		);
	}

	/// Assert exactly N matching calls.
	pub fn assert_count(&self, expected: usize) {
		let actual = self.count();
		assert_eq!(
			actual, expected,
			"Expected {} calls matching {:?}, but found {}",
			expected, self.matcher, actual
		);
	}
}

/// Type-safe call query for server_fn calls with Args deserialization.
///
/// Created via `MockServiceWorker::calls_to_server_fn`.
pub struct ServerFnCallQuery<'a, S> {
	pub(crate) inner: CallQuery<'a>,
	pub(crate) _marker: PhantomData<S>,
}

use reinhardt_pages::server_fn::MockableServerFn;

impl<'a, S: MockableServerFn> ServerFnCallQuery<'a, S> {
	/// Number of matching calls.
	pub fn count(&self) -> usize {
		self.inner.count()
	}

	/// Assert at least one call was recorded.
	pub fn assert_called(&self) {
		self.inner.assert_called();
	}

	/// Assert no calls were recorded.
	pub fn assert_not_called(&self) {
		self.inner.assert_not_called();
	}

	/// Assert exactly N calls.
	pub fn assert_count(&self, expected: usize) {
		self.inner.assert_count(expected);
	}

	/// Deserialize the last call's body into the server function's `Args` type.
	pub fn last_args(&self) -> Option<S::Args> {
		let last = self.inner.last()?;
		let body = last.body.as_deref().unwrap_or("{}");
		serde_json::from_str(body).ok()
	}

	/// Assert the last call was made with specific args.
	pub fn assert_called_with(&self, expected: &S::Args)
	where
		S::Args: PartialEq + std::fmt::Debug,
	{
		let actual = self
			.last_args()
			.expect("assert_called_with: no calls recorded");
		assert_eq!(actual, *expected, "assert_called_with: args do not match");
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	fn make_request(url: &str, method: &str) -> RecordedRequest {
		RecordedRequest {
			url: url.to_string(),
			method: method.to_string(),
			headers: HashMap::new(),
			body: None,
			timestamp: 0.0,
		}
	}

	#[rstest]
	fn recorder_records_and_counts() {
		let recorder = RefCell::new(RequestRecorder::new());
		recorder
			.borrow_mut()
			.record(make_request("/api/users", "GET"));
		recorder
			.borrow_mut()
			.record(make_request("/api/users", "POST"));
		recorder
			.borrow_mut()
			.record(make_request("/api/posts", "GET"));

		let query = CallQuery::new(&recorder, "/api/users");
		assert_eq!(query.count(), 2);
	}

	#[rstest]
	fn call_query_first_and_last() {
		let recorder = RefCell::new(RequestRecorder::new());
		recorder.borrow_mut().record(make_request("/api/a", "GET"));
		recorder.borrow_mut().record(make_request("/api/a", "POST"));

		let query = CallQuery::new(&recorder, "/api/a");
		assert_eq!(query.first().unwrap().method, "GET");
		assert_eq!(query.last().unwrap().method, "POST");
	}

	#[rstest]
	fn assert_called_succeeds() {
		let recorder = RefCell::new(RequestRecorder::new());
		recorder.borrow_mut().record(make_request("/api/x", "GET"));
		CallQuery::new(&recorder, "/api/x").assert_called();
	}

	#[rstest]
	#[should_panic(expected = "Expected at least one call")]
	fn assert_called_fails() {
		let recorder = RefCell::new(RequestRecorder::new());
		CallQuery::new(&recorder, "/api/x").assert_called();
	}

	#[rstest]
	fn assert_not_called_succeeds() {
		let recorder = RefCell::new(RequestRecorder::new());
		CallQuery::new(&recorder, "/api/x").assert_not_called();
	}

	#[rstest]
	fn assert_count_succeeds() {
		let recorder = RefCell::new(RequestRecorder::new());
		recorder.borrow_mut().record(make_request("/api/x", "GET"));
		recorder.borrow_mut().record(make_request("/api/x", "GET"));
		CallQuery::new(&recorder, "/api/x").assert_count(2);
	}

	#[rstest]
	fn recorder_clear() {
		let recorder = RefCell::new(RequestRecorder::new());
		recorder.borrow_mut().record(make_request("/api/x", "GET"));
		recorder.borrow_mut().clear();
		assert!(recorder.borrow().all().is_empty());
	}
}
