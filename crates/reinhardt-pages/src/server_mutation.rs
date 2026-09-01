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
type FormPrepare<Form, Deps, Input> = Rc<
	dyn Fn(
		&crate::UseFormReturn<Form, Deps>,
	)
		-> Result<Input, crate::FormValidationError<<Form as crate::FormRuntimeSource>::Field>>,
>;

/// Outcome returned by [`ServerMutation::dispatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationDispatchOutcome {
	/// The mutation was dispatched.
	Dispatched,
	/// The mutation was pending or no longer live, so no new dispatch occurred.
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
	Output: 'static,
{
	action_fn: ServerMutationFn<Input, Output>,
	before_success: Vec<SuccessCallback<Output>>,
	on_success: Vec<SuccessCallback<Output>>,
	after_success: Vec<SuccessCallback<Output>>,
	before_error: Vec<ErrorCallback>,
	on_error: Vec<ErrorCallback>,
	exact_invalidations: Vec<CompletionHook>,
	family_invalidations: Vec<CompletionHook>,
	redirect: Option<String>,
	on_redirect_error: Vec<RedirectErrorCallback>,
}

/// Builder for a generated-form server mutation adapter.
pub struct FormServerMutationBuilder<Form, Deps, Input, Output>
where
	Form: crate::FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	Input: 'static,
	Output: 'static,
{
	builder: ServerMutationBuilder<Input, Output>,
	form: crate::UseFormReturn<Form, Deps>,
	prepare: FormPrepare<Form, Deps, Input>,
	reset_form_on_success: bool,
}

/// Handle for observing and dispatching a target-neutral server mutation.
pub struct ServerMutation<Input, Output>
where
	Input: 'static,
	Output: Clone + 'static,
{
	action: Action<Output, ServerFnError>,
	last_success: crate::Signal<Option<Output>>,
	_input: PhantomData<fn(Input)>,
}

/// Handle for observing and dispatching a generated-form server mutation.
pub struct FormServerMutation<Form, Deps, Input, Output>
where
	Form: crate::FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	Input: 'static,
	Output: Clone + 'static,
{
	form: crate::UseFormReturn<Form, Deps>,
	prepare: FormPrepare<Form, Deps, Input>,
	mutation: ServerMutation<Input, Output>,
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
	Output: 'static,
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
		before_success: Vec::new(),
		on_success: Vec::new(),
		after_success: Vec::new(),
		before_error: Vec::new(),
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
	/// Registers a callback that runs after a failed mutation.
	pub fn on_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&ServerFnError) + 'static,
	{
		self.on_error.push(Rc::new(callback));
		self
	}

	/// Registers an exact query invalidation that runs after a successful mutation.
	pub fn invalidate<T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		key: crate::QueryKey<T, E>,
	) -> Self {
		self.exact_invalidations
			.push(Rc::new(move || client.invalidate(&key)));
		self
	}

	/// Registers a query-family invalidation that runs after a successful mutation.
	pub fn invalidate_family<Args: 'static, T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		family: crate::QueryFamily<Args, T, E>,
	) -> Self {
		self.family_invalidations
			.push(Rc::new(move || client.invalidate_family(family)));
		self
	}

	/// Registers a client-side redirect that runs after a successful mutation.
	pub fn redirect(mut self, path: impl Into<String>) -> Self {
		self.redirect = Some(path.into());
		self
	}

	/// Registers a callback that observes redirect failures after success hooks run.
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
			before_success,
			on_success,
			after_success,
			before_error,
			on_error,
			exact_invalidations,
			family_invalidations,
			redirect,
			on_redirect_error,
		} = self;
		let last_success = crate::Signal::new(None);
		let last_success_for_success = last_success;
		let action = use_action(move |input: Input| (action_fn)(input))
			.on_success(move |output| {
				last_success_for_success.set(Some(output.clone()));
				for callback in &before_success {
					callback(output);
				}
				for callback in &on_success {
					callback(output);
				}
				for callback in &after_success {
					callback(output);
				}
				for callback in &exact_invalidations {
					callback();
				}
				for callback in &family_invalidations {
					callback();
				}
				if let Some(path) = &redirect
					&& let Err(error) =
						crate::navigate_or_reload(path.clone(), crate::NavigationType::Push)
				{
					crate::error_log!("server mutation redirect failed: {error}");
					for callback in &on_redirect_error {
						callback(&error);
					}
				}
			})
			.on_error(move |error| {
				for callback in &before_error {
					callback(error);
				}
				for callback in &on_error {
					callback(error);
				}
			});
		ServerMutation {
			action,
			last_success,
			_input: PhantomData,
		}
	}
}

