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
type CompletionHook = Rc<dyn Fn()>;
type RedirectErrorCallback = Rc<dyn Fn(&crate::NavigateError)>;

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
	exact_invalidations: Vec<CompletionHook>,
	family_invalidations: Vec<CompletionHook>,
	redirect: Option<String>,
	on_redirect_error: Vec<RedirectErrorCallback>,
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
		exact_invalidations: Vec::new(),
		family_invalidations: Vec::new(),
		redirect: None,
		on_redirect_error: Vec::new(),
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

	pub fn invalidate<T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		key: crate::QueryKey<T, E>,
	) -> Self {
		self.exact_invalidations
			.push(Rc::new(move || client.invalidate(&key)));
		self
	}

	pub fn invalidate_family<Args: 'static, T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		family: crate::QueryFamily<Args, T, E>,
	) -> Self {
		self.family_invalidations
			.push(Rc::new(move || client.invalidate_family(family.clone())));
		self
	}

	pub fn redirect(mut self, path: impl Into<String>) -> Self {
		self.redirect = Some(path.into());
		self
	}

	pub fn on_redirect_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&crate::NavigateError) + 'static,
	{
		self.on_redirect_error.push(Rc::new(callback));
		self
	}

	/// Builds the configured mutation handle.
	pub fn build(self) -> ServerMutation<Input, Output> {
		let ServerMutationBuilder {
			action_fn,
			on_success,
			on_error,
			exact_invalidations,
			family_invalidations,
			redirect,
			on_redirect_error,
		} = self;
		let action = use_action(move |input: Input| (action_fn)(input))
			.on_success(move |output| {
				for callback in &on_success {
					callback(output);
				}
				for callback in &exact_invalidations {
					callback();
				}
				for callback in &family_invalidations {
					callback();
				}
				if let Some(path) = &redirect {
					if let Err(error) =
						crate::navigate_or_reload(path.clone(), crate::NavigationType::Push)
					{
						crate::error_log!("server mutation redirect failed: {error}");
						for callback in &on_redirect_error {
							callback(&error);
						}
					}
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

	#[cfg(test)]
	pub(crate) fn force_success_for_test(&self, value: Output) {
		self.action.force_success_for_test(value);
	}

	#[cfg(test)]
	pub(crate) fn force_error_for_test(&self, error: ServerFnError) {
		self.action.force_error_for_test(error);
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
	use std::cell::{Cell, RefCell};
	use std::rc::Rc;

	use reinhardt_core::reactive::ReactiveScope;
	use rstest::rstest;

	use super::{ActionPhase, MutationDispatchOutcome, use_server_mutation};
	use super::{ServerMutation, execute_server_mutation_once};
	use crate::{QueryClient, QueryDefaults, QueryFamily, ServerFnError};

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

	#[rstest]
	fn success_hooks_run_in_fixed_order_and_redirect_failure_preserves_success() {
		ReactiveScope::run(|| {
			let order = Rc::new(RefCell::new(Vec::new()));
			let order_for_success = Rc::clone(&order);
			let order_for_exact = Rc::clone(&order);
			let order_for_family = Rc::clone(&order);
			let order_for_redirect_error = Rc::clone(&order);
			let client = QueryClient::new_ssr(QueryDefaults::default());
			let family = QueryFamily::<(), i32, ServerFnError>::new("test.server-mutation");
			let mut builder =
				use_server_mutation(|value: i32| async move { Ok::<i32, ServerFnError>(value) })
					.on_success(move |_| {
						order_for_success.borrow_mut().push("user-success");
					})
					.redirect("/without-a-router")
					.on_redirect_error(move |_| {
						order_for_redirect_error.borrow_mut().push("redirect-error");
					})
					.invalidate(client, family.key(()))
					.invalidate_family(client, family);
			builder.exact_invalidations.clear();
			builder.family_invalidations.clear();
			builder.exact_invalidations.push(Rc::new(move || {
				order_for_exact.borrow_mut().push("exact");
			}));
			builder.family_invalidations.push(Rc::new(move || {
				order_for_family.borrow_mut().push("family");
			}));
			let mutation = builder.build();

			mutation.force_success_for_test(11);

			assert_eq!(
				order.borrow().as_slice(),
				["user-success", "exact", "family", "redirect-error"]
			);
			assert_eq!(mutation.phase(), ActionPhase::Success(11));
		});
	}

	#[rstest]
	fn error_callbacks_run_without_success_hooks() {
		ReactiveScope::run(|| {
			let events = Rc::new(RefCell::new(Vec::new()));
			let events_for_error = Rc::clone(&events);
			let events_for_success = Rc::clone(&events);
			let mutation =
				use_server_mutation(|value: i32| async move { Ok::<i32, ServerFnError>(value) })
					.on_success(move |_| {
						events_for_success.borrow_mut().push("success");
					})
					.on_error(move |error| {
						assert_eq!(error, &ServerFnError::application("save failed"));
						events_for_error.borrow_mut().push("error");
					})
					.build();

			mutation.force_error_for_test(ServerFnError::application("save failed"));

			assert_eq!(events.borrow().as_slice(), ["error"]);
		});
	}
}
