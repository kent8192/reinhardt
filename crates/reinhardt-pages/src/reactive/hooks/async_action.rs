//! Async action hook: `use_action`
//!
//! Provides an async mutation hook with pending/success/error state tracking.
//! This is designed for handling async operations like API calls, form submissions,
//! and other side effects that return a `Result`.

use std::future::Future;
use std::rc::Rc;

use crate::reactive::Signal;

/// Represents the current phase of an async action.
///
/// An action progresses through phases: `Idle` -> `Pending` -> `Success`/`Error`.
///
/// # Type Parameters
///
/// * `T` - The success value type
/// * `E` - The error value type
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::reactive::hooks::ActionPhase;
///
/// let phase: ActionPhase<String, String> = ActionPhase::Idle;
/// assert!(phase.is_idle());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum ActionPhase<T, E> {
	/// No action has been dispatched yet, or the action has been reset.
	Idle,
	/// An action is currently in progress.
	Pending,
	/// The action completed successfully with a value.
	Success(T),
	/// The action failed with an error.
	Error(E),
}

impl<T, E> ActionPhase<T, E> {
	/// Returns `true` if the phase is `Idle`.
	pub fn is_idle(&self) -> bool {
		matches!(self, ActionPhase::Idle)
	}

	/// Returns `true` if the phase is `Pending`.
	pub fn is_pending(&self) -> bool {
		matches!(self, ActionPhase::Pending)
	}

	/// Returns `true` if the phase is `Success`.
	pub fn is_success(&self) -> bool {
		matches!(self, ActionPhase::Success(_))
	}

	/// Returns `true` if the phase is `Error`.
	pub fn is_error(&self) -> bool {
		matches!(self, ActionPhase::Error(_))
	}

	/// Returns the success value if available.
	pub fn result(&self) -> Option<&T> {
		match self {
			ActionPhase::Success(val) => Some(val),
			_ => None,
		}
	}

	/// Returns the error value if available.
	pub fn error(&self) -> Option<&E> {
		match self {
			ActionPhase::Error(err) => Some(err),
			_ => None,
		}
	}
}

/// Handle returned by [`use_action`] for dispatching async mutations.
///
/// `Action` wraps the lifecycle of an async operation, tracking its phase
/// through `Idle` -> `Pending` -> `Success`/`Error`. The payload type `P`
/// is captured in the dispatch closure and does not appear in the struct type,
/// keeping the API ergonomic.
///
/// # Type Parameters
///
/// * `T` - The success value type (must be `Clone + 'static`)
/// * `E` - The error value type (must be `Clone + 'static`)
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_action;
///
/// let action = use_action(|user_id: u32| async move {
///     api_delete_user(user_id).await
/// });
///
/// // Dispatch the action
/// action.dispatch(42);
///
/// // Check the phase
/// if action.is_pending() {
///     // show spinner
/// }
/// ```
pub struct Action<T: Clone + 'static, E: Clone + 'static> {
	state: Signal<ActionPhase<T, E>>,
	dispatch_fn: Rc<dyn Fn()>,
	/// Stores the payload setter so dispatch can pass payload before triggering.
	payload_setter: Rc<dyn Fn(Box<dyn std::any::Any>)>,
}

impl<T: Clone + 'static, E: Clone + 'static> Action<T, E> {
	/// Returns the current phase of the action, tracking the dependency.
	pub fn phase(&self) -> ActionPhase<T, E> {
		self.state.get()
	}

	/// Returns `true` if the action is idle.
	pub fn is_idle(&self) -> bool {
		self.phase().is_idle()
	}

	/// Returns `true` if the action is pending.
	pub fn is_pending(&self) -> bool {
		self.phase().is_pending()
	}

	/// Returns `true` if the action completed successfully.
	pub fn is_success(&self) -> bool {
		self.phase().is_success()
	}

	/// Returns `true` if the action failed.
	pub fn is_error(&self) -> bool {
		self.phase().is_error()
	}

	/// Returns the success value if available.
	pub fn result(&self) -> Option<T> {
		match self.state.get() {
			ActionPhase::Success(val) => Some(val),
			_ => None,
		}
	}

	/// Returns the error value if available.
	pub fn error(&self) -> Option<E> {
		match self.state.get() {
			ActionPhase::Error(err) => Some(err),
			_ => None,
		}
	}

	/// Resets the action back to `Idle` phase.
	pub fn reset(&self) {
		self.state.set(ActionPhase::Idle);
	}
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for Action<T, E> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			dispatch_fn: Rc::clone(&self.dispatch_fn),
			payload_setter: Rc::clone(&self.payload_setter),
		}
	}
}