impl<Input, Output> ServerMutationBuilder<Input, Output>
where
	Input: 'static,
	Output: 'static,
{
	/// Registers a callback that runs after a successful mutation.
	pub fn on_success<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&Output) + 'static,
	{
		self.on_success.push(Rc::new(callback));
		self
	}

	#[doc(hidden)]
	pub fn with_generated_form<Form, Deps, Prepare>(
		self,
		form: &crate::UseFormReturn<Form, Deps>,
		prepare: Prepare,
	) -> FormServerMutationBuilder<Form, Deps, Input, Output>
	where
		Form: crate::FormRuntimeSource,
		Deps: Clone + PartialEq + 'static,
		Prepare: Fn(
				&crate::UseFormReturn<Form, Deps>,
			) -> Result<Input, crate::FormValidationError<Form::Field>>
			+ 'static,
	{
		FormServerMutationBuilder {
			builder: self,
			form: form.clone(),
			prepare: Rc::new(prepare),
			reset_form_on_success: false,
		}
	}
}

impl<Form, Deps, Input, Output> FormServerMutationBuilder<Form, Deps, Input, Output>
where
	Form: crate::FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	Input: 'static,
	Output: Clone + 'static,
{
	/// Registers a callback that runs after a successful mutation.
	pub fn on_success<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&Output) + 'static,
	{
		self.builder = self.builder.on_success(callback);
		self
	}

	/// Registers a callback that runs after a failed server request.
	pub fn on_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&ServerFnError) + 'static,
	{
		self.builder = self.builder.on_error(callback);
		self
	}

	/// Registers an exact query invalidation that runs after a successful mutation.
	pub fn invalidate<T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		key: crate::QueryKey<T, E>,
	) -> Self {
		self.builder = self.builder.invalidate(client, key);
		self
	}

	/// Registers a query-family invalidation that runs after a successful mutation.
	pub fn invalidate_family<Args: 'static, T: 'static, E: 'static>(
		mut self,
		client: crate::QueryClient,
		family: crate::QueryFamily<Args, T, E>,
	) -> Self {
		self.builder = self.builder.invalidate_family(client, family);
		self
	}

	/// Registers a client-side redirect that runs after a successful mutation.
	pub fn redirect(mut self, path: impl Into<String>) -> Self {
		self.builder = self.builder.redirect(path);
		self
	}

	/// Registers a callback that observes redirect failures after success hooks run.
	pub fn on_redirect_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&crate::NavigateError) + 'static,
	{
		self.builder = self.builder.on_redirect_error(callback);
		self
	}

	/// Resets generated form values back to defaults after a successful mutation.
	pub fn reset_form_on_success(mut self) -> Self {
		self.reset_form_on_success = true;
		self
	}

	/// Builds the configured generated-form mutation handle.
	pub fn build(self) -> FormServerMutation<Form, Deps, Input, Output> {
		let FormServerMutationBuilder {
			mut builder,
			form,
			prepare,
			reset_form_on_success,
		} = self;
		let action_fn = Rc::clone(&builder.action_fn);
		let form_for_pending = form.clone();
		builder.action_fn = Rc::new(move |input| {
			let pending_guard = crate::form_state::SubmitPendingGuard::new(
				form_for_pending.form_state().is_submitting,
			);
			let future = action_fn(input);
			let future: ServerMutationFuture<Output> = Box::pin(async move {
				let _pending_guard = pending_guard;
				future.await
			});
			future
		});
		let form_for_success = form.clone();
		builder.before_success.push(Rc::new(move |_| {
			form_for_success.complete_submit_success();
		}));
		if reset_form_on_success {
			let form_for_reset = form.clone();
			builder.after_success.push(Rc::new(move |_| {
				form_for_reset.reset();
			}));
		}
		let form_for_error = form.clone();
		builder.before_error.push(Rc::new(move |error| {
			form_for_error.complete_mutation_server_error(error);
		}));
		FormServerMutation {
			form,
			prepare,
			mutation: builder.build(),
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
		self.last_success.get()
	}

	/// Returns the latest error, if any.
	pub fn error(&self) -> Option<ServerFnError> {
		self.action.error()
	}

	/// Resets the mutation back to `Idle`.
	pub fn reset(&self) {
		if self.action.try_is_pending_untracked() != Some(false) {
			return;
		}
		self.action.reset();
		self.last_success.set(None);
	}

	#[cfg(wasm)]
	fn reset_action_preserving_result(&self) {
		if self.action.try_is_pending_untracked() == Some(false) {
			self.action.reset();
		}
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
			if self.action.try_is_pending_untracked() != Some(false) {
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

impl<Form, Deps, Input, Output> Clone for FormServerMutation<Form, Deps, Input, Output>
where
	Form: crate::FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	Input: 'static,
	Output: Clone + 'static,
{
	fn clone(&self) -> Self {
		Self {
			form: self.form.clone(),
			prepare: Rc::clone(&self.prepare),
			mutation: self.mutation,
		}
	}
}

impl<Form, Deps, Input, Output> FormServerMutation<Form, Deps, Input, Output>
where
	Form: crate::FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	Input: 'static,
	Output: Clone + 'static,
{
	/// Returns the generated form runtime attached to this mutation.
	pub fn form(&self) -> crate::UseFormReturn<Form, Deps> {
		self.form.clone()
	}

	/// Returns the current mutation phase.
	pub fn phase(&self) -> ActionPhase<Output, ServerFnError> {
		self.mutation.phase()
	}

	/// Returns `true` while validation or the server request is pending.
	///
	/// A disposed form runtime is treated as unavailable, so the live mutation
	/// action remains observable without accessing stale form signals.
	pub fn is_pending(&self) -> bool {
		let mutation_pending = self.mutation.is_pending();
		let form_pending = self
			.form
			.form_state()
			.is_submitting
			.try_get_untracked()
			.unwrap_or(false);
		mutation_pending || form_pending
	}

	/// Returns `true` when the mutation completed successfully.
	pub fn is_success(&self) -> bool {
		self.mutation.is_success()
	}

	/// Returns the latest successful result, if any.
	pub fn result(&self) -> Option<Output> {
		self.mutation.result()
	}

	/// Returns the latest error, if any.
	pub fn error(&self) -> Option<ServerFnError> {
		self.mutation.error()
	}

	/// Resets the underlying mutation back to `Idle`.
	pub fn reset(&self) {
		self.mutation.reset();
	}

	#[cfg(test)]
	pub(crate) fn force_success_for_test(&self, value: Output) {
		self.mutation.force_success_for_test(value);
	}

	#[cfg(test)]
	pub(crate) fn force_error_for_test(&self, error: ServerFnError) {
		self.mutation.force_error_for_test(error);
	}

	/// Validates the generated form and dispatches the prepared input on success.
	pub fn dispatch(&self) -> MutationDispatchOutcome {
		#[cfg(native)]
		{
			MutationDispatchOutcome::UnsupportedTarget
		}

		#[cfg(wasm)]
		{
			crate::reactive::untracked(|| {
				let mut pending_guard = crate::form_state::SubmitPendingGuard::new(
					self.form.form_state().is_submitting,
				);
				match self.mutation.action.try_is_pending_untracked() {
					Some(false) => {}
					Some(true) => {
						pending_guard.disarm();
						return MutationDispatchOutcome::AlreadyPending;
					}
					None => {
						pending_guard.disarm();
						return MutationDispatchOutcome::AlreadyPending;
					}
				}
				match self.form.begin_submit_lifecycle() {
					crate::UseFormSubmitOutcome::AlreadyPending => {
						pending_guard.disarm();
						return MutationDispatchOutcome::AlreadyPending;
					}
					crate::UseFormSubmitOutcome::ValidationFailed => {
						pending_guard.disarm();
						self.mutation.reset_action_preserving_result();
						return MutationDispatchOutcome::ValidationFailed;
					}
					crate::UseFormSubmitOutcome::Submitted => {}
				}
				let input = match (self.prepare)(&self.form) {
					Ok(input) => input,
					Err(error) => {
						self.form.complete_mutation_validation_error(error);
						pending_guard.disarm();
						self.mutation.reset_action_preserving_result();
						return MutationDispatchOutcome::ValidationFailed;
					}
				};
				let outcome = self.mutation.dispatch(input);
				if outcome == MutationDispatchOutcome::Dispatched {
					pending_guard.disarm();
				}
				outcome
			})
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
	use std::any::Any;
	use std::cell::{Cell, RefCell};
	use std::rc::Rc;

	use reinhardt_core::reactive::{Effect, ReactiveScope, with_runtime};
	use rstest::rstest;

	use super::ServerMutation;
	#[cfg(native)]
	use super::execute_server_mutation_once;
	use super::{ActionPhase, MutationDispatchOutcome, use_server_mutation};
	use crate::{
		FormRuntimeSource, FormValidationError, QueryClient, QueryDefaults, QueryFamily,
		ServerFnError, Signal, use_form,
	};

	#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
	enum NameField {
		Name,
	}

	#[derive(Clone)]
	struct NameForm {
		value: Signal<String>,
		validation_calls: Rc<Cell<usize>>,
	}

	impl NameForm {
		fn new(initial: impl Into<String>, validation_calls: Rc<Cell<usize>>) -> Self {
			Self {
				value: Signal::new(initial.into()),
				validation_calls,
			}
		}
	}

	impl FormRuntimeSource for NameForm {
		type Values = String;
		type Field = NameField;

		fn runtime_initial_values(&self) -> Self::Values {
			self.value.get_untracked()
		}

		fn runtime_current_values(&self) -> Self::Values {
			self.value.get_untracked()
		}

		fn runtime_apply_values(&self, values: &Self::Values) {
			self.value.set(values.clone());
		}

		fn runtime_set_field_value<T>(&self, field: Self::Field, value: T)
		where
			T: Any + 'static,
		{
			match field {
				NameField::Name => {
					let value = (&value as &dyn Any)
						.downcast_ref::<String>()
						.expect("test form only accepts String values");
					self.value.set(value.clone());
				}
			}
		}

		fn runtime_apply_field_value(&self, field: Self::Field, values: &Self::Values) {
			match field {
				NameField::Name => self.value.set(values.clone()),
			}
		}

		fn runtime_field_is_dirty(
			&self,
			field: Self::Field,
			current: &Self::Values,
			defaults: &Self::Values,
		) -> bool {
			match field {
				NameField::Name => current != defaults,
			}
		}

		fn runtime_watch_field<T>(&self, _field: Self::Field) -> Option<Signal<T>>
		where
			T: Clone + 'static,
		{
			None
		}

		fn runtime_field_by_name(&self, name: &str) -> Option<Self::Field> {
			(name == "name").then_some(NameField::Name)
		}

		fn runtime_validate(&self) -> Result<(), FormValidationError<Self::Field>> {
			self.validation_calls.set(self.validation_calls.get() + 1);
			if self.value.get_untracked().is_empty() {
				return Err(FormValidationError::field(
					NameField::Name,
					"Name is required",
				));
			}
			Ok(())
		}

		fn runtime_fields(&self) -> &'static [Self::Field] {
			&[NameField::Name]
		}
	}

	#[derive(Debug)]
	#[cfg(native)]
	struct DemoError;

	#[derive(Debug)]
	#[cfg(all(native, feature = "testing"))]
	struct PanicOnClone;

	#[cfg(all(native, feature = "testing"))]
	impl Clone for PanicOnClone {
		fn clone(&self) -> Self {
			panic!("response clone failed")
		}
	}

	#[cfg(native)]
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
	fn generated_form_native_dispatch_is_inert() {
		ReactiveScope::run(|| {
			let validation_calls = Rc::new(Cell::new(0));
			let prepare_calls = Rc::new(Cell::new(0));
			let form = NameForm::new("", Rc::clone(&validation_calls));
			let runtime = use_form(&form).build();
			let prepare_calls_for_builder = Rc::clone(&prepare_calls);
			let mutation =
				use_server_mutation(
					|value: String| async move { Ok::<String, ServerFnError>(value) },
				)
				.with_generated_form(&runtime, move |form| {
					prepare_calls_for_builder.set(prepare_calls_for_builder.get() + 1);
					Ok(form.get_values())
				})
				.build();

			assert_eq!(
				mutation.dispatch(),
				MutationDispatchOutcome::UnsupportedTarget
			);
			assert!(!runtime.form_state().is_submitting.get());
			assert_eq!(validation_calls.get(), 0);
			assert_eq!(prepare_calls.get(), 0);
		});
	}

	#[cfg(all(native, feature = "testing"))]
	#[rstest]
	fn generated_form_pending_clears_when_response_clone_panics() {
		let queued = Rc::new(RefCell::new(None));
		let queued_for_sink = Rc::clone(&queued);
		let _task_sink = crate::platform::install_task_sink(move |task| {
			*queued_for_sink.borrow_mut() = Some(task);
		});
		let scope = ReactiveScope::new();
		let (runtime, mutation) = scope.enter(|| {
			let form = NameForm::new("Ada", Rc::new(Cell::new(0)));
			let runtime = use_form(&form).build();
			let mutation = use_server_mutation(|_: ()| async {
				Ok::<PanicOnClone, ServerFnError>(PanicOnClone)
			})
			.with_generated_form(&runtime, |_| Ok(()))
			.build();
			(runtime, mutation)
		});
		runtime.form_state().is_submitting.set(true);
		mutation.mutation.action.dispatch(());

		let mut task = queued
			.borrow_mut()
			.take()
			.expect("dispatch should queue a native task");
		let mut context = std::task::Context::from_waker(std::task::Waker::noop());
		let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			task.as_mut().poll(&mut context)
		}));

		assert!(unwind.is_err());
		assert!(!runtime.form_state().is_submitting.get());
	}

	#[cfg(all(native, feature = "testing"))]
	#[rstest]
	#[serial_test::serial(reactive_runtime)]
	fn generated_form_pending_observer_tracks_action_completion() {
		let queued = Rc::new(RefCell::new(None));
		let queued_for_sink = Rc::clone(&queued);
		let _task_sink = crate::platform::install_task_sink(move |task| {
			*queued_for_sink.borrow_mut() = Some(task);
		});
		let scope = ReactiveScope::new();
		let observed = Rc::new(RefCell::new(Vec::new()));
		let (runtime, mutation, _effect) = scope.enter(|| {
			let form = NameForm::new("Ada", Rc::new(Cell::new(0)));
			let runtime = use_form(&form).build();
			let mutation = use_server_mutation(|_: ()| async { Ok::<(), ServerFnError>(()) })
				.with_generated_form(&runtime, |_| Ok(()))
				.build();
			let mutation_for_effect = mutation.clone();
			let observed_for_effect = Rc::clone(&observed);
			let effect = Effect::new(move || {
				observed_for_effect
					.borrow_mut()
					.push(mutation_for_effect.is_pending());
			});
			(runtime, mutation, effect)
		});

		runtime.form_state().is_submitting.set(true);
		mutation.mutation.action.dispatch(());
		with_runtime(|runtime| runtime.flush_updates());
		assert_eq!(observed.borrow().as_slice(), [false, true]);

		let mut task = queued
			.borrow_mut()
			.take()
			.expect("dispatch should queue a native task");
		let mut context = std::task::Context::from_waker(std::task::Waker::noop());
		assert_eq!(task.as_mut().poll(&mut context), std::task::Poll::Ready(()));
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(observed.borrow().as_slice(), [false, true, false]);
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
	#[cfg(native)]
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

			mutation.force_error_for_test(ServerFnError::application("failed"));

			assert!(mutation.phase().is_error());
			assert_eq!(mutation.result(), Some(8));

			mutation.reset();

			assert_eq!(mutation.phase(), ActionPhase::Idle);
			assert_eq!(mutation.result(), None);
		});
	}

	#[rstest]
	#[serial_test::serial(reactive_runtime)]
	fn reset_does_not_subscribe_the_calling_effect() {
		let scope = ReactiveScope::new();
		let mutation = scope.enter(|| {
			use_server_mutation(|value: i32| async move { Ok::<i32, ServerFnError>(value) }).build()
		});
		mutation.force_success_for_test(7);
		let runs = Rc::new(Cell::new(0));
		let runs_for_effect = Rc::clone(&runs);
		let _effect = scope.enter(|| {
			Effect::new(move || {
				runs_for_effect.set(runs_for_effect.get() + 1);
				mutation.reset();
			})
		});

		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(runs.get(), 1);
		assert_eq!(mutation.phase(), ActionPhase::Idle);
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
					.invalidate(client.clone(), family.key(()))
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
	fn structured_errors_reach_form_state_before_public_error_callbacks() {
		ReactiveScope::run(|| {
			let validation_calls = Rc::new(Cell::new(0));
			let form = NameForm::new("Ada", validation_calls);
			let runtime = use_form(&form).build();
			let seen_error = Rc::new(RefCell::new(None::<String>));
			let seen_error_for_callback = Rc::clone(&seen_error);
			let runtime_for_callback = runtime.clone();
			let mutation =
				use_server_mutation(
					|value: String| async move { Ok::<String, ServerFnError>(value) },
				)
				.with_generated_form(&runtime, |form| Ok(form.get_values()))
				.on_error(move |_| {
					let message = runtime_for_callback
						.form_state()
						.field_errors
						.get()
						.get(&NameField::Name)
						.map(|error| error.message().to_string());
					*seen_error_for_callback.borrow_mut() = message;
				})
				.build();

			mutation.force_error_for_test(ServerFnError::validation_with_message(
				"Please correct the form",
				[("name", "Name is already used")],
			));

			assert_eq!(seen_error.borrow().as_deref(), Some("Name is already used"));
		});
	}

	#[rstest]
	fn unknown_structured_fields_are_retained_in_form_level_errors() {
		ReactiveScope::run(|| {
			let validation_calls = Rc::new(Cell::new(0));
			let form = NameForm::new("Ada", validation_calls);
			let runtime = use_form(&form).build();
			let mutation =
				use_server_mutation(
					|value: String| async move { Ok::<String, ServerFnError>(value) },
				)
				.with_generated_form(&runtime, |form| Ok(form.get_values()))
				.build();

			mutation.force_error_for_test(ServerFnError::validation_with_message(
				"Please correct the form",
				[("unknown", "Unmapped field failed")],
			));

			assert_eq!(
				runtime.form_state().submit_error.get().as_deref(),
				Some("Please correct the form\nunknown: Unmapped field failed")
			);
		});
	}

	#[rstest]
	fn generated_form_success_hooks_run_before_reset_and_invalidations() {
		ReactiveScope::run(|| {
			let validation_calls = Rc::new(Cell::new(0));
			let form = NameForm::new("Ada", validation_calls);
			let order = Rc::new(RefCell::new(Vec::new()));
			let order_for_form = Rc::clone(&order);
			let order_for_user = Rc::clone(&order);
			let order_for_exact = Rc::clone(&order);
			let order_for_family = Rc::clone(&order);
			let runtime = use_form(&form)
				.on_submit_success(move |_| {
					order_for_form.borrow_mut().push("form-success");
				})
				.build();
			runtime.set_value(NameField::Name, "Grace".to_string());
			let runtime_for_exact = runtime.clone();
			let client = QueryClient::new_ssr(QueryDefaults::default());
			let family = QueryFamily::<(), i32, ServerFnError>::new("test.form-server-mutation");
			let mut builder =
				use_server_mutation(
					|value: String| async move { Ok::<String, ServerFnError>(value) },
				)
				.with_generated_form(&runtime, |form| Ok(form.get_values()))
				.on_success(move |_| {
					order_for_user.borrow_mut().push("user-success");
				})
				.reset_form_on_success()
				.invalidate(client.clone(), family.key(()))
				.invalidate_family(client, family);
			builder.builder.exact_invalidations.clear();
			builder.builder.family_invalidations.clear();
			builder.builder.exact_invalidations.push(Rc::new(move || {
				assert_eq!(runtime_for_exact.get_values(), "Ada".to_string());
				assert!(!runtime_for_exact.form_state().is_dirty.get());
				order_for_exact.borrow_mut().push("form-reset");
				order_for_exact.borrow_mut().push("exact");
			}));
			builder.builder.family_invalidations.push(Rc::new(move || {
				order_for_family.borrow_mut().push("family");
			}));
			let mutation = builder.build();

			mutation.force_success_for_test("saved".to_string());

			assert_eq!(
				order.borrow().as_slice(),
				[
					"form-success",
					"user-success",
					"form-reset",
					"exact",
					"family"
				]
			);
			assert_eq!(mutation.result().as_deref(), Some("saved"));
			assert_eq!(runtime.get_values(), "Ada".to_string());
		});
	}

	#[cfg(wasm)]
	#[rstest]
	fn synchronous_preflight_failures_reset_the_mutation_without_public_server_errors() {
		ReactiveScope::run(|| {
			let validation_calls = Rc::new(Cell::new(0));
			let form = NameForm::new("Ada", validation_calls);
			let form_failures = Rc::new(Cell::new(0));
			let public_server_errors = Rc::new(Cell::new(0));
			let form_failures_for_callback = Rc::clone(&form_failures);
			let runtime = use_form(&form)
				.on_submit_error(move |_| {
					form_failures_for_callback.set(form_failures_for_callback.get() + 1);
				})
				.build();
			let public_server_errors_for_callback = Rc::clone(&public_server_errors);
			let mutation =
				use_server_mutation(
					|value: String| async move { Ok::<String, ServerFnError>(value) },
				)
				.with_generated_form(&runtime, |_| {
					Err(FormValidationError::form("Please correct the form"))
				})
				.on_error(move |_| {
					public_server_errors_for_callback
						.set(public_server_errors_for_callback.get() + 1);
				})
				.build();
			mutation.force_success_for_test("previous".to_owned());

			assert_eq!(
				mutation.dispatch(),
				MutationDispatchOutcome::ValidationFailed
			);
			assert_eq!(mutation.phase(), ActionPhase::Idle);
			assert_eq!(mutation.result().as_deref(), Some("previous"));
			assert_eq!(form_failures.get(), 1);
			assert_eq!(public_server_errors.get(), 0);
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
