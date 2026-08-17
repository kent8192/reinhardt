//! In-process server function mocks for native component tests.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::server_fn::{MockableServerFn, ServerFnError};

type ErasedHandler = Rc<dyn Fn(Box<dyn Any>) -> Result<Box<dyn Any>, ServerFnError>>;

#[derive(Default)]
pub(crate) struct ServerFnMockRegistry {
	handlers: HashMap<TypeId, ErasedHandler>,
	calls: HashMap<TypeId, Vec<Vec<u8>>>,
}

#[derive(Clone)]
pub(crate) struct SharedServerFnMocks {
	inner: Rc<RefCell<ServerFnMockRegistry>>,
}

impl Default for SharedServerFnMocks {
	fn default() -> Self {
		Self {
			inner: Rc::new(RefCell::new(ServerFnMockRegistry::default())),
		}
	}
}

thread_local! {
	static ACTIVE_MOCKS: RefCell<Option<SharedServerFnMocks>> = const { RefCell::new(None) };
}

pub(crate) struct ServerFnMockScope {
	previous: Option<SharedServerFnMocks>,
}

impl Drop for ServerFnMockScope {
	fn drop(&mut self) {
		let previous = self.previous.take();
		ACTIVE_MOCKS.with(|slot| {
			*slot.borrow_mut() = previous;
		});
	}
}

pub(crate) fn activate(mocks: SharedServerFnMocks) -> ServerFnMockScope {
	let previous = ACTIVE_MOCKS.with(|slot| slot.borrow_mut().replace(mocks));
	ServerFnMockScope { previous }
}

pub(crate) fn with_active<R>(mocks: SharedServerFnMocks, f: impl FnOnce() -> R) -> R {
	let _scope = activate(mocks);
	f()
}

/// Returns whether the current thread has an active server-function mock scope.
pub fn has_active_scope() -> bool {
	ACTIVE_MOCKS.with(|slot| slot.borrow().is_some())
}

/// Calls the active native test mock for `S`, recording the typed arguments.
pub fn try_call_active_mock<S>(args: S::Args) -> Option<Result<S::Response, ServerFnError>>
where
	S: MockableServerFn + 'static,
	S::Args: 'static,
	S::Response: 'static,
{
	ACTIVE_MOCKS.with(|slot| {
		let mocks = slot.borrow().clone()?;
		let type_id = TypeId::of::<S>();
		let recorded_args = match serde_json::to_vec(&args) {
			Ok(recorded_args) => recorded_args,
			Err(error) => {
				return Some(Err(ServerFnError::application(format!(
					"failed to record mock arguments: {error}"
				))));
			}
		};
		let handler = {
			let mut registry = mocks.inner.borrow_mut();
			registry
				.calls
				.entry(type_id)
				.or_default()
				.push(recorded_args);
			registry.handlers.get(&type_id).cloned()
		};
		let Some(handler) = handler else {
			return Some(Err(ServerFnError::application(
				"no mock registered for active server function",
			)));
		};
		let response = handler(Box::new(args));
		Some(response.and_then(|value| {
			value
				.downcast::<S::Response>()
				.map(|boxed| *boxed)
				.map_err(|_| ServerFnError::application("mock response type mismatch"))
		}))
	})
}

impl SharedServerFnMocks {
	pub(crate) fn mock_server_fn<S>(
		&self,
		handler: impl Fn(S::Args) -> Result<S::Response, ServerFnError> + 'static,
	) where
		S: MockableServerFn + 'static,
		S::Args: 'static,
		S::Response: 'static,
	{
		self.inner.borrow_mut().handlers.insert(
			TypeId::of::<S>(),
			Rc::new(move |args| {
				let args = args
					.downcast::<S::Args>()
					.map_err(|_| ServerFnError::application("mock args type mismatch"))?;
				handler(*args).map(|response| Box::new(response) as Box<dyn Any>)
			}),
		);
	}

	pub(crate) fn calls_to_server_fn<S>(&self) -> ServerFnCallQuery<S>
	where
		S: MockableServerFn + 'static,
		S::Args: 'static,
	{
		let calls = self
			.inner
			.borrow()
			.calls
			.get(&TypeId::of::<S>())
			.map(|values| {
				values
					.iter()
					.filter_map(|value| serde_json::from_slice::<S::Args>(value).ok())
					.map(|args| RecordedServerFnCall {
						path: S::PATH.to_string(),
						args,
						_marker: PhantomData,
					})
					.collect()
			})
			.unwrap_or_default();
		ServerFnCallQuery { calls }
	}
}

/// Recorded server function call.
pub struct RecordedServerFnCall<S: MockableServerFn> {
	/// Server function path.
	pub path: String,
	/// Typed argument payload passed to the server function.
	pub args: S::Args,
	_marker: PhantomData<S>,
}

impl<S> Clone for RecordedServerFnCall<S>
where
	S: MockableServerFn,
	S::Args: Clone,
{
	fn clone(&self) -> Self {
		Self {
			path: self.path.clone(),
			args: self.args.clone(),
			_marker: PhantomData,
		}
	}
}

/// Queryable collection of recorded server function calls.
pub struct ServerFnCallQuery<S: MockableServerFn> {
	calls: Vec<RecordedServerFnCall<S>>,
}

impl<S> Clone for ServerFnCallQuery<S>
where
	S: MockableServerFn,
	S::Args: Clone,
{
	fn clone(&self) -> Self {
		Self {
			calls: self.calls.clone(),
		}
	}
}

impl<S: MockableServerFn> ServerFnCallQuery<S> {
	/// Returns the number of recorded calls.
	pub fn len(&self) -> usize {
		self.calls.len()
	}

	/// Returns true when no calls were recorded.
	pub fn is_empty(&self) -> bool {
		self.calls.is_empty()
	}

	/// Returns all recorded calls.
	pub fn all(&self) -> &[RecordedServerFnCall<S>] {
		&self.calls
	}
}