/// Creates an async action hook for performing mutations with state tracking.
///
/// `use_action` accepts an async function that takes a payload and returns a `Result`.
/// It returns an [`Action`] handle that tracks the lifecycle phases:
/// `Idle` -> `Pending` -> `Success(T)` / `Error(E)`.
///
/// # Type Parameters
///
/// * `P` - The payload type passed to dispatch
/// * `T` - The success value type
/// * `E` - The error value type
/// * `F` - The action function type
/// * `Fut` - The future type returned by the action function
///
/// # Arguments
///
/// * `action_fn` - An async function `Fn(P) -> Future<Output = Result<T, E>>`
///
/// # Returns
///
/// An [`Action<T, E>`] handle for dispatching and observing the action
///
/// # Dual-target behavior
///
/// - **WASM**: Uses `spawn_task` to run the future asynchronously. The phase
///   transitions `Idle -> Pending -> Success/Error` over time.
/// - **Non-WASM**: The future is not awaited (dropped). The phase transitions
///   `Idle -> Pending -> Idle` synchronously. This is intentional for SSR where
///   async mutations are not meaningful.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_action;
///
/// async fn submit_form(data: FormData) -> Result<String, String> {
///     // Perform API call...
///     Ok("Success".to_string())
/// }
///
/// let action = use_action(submit_form);
///
/// // Dispatch with payload
/// action.dispatch(form_data);
///
/// // Observe phase
/// match action.phase() {
///     ActionPhase::Idle => { /* show form */ }
///     ActionPhase::Pending => { /* show spinner */ }
///     ActionPhase::Success(msg) => { /* show success */ }
///     ActionPhase::Error(err) => { /* show error */ }
/// }
/// ```
pub fn use_action<P, T, E, F, Fut>(action_fn: F) -> Action<T, E>
where
	P: 'static,
	T: Clone + 'static,
	E: Clone + 'static,
	F: Fn(P) -> Fut + 'static,
	Fut: Future<Output = Result<T, E>> + 'static,
{
	let state = Signal::new(ActionPhase::Idle);

	// Store the payload in a shared cell so dispatch_fn can access it
	let payload_cell: Rc<std::cell::RefCell<Option<Box<dyn std::any::Any>>>> =
		Rc::new(std::cell::RefCell::new(None));

	let payload_setter: Rc<dyn Fn(Box<dyn std::any::Any>)> = {
		let payload_cell = Rc::clone(&payload_cell);
		Rc::new(move |payload: Box<dyn std::any::Any>| {
			*payload_cell.borrow_mut() = Some(payload);
		})
	};

	let dispatch_fn: Rc<dyn Fn()> = {
		let state = state.clone();
		let action_fn = Rc::new(action_fn);
		let payload_cell = Rc::clone(&payload_cell);

		Rc::new(move || {
			let payload = payload_cell
				.borrow_mut()
				.take()
				.and_then(|p| p.downcast::<P>().ok())
				.expect("dispatch called without payload");

			state.set(ActionPhase::Pending);

			#[cfg(wasm)]
			{
				use crate::spawn::spawn_task;
				let state = state.clone();
				let fut = action_fn(*payload);
				spawn_task(async move {
					match fut.await {
						Ok(val) => state.set(ActionPhase::Success(val)),
						Err(err) => state.set(ActionPhase::Error(err)),
					}
				});
			}

			#[cfg(native)]
			{
				// Non-WASM: drop the future, reset to Idle
				let _fut = action_fn(*payload);
				state.set(ActionPhase::Idle);
			}
		})
	};

	Action {
		state,
		dispatch_fn,
		payload_setter,
	}
}

impl<T: Clone + 'static, E: Clone + 'static> Action<T, E> {
	/// Dispatches the action with the given payload.
	///
	/// This sets the phase to `Pending` and begins executing the async action.
	/// On WASM, the future runs asynchronously. On non-WASM, the phase resets to `Idle`.
	pub fn dispatch<P: 'static>(&self, payload: P) {
		(self.payload_setter)(Box::new(payload));
		(self.dispatch_fn)();
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	fn test_action_phase_methods() {
		// Arrange
		let idle: ActionPhase<String, String> = ActionPhase::Idle;
		let pending: ActionPhase<String, String> = ActionPhase::Pending;
		let success: ActionPhase<String, String> = ActionPhase::Success("ok".to_string());
		let error: ActionPhase<String, String> = ActionPhase::Error("fail".to_string());

		// Act & Assert
		assert!(idle.is_idle());
		assert!(!idle.is_pending());
		assert!(!idle.is_success());
		assert!(!idle.is_error());
		assert_eq!(idle.result(), None);
		assert_eq!(idle.error(), None);

		assert!(!pending.is_idle());
		assert!(pending.is_pending());
		assert!(!pending.is_success());
		assert!(!pending.is_error());

		assert!(!success.is_idle());
		assert!(!success.is_pending());
		assert!(success.is_success());
		assert!(!success.is_error());
		assert_eq!(success.result(), Some(&"ok".to_string()));
		assert_eq!(success.error(), None);

		assert!(!error.is_idle());
		assert!(!error.is_pending());
		assert!(!error.is_success());
		assert!(error.is_error());
		assert_eq!(error.result(), None);
		assert_eq!(error.error(), Some(&"fail".to_string()));
	}

	#[rstest]
	fn test_use_action_initial_idle() {
		// Arrange & Act
		let action = use_action(|_: ()| async { Ok::<String, String>("done".to_string()) });

		// Assert
		assert!(action.is_idle());
		assert_eq!(action.phase(), ActionPhase::Idle);
		assert_eq!(action.result(), None);
		assert_eq!(action.error(), None);
	}

	#[rstest]
	fn test_use_action_dispatch_native() {
		// Arrange
		let action = use_action(|x: i32| async move {
			if x > 0 {
				Ok::<i32, String>(x * 2)
			} else {
				Err("negative".to_string())
			}
		});

		// Act
		action.dispatch(5);

		// Assert
		// On non-WASM, dispatch sets Pending then immediately resets to Idle
		assert!(action.is_idle());
	}

	#[rstest]
	fn test_action_clone() {
		// Arrange
		let action1 = use_action(|_: ()| async { Ok::<(), String>(()) });

		// Act
		let action2 = action1.clone();

		// Assert - both share the same Signal
		assert!(action1.is_idle());
		assert!(action2.is_idle());

		// Dispatching via one affects the other
		action1.dispatch(());
		assert_eq!(action1.phase(), action2.phase());
	}

	#[rstest]
	fn test_action_reset() {
		// Arrange
		let action = use_action(|_: ()| async { Ok::<String, String>("done".to_string()) });
		action.dispatch(());

		// Act
		action.reset();

		// Assert
		assert!(action.is_idle());
		assert_eq!(action.phase(), ActionPhase::Idle);
	}
}
