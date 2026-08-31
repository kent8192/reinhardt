//! Target-neutral server mutation runtime.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;

use crate::ServerFnError;
use crate::reactive::{Action, ActionPhase, use_action};

type ServerMutationFuture<Output> = Pin<Box<dyn Future<Output = Result<Output, ServerFnError>>>>;
type ServerMutationFn<Input, Output> = Rc<dyn Fn(Input) -> ServerMutationFuture<Output>>;
type SuccessCallback<Output> = Rc<dyn Fn(&Output)>;
type ErrorCallback = Rc<dyn Fn(&ServerFnError)>;

/// Outcome returned by [`ServerMutation::dispatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationDispatchOutcome {
	/// The mutation was dispatched.
	Dispatched,
	/// The mutation was already pending and no new dispatch occurred.
	AlreadyPending,
	/// Validation failed before dispatch could start.
	ValidationFailed,
	/// The current target does not execute the mutation closure.
	UnsupportedTarget,
}

/// Builder for a target-neutral server mutation.
pub struct ServerMutationBuilder<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	action_fn: ServerMutationFn<Input, Output>,
	on_success: Vec<SuccessCallback<Output>>,
	on_error: Vec<ErrorCallback>,
}

/// Handle for observing and dispatching a target-neutral server mutation.
pub struct ServerMutation<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	action: Action<Output, ServerFnError>,
	_input: PhantomData<fn(Input)>,
}

impl<Input, Output> Clone for ServerMutation<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	fn clone(&self) -> Self {
		*self
	}
}

impl<Input, Output> Copy for ServerMutation<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
}

/// Creates a builder for a target-neutral server mutation.
///
/// Errors produced by the action are normalized into [`ServerFnError`].
pub fn use_server_mutation<Input, Output, Error, ActionFn, ActionFuture>(
	action_fn: ActionFn,
) -> ServerMutationBuilder<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
	Error: Into<ServerFnError> + 'static,
	ActionFn: Fn(Input) -> ActionFuture + 'static,
	ActionFuture: Future<Output = Result<Output, Error>> + 'static,
{
	let action_fn = Rc::new(action_fn);
	ServerMutationBuilder {
		action_fn: Rc::new(move |input| {
			let future = action_fn(input);
			let future: ServerMutationFuture<Output> =
				Box::pin(async move { future.await.map_err(Into::into) });
			future
		}),
		on_success: Vec::new(),
		on_error: Vec::new(),
	}
}

impl<Input, Output> ServerMutationBuilder<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	/// Registers a callback that runs after a successful mutation.
	pub fn on_success<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&Output) + 'static,
	{
		self.on_success.push(Rc::new(callback));
		self
	}

	/// Registers a callback that runs after a failed mutation.
	pub fn on_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&ServerFnError) + 'static,
	{
		self.on_error.push(Rc::new(callback));
		self
	}

	/// Builds the configured mutation handle.
	pub fn build(self) -> ServerMutation<Input, Output> {
		let ServerMutationBuilder {
			action_fn,
			on_success,
			on_error,
		} = self;
		let action = use_action(move |input: Input| (action_fn)(input))
			.on_success(move |output| {
				for callback in &on_success {
					callback(output);
				}
			})
			.on_error(move |error| {
				for callback in &on_error {
					callback(error);
				}
			});
		ServerMutation {
			action,
			_input: PhantomData,
		}
	}
}

