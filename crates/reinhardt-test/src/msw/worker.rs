//! `MockServiceWorker` — the core orchestrator for MSW-style fetch interception.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js_sys::Promise;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

use reinhardt_pages::server_fn::{MockableServerFn, ServerFnError};

use super::context::TestContext;
use super::handler::RestHandler;
use super::handler::{ErasedHandler, ServerFnContextHandler, ServerFnHandler};
use super::interceptor;
use super::matcher::UrlMatcher;
use super::recorder::{CallQuery, RecordedRequest, RequestRecorder, ServerFnCallQuery};

// Global guard to prevent multiple concurrent MockServiceWorker instances from
// overriding `window.fetch`. Without this, a second worker would capture the
// *already-overridden* fetch as its "original", leading to incorrect restoration.
thread_local! {
	static ACTIVE_WORKER_COUNT: Cell<u32> = const { Cell::new(0) };
}

/// Policy for requests that don't match any registered handler.
#[derive(Debug, Clone)]
pub enum UnhandledPolicy {
	/// Return a network error with a descriptive message (default).
	/// Ensures test determinism by preventing accidental real network calls.
	Error,
	/// Pass through to the original `window.fetch`.
	Passthrough,
	/// Log a warning to `console.warn` and pass through.
	Warn,
}

impl From<&UnhandledPolicy> for interceptor::UnhandledPolicy {
	fn from(p: &UnhandledPolicy) -> Self {
		match p {
			UnhandledPolicy::Error => interceptor::UnhandledPolicy::Error,
			UnhandledPolicy::Passthrough => interceptor::UnhandledPolicy::Passthrough,
			UnhandledPolicy::Warn => interceptor::UnhandledPolicy::Warn,
		}
	}
}

/// MSW-style network-level request interceptor for WASM testing.
///
/// Overrides `window.fetch` to intercept HTTP requests and return mock
/// responses. Supports type-safe `server_fn` mocking, REST endpoint
/// mocking, request recording, and assertion helpers.
///
/// # Example
///
/// ```rust,ignore
/// let worker = MockServiceWorker::new();
/// worker.handle(rest::get("/api/data").respond(MockResponse::json(42)));
/// worker.start().await;
/// // ... test ...
/// worker.calls_to("/api/data").assert_called();
/// // worker.stop() called automatically on drop
/// ```
pub struct MockServiceWorker {
	handlers: Rc<RefCell<Vec<Box<dyn ErasedHandler>>>>,
	recorder: Rc<RefCell<RequestRecorder>>,
	unhandled_policy: UnhandledPolicy,
	active: Cell<bool>,
	original_fetch: RefCell<Option<JsValue>>,
	#[allow(clippy::type_complexity)]
	// Store the closure to prevent deallocation while the override is active
	closure: RefCell<Option<Closure<dyn FnMut(JsValue, JsValue) -> Promise>>>,
}

impl MockServiceWorker {
	/// Create a new worker with `UnhandledPolicy::Error` (default).
	pub fn new() -> Self {
		Self::with_policy(UnhandledPolicy::Error)
	}

	/// Create a new worker with a custom unhandled request policy.
	pub fn with_policy(policy: UnhandledPolicy) -> Self {
		Self {
			handlers: Rc::new(RefCell::new(Vec::new())),
			recorder: Rc::new(RefCell::new(RequestRecorder::new())),
			unhandled_policy: policy,
			active: Cell::new(false),
			original_fetch: RefCell::new(None),
			closure: RefCell::new(None),
		}
	}

	/// Install the fetch override. Must be called before component rendering.
	///
	/// # Panics
	///
	/// Panics if already started or if `window` is unavailable.
	pub async fn start(&self) {
		assert!(
			!self.active.get(),
			"MockServiceWorker: already started. Call stop() before starting again."
		);

		ACTIVE_WORKER_COUNT.with(|count| {
			assert!(
				count.get() == 0,
				"MockServiceWorker: another worker is already active. \
				 Only one MockServiceWorker can override window.fetch at a time. \
				 Call stop() on the existing worker first."
			);
			count.set(count.get() + 1);
		});

		let original = interceptor::save_original_fetch();
		let closure = interceptor::install_fetch_override(
			self.handlers.clone(),
			self.recorder.clone(),
			(&self.unhandled_policy).into(),
			original.clone(),
		);

		*self.original_fetch.borrow_mut() = Some(original);
		*self.closure.borrow_mut() = Some(closure);
		self.active.set(true);
	}

	/// Restore original `window.fetch` and clean up.
	pub async fn stop(&self) {
		if self.active.get() {
			if let Some(original) = self.original_fetch.borrow().as_ref() {
				interceptor::restore_fetch(original);
			}
			self.closure.borrow_mut().take();
			self.active.set(false);
			ACTIVE_WORKER_COUNT.with(|count| {
				count.set(count.get().saturating_sub(1));
			});
		}
	}

	/// Remove all handlers and recorded requests.
	pub fn reset(&self) {
		self.handlers.borrow_mut().clear();
		self.recorder.borrow_mut().clear();
	}

	/// Remove all handlers but keep recorded requests.
	pub fn reset_handlers(&self) {
		self.handlers.borrow_mut().clear();
	}

	// --- REST handler registration ---

	/// Register a REST handler.
	pub fn handle(&self, handler: RestHandler) {
		self.handlers.borrow_mut().push(Box::new(handler));
	}

	// --- server_fn handler registration ---

	/// Register a type-safe server_fn handler.
	pub fn handle_server_fn<S: MockableServerFn>(
		&self,
		handler: impl Fn(S::Args) -> Result<S::Response, ServerFnError> + 'static,
	) {
		self.handlers
			.borrow_mut()
			.push(Box::new(ServerFnHandler::<S>::new(
				Box::new(handler),
				false,
				None,
			)));
	}

	/// Register a server_fn handler with a DI test context.
	pub fn handle_server_fn_with_context<S: MockableServerFn>(
		&self,
		context: TestContext,
		handler: impl Fn(S::Args, &TestContext) -> Result<S::Response, ServerFnError> + 'static,
	) {
		self.handlers
			.borrow_mut()
			.push(Box::new(ServerFnContextHandler::<S>::new(
				context,
				Box::new(handler),
				false,
				None,
			)));
	}

	// --- Query API ---

	/// Query recorded calls matching a URL pattern.
	pub fn calls_to(&self, pattern: impl Into<UrlMatcher>) -> CallQuery<'_> {
		CallQuery::new(&self.recorder, pattern)
	}

	/// Query recorded calls to a specific server function (type-safe).
	pub fn calls_to_server_fn<S: MockableServerFn>(&self) -> ServerFnCallQuery<'_, S> {
		ServerFnCallQuery {
			inner: CallQuery::new(&self.recorder, S::PATH),
			_marker: std::marker::PhantomData,
		}
	}

	/// All recorded calls.
	pub fn all_calls(&self) -> Vec<RecordedRequest> {
		self.recorder.borrow().all().to_vec()
	}
}

impl Drop for MockServiceWorker {
	fn drop(&mut self) {
		if self.active.get() {
			if let Some(original) = self.original_fetch.borrow().as_ref() {
				interceptor::restore_fetch(original);
			}
			ACTIVE_WORKER_COUNT.with(|count| {
				count.set(count.get().saturating_sub(1));
			});
		}
	}
}
