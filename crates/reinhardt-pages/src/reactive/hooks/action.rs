//! Action hooks: use_action_state and use_optimistic
//!
//! These hooks provide React-like form action and optimistic update support.

use std::rc::Rc;

use crate::reactive::Signal;

/// State returned by use_action_state.
///
/// Contains the current state, the action function, and pending status.
pub struct ActionState<S: 'static, P> {
	/// The current state value.
	pub state: Signal<S>,
	/// Whether an action is currently pending.
	pub is_pending: Signal<bool>,
	/// The action function to call.
	action: Rc<dyn Fn(P)>,
}

impl<S: 'static, P> ActionState<S, P> {
	/// Dispatches the action with the given payload.
	pub fn dispatch(&self, payload: P) {
		(self.action)(payload);
	}
}

impl<S: Clone + 'static, P> Clone for ActionState<S, P> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			is_pending: self.is_pending.clone(),
			action: Rc::clone(&self.action),
		}
	}
}

/// Manages state that updates based on form action results.
///
/// This is the React-like equivalent of `useActionState` (formerly `useFormState`).
/// It's designed for handling form submissions and their resulting state changes.
///
/// # Type Parameters
///
/// * `S` - The state type
/// * `P` - The payload/action type
/// * `F` - The action function type
///
/// # Arguments
///
/// * `action` - A function that takes the current state and payload, returns new state
/// * `initial` - The initial state value
///
/// # Returns
///
/// An `ActionState` containing the state, pending status, and dispatch function
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_action_state;
///
/// #[derive(Clone, Default)]
/// struct FormState {
///     message: Option<String>,
///     error: Option<String>,
/// }
///
/// struct FormData {
///     email: String,
///     password: String,
/// }
///
/// let action_state = use_action_state(
///     |_state: &FormState, data: FormData| {
///         // In real code, this would be async
///         if data.email.is_empty() {
///             FormState { message: None, error: Some("Email required".into()) }
///         } else {
///             FormState { message: Some("Success!".into()), error: None }
///         }
///     },
///     FormState::default(),
/// );
///
/// // In form submission
/// action_state.dispatch(FormData {
///     email: "user@example.com".into(),
///     password: "secret".into(),
/// });
///
/// // Show loading state
/// if action_state.is_pending.get() {
///     // ... show spinner
/// }
///
/// // Show result
/// if let Some(error) = &action_state.state.get().error {
///     // ... show error message
/// }
/// ```
pub fn use_action_state<S, P, F>(action: F, initial: S) -> ActionState<S, P>
where
	S: Clone + 'static,
	P: 'static,
	F: Fn(&S, P) -> S + 'static,
{
	let state = Signal::new(initial);
	let is_pending = Signal::new(false);

	let dispatch: Rc<dyn Fn(P)> = {
		let state = state.clone();
		let is_pending = is_pending.clone();
		let action = Rc::new(action);

		Rc::new(move |payload: P| {
			is_pending.set(true);
			let current = state.get();
			let new_state = action(&current, payload);
			state.set(new_state);
			is_pending.set(false);
		})
	};

	ActionState {
		state,
		is_pending,
		action: dispatch,
	}
}

/// State for optimistic updates.
pub struct OptimisticState<T: 'static> {
	/// The current value (may be optimistic).
	value: Signal<T>,
	/// The actual confirmed value.
	confirmed: Signal<T>,
	/// Whether an optimistic update is pending.
	is_optimistic: Signal<bool>,
}

impl<T: Clone + 'static> OptimisticState<T> {
	/// Gets the current value (optimistic if pending, confirmed otherwise).
	pub fn get(&self) -> T {
		self.value.get()
	}

	/// Returns whether the current value is optimistic (unconfirmed).
	pub fn is_optimistic(&self) -> bool {
		self.is_optimistic.get()
	}

	/// Applies an optimistic update.
	pub fn update_optimistic(&self, value: T) {
		self.is_optimistic.set(true);
		self.value.set(value);
	}

	/// Confirms the value (called after successful async operation).
	pub fn confirm(&self, value: T) {
		self.confirmed.set(value.clone());
		self.value.set(value);
		self.is_optimistic.set(false);
	}

	/// Reverts to the confirmed value (called on error).
	pub fn revert(&self) {
		self.value.set(self.confirmed.get());
		self.is_optimistic.set(false);
	}
}

impl<T: Clone + 'static> Clone for OptimisticState<T> {
	fn clone(&self) -> Self {
		Self {
			value: self.value.clone(),
			confirmed: self.confirmed.clone(),
			is_optimistic: self.is_optimistic.clone(),
		}
	}
}

/// Enables optimistic UI updates during async operations.
///
/// This is the React-like equivalent of `useOptimistic`. It allows you to show
/// an optimistic (predicted) state while an async operation is pending, then
/// either confirm or revert based on the result.
///
/// # Type Parameters
///
/// * `T` - The type of the state value
///
/// # Arguments
///
/// * `initial` - The initial (confirmed) value
///
/// # Returns
///
/// An `OptimisticState<T>` for managing optimistic updates
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_optimistic;
///
/// let likes = use_optimistic(100);
///
/// let on_like = use_callback({
///     let likes = likes.clone();
///     move |_| {
///         // Optimistically increment
///         likes.update_optimistic(likes.get() + 1);
///
///         // Perform async operation
///         spawn_local({
///             let likes = likes.clone();
///             async move {
///                 match api_like_post().await {
///                     Ok(new_count) => likes.confirm(new_count),
///                     Err(_) => likes.revert(),
///                 }
///             }
///         });
///     }
/// });
///
/// page!(|| {
///     button {
///         @click: on_like,
///         format!("♥ {} {}", likes.get(), if likes.is_optimistic() { "(updating...)" } else { "" })
///     }
/// })
/// ```
pub fn use_optimistic<T: Clone + 'static>(initial: T) -> OptimisticState<T> {
	let value = Signal::new(initial.clone());
	let confirmed = Signal::new(initial);
	let is_optimistic = Signal::new(false);

	OptimisticState {
		value,
		confirmed,
		is_optimistic,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_use_action_state_basic() {
		#[derive(Clone, Debug, PartialEq)]
		struct State {
			count: i32,
		}

		let action_state = use_action_state(
			|state: &State, increment: i32| State {
				count: state.count + increment,
			},
			State { count: 0 },
		);

		assert_eq!(action_state.state.get().count, 0);
		assert!(!action_state.is_pending.get());

		action_state.dispatch(5);
		assert_eq!(action_state.state.get().count, 5);

		action_state.dispatch(3);
		assert_eq!(action_state.state.get().count, 8);
	}

	#[test]
	fn test_use_optimistic_basic() {
		let state = use_optimistic(10);

		assert_eq!(state.get(), 10);
		assert!(!state.is_optimistic());

		// Apply optimistic update
		state.update_optimistic(20);
		assert_eq!(state.get(), 20);
		assert!(state.is_optimistic());

		// Confirm the update
		state.confirm(20);
		assert_eq!(state.get(), 20);
		assert!(!state.is_optimistic());
	}

	#[test]
	fn test_use_optimistic_revert() {
		let state = use_optimistic(10);

		// Apply optimistic update
		state.update_optimistic(20);
		assert_eq!(state.get(), 20);

		// Revert on error
		state.revert();
		assert_eq!(state.get(), 10);
		assert!(!state.is_optimistic());
	}

	#[test]
	fn test_optimistic_state_clone() {
		let state1 = use_optimistic(42);
		let state2 = state1.clone();

		state1.update_optimistic(100);
		assert_eq!(state2.get(), 100);
	}
}