impl<Input, Output> ServerMutation<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	/// Returns the current mutation phase.
	pub fn phase(&self) -> ActionPhase<Output, ServerFnError> {
		self.action.phase()
	}

	/// Returns `true` when the mutation is pending.
	pub fn is_pending(&self) -> bool {
		self.action.is_pending()
	}

	/// Returns `true` when the mutation completed successfully.
	pub fn is_success(&self) -> bool {
		self.action.is_success()
	}

	/// Returns the latest successful result, if any.
	pub fn result(&self) -> Option<Output> {
		self.action.result()
	}

	/// Returns the latest error, if any.
	pub fn error(&self) -> Option<ServerFnError> {
		self.action.error()
	}

	/// Resets the mutation back to `Idle`.
	pub fn reset(&self) {
		self.action.reset();
	}

	/// Dispatches the mutation.
	///
	/// On native and SSR targets, this is inert and returns
	/// [`MutationDispatchOutcome::UnsupportedTarget`] without invoking the
	/// action closure.
	pub fn dispatch(&self, input: Input) -> MutationDispatchOutcome {
		#[cfg(wasm)]
		{
			if self.action.is_pending() {
				return MutationDispatchOutcome::AlreadyPending;
			}
			self.action.dispatch(input);
			MutationDispatchOutcome::Dispatched
		}

		#[cfg(native)]
		{
			drop(input);
			MutationDispatchOutcome::UnsupportedTarget
		}
	}
}

/// Executes one server mutation action and normalizes the error into [`ServerFnError`].
#[doc(hidden)]
pub async fn execute_server_mutation_once<Input, Output, Error, ActionFn, ActionFuture>(
	input: Input,
	action_fn: ActionFn,
) -> Result<Output, ServerFnError>
where
	Error: Into<ServerFnError>,
	ActionFn: FnOnce(Input) -> ActionFuture,
	ActionFuture: Future<Output = Result<Output, Error>>,
{
	action_fn(input).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
	use std::cell::Cell;
	use std::rc::Rc;

	use reinhardt_core::reactive::ReactiveScope;
	use rstest::rstest;

	use super::{ActionPhase, MutationDispatchOutcome, use_server_mutation};
	use super::{ServerMutation, execute_server_mutation_once};
	use crate::ServerFnError;

	#[derive(Debug)]
	struct DemoError;

	impl From<DemoError> for ServerFnError {
		fn from(_: DemoError) -> Self {
			ServerFnError::application("demo")
		}
	}

	#[rstest]
	fn native_dispatch_is_inert() {
		ReactiveScope::run(|| {
			let calls = Rc::new(Cell::new(0));
			let calls_for_action = Rc::clone(&calls);
			let mutation = use_server_mutation(move |value: i32| {
				calls_for_action.set(calls_for_action.get() + 1);
				async move { Ok::<i32, ServerFnError>(value + 1) }
			})
			.build();

			assert_eq!(
				mutation.dispatch(7),
				MutationDispatchOutcome::UnsupportedTarget
			);
			assert_eq!(mutation.phase(), ActionPhase::Idle);
			assert_eq!(calls.get(), 0);
		});
	}

	#[rstest]
	fn tuple_input_is_retained_by_the_public_handle() {
		fn assert_type(_: &ServerMutation<(String, bool), usize>) {}

		ReactiveScope::run(|| {
			let mutation = use_server_mutation(|(name, force): (String, bool)| async move {
				Ok::<usize, ServerFnError>(name.len() + usize::from(force))
			})
			.build();
			assert_type(&mutation);
		});
	}

	#[rstest]
	fn custom_errors_are_normalized() {
		let result = tokio_test::block_on(execute_server_mutation_once(7, |value| async move {
			if value > 0 {
				Err::<i32, DemoError>(DemoError)
			} else {
				Ok::<i32, DemoError>(value)
			}
		}));

		assert_eq!(result, Err(ServerFnError::application("demo")));
	}

	#[rstest]
	fn result_survives_until_reset() {
		ReactiveScope::run(|| {
			let mutation =
				use_server_mutation(
					|value: i32| async move { Ok::<i32, ServerFnError>(value + 1) },
				)
				.build();

			mutation.action.force_success_for_test(8);

			assert_eq!(mutation.phase(), ActionPhase::Success(8));
			assert_eq!(mutation.result(), Some(8));
			assert!(mutation.is_success());
			assert_eq!(mutation.result(), Some(8));

			mutation.reset();

			assert_eq!(mutation.phase(), ActionPhase::Idle);
			assert_eq!(mutation.result(), None);
		});
	}
}
