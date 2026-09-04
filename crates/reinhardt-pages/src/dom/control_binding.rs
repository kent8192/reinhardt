use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::component::{
	ControlBinding, ControlBindingError, ControlKind, ControlValue, ControlWriteOutcome,
};
use crate::dom::{Element, EventHandle};
use crate::reactive::{Effect, EffectTiming, batch, untracked};
use reinhardt_core::{reactive::runtime::NodeId, types::page::ControlBindingSnapshot};

type HydrationSnapshotStore = Rc<RefCell<Vec<ControlBindingSnapshot>>>;
type RejectedNumberSnapshotStore = Rc<RefCell<Vec<RejectedNumberSnapshot>>>;

thread_local! {
	static ACTIVE_HYDRATION_SNAPSHOT_STORE: RefCell<Option<HydrationSnapshotStore>> =
		const { RefCell::new(None) };
	static ACTIVE_HYDRATION_ADOPTED_TARGETS: RefCell<Option<Rc<RefCell<Vec<NodeId>>>>> =
		const { RefCell::new(None) };
	static ACTIVE_REJECTED_NUMBER_SNAPSHOTS: RefCell<Option<RejectedNumberSnapshotStore>> =
		const { RefCell::new(None) };
	static ACTIVE_NUMBER_BINDINGS: RefCell<Vec<MountedNumberBinding>> = const { RefCell::new(Vec::new()) };
}

struct ActiveHydrationSnapshotStoreGuard {
	previous: Option<HydrationSnapshotStore>,
	previous_adopted_targets: Option<Rc<RefCell<Vec<NodeId>>>>,
	previous_rejected_number_snapshots: Option<RejectedNumberSnapshotStore>,
}

impl Drop for ActiveHydrationSnapshotStoreGuard {
	fn drop(&mut self) {
		ACTIVE_HYDRATION_SNAPSHOT_STORE.with(|active| {
			active.replace(self.previous.take());
		});
		ACTIVE_HYDRATION_ADOPTED_TARGETS.with(|active| {
			active.replace(self.previous_adopted_targets.take());
		});
		ACTIVE_REJECTED_NUMBER_SNAPSHOTS.with(|active| {
			active.replace(self.previous_rejected_number_snapshots.take());
		});
	}
}

struct HydrationSnapshotTransaction {
	store: HydrationSnapshotStore,
	committed: bool,
}

impl HydrationSnapshotTransaction {
	fn new() -> Self {
		Self {
			store: Rc::new(RefCell::new(Vec::new())),
			committed: false,
		}
	}

	fn commit(&mut self) {
		for snapshot in self.store.borrow_mut().drain(..) {
			snapshot.commit();
		}
		self.committed = true;
	}
}

impl Drop for HydrationSnapshotTransaction {
	fn drop(&mut self) {
		if !self.committed {
			let mut snapshots = self.store.borrow_mut();
			while let Some(snapshot) = snapshots.pop() {
				drop(snapshot);
			}
		}
	}
}

pub(crate) fn with_hydration_snapshot_transaction<T, E>(
	f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
	let mut transaction = HydrationSnapshotTransaction::new();
	let previous = ACTIVE_HYDRATION_SNAPSHOT_STORE
		.with(|active| active.replace(Some(transaction.store.clone())));
	let previous_adopted_targets = ACTIVE_HYDRATION_ADOPTED_TARGETS
		.with(|active| active.replace(Some(Rc::new(RefCell::new(Vec::new())))));
	let previous_rejected_number_snapshots = ACTIVE_REJECTED_NUMBER_SNAPSHOTS
		.with(|active| active.replace(Some(Rc::new(RefCell::new(Vec::new())))));
	let guard = ActiveHydrationSnapshotStoreGuard {
		previous,
		previous_adopted_targets,
		previous_rejected_number_snapshots,
	};
	let result = f();
	drop(guard);
	if result.is_ok() {
		transaction.commit();
	}
	result
}

struct ActiveRejectedNumberSnapshotStoreGuard {
	previous_rejected_number_snapshots: Option<RejectedNumberSnapshotStore>,
}

impl Drop for ActiveRejectedNumberSnapshotStoreGuard {
	fn drop(&mut self) {
		ACTIVE_REJECTED_NUMBER_SNAPSHOTS.with(|active| {
			active.replace(self.previous_rejected_number_snapshots.take());
		});
	}
}

fn with_rejected_number_snapshot_transaction<T, E>(
	f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
	let already_active = ACTIVE_REJECTED_NUMBER_SNAPSHOTS.with(|active| active.borrow().is_some());
	if already_active {
		return f();
	}

	let previous_rejected_number_snapshots = ACTIVE_REJECTED_NUMBER_SNAPSHOTS
		.with(|active| active.replace(Some(Rc::new(RefCell::new(Vec::new())))));
	let _guard = ActiveRejectedNumberSnapshotStoreGuard {
		previous_rejected_number_snapshots,
	};
	f()
}

fn commit_or_stage_hydration_snapshot(snapshot: ControlBindingSnapshot) {
	let snapshot = ACTIVE_HYDRATION_SNAPSHOT_STORE.with(|active| {
		if let Some(store) = active.borrow().as_ref() {
			store.borrow_mut().push(snapshot);
			None
		} else {
			Some(snapshot)
		}
	});
	if let Some(snapshot) = snapshot {
		snapshot.commit();
	}
}

fn hydration_target_was_adopted(binding: &ControlBinding) -> bool {
	ACTIVE_HYDRATION_ADOPTED_TARGETS.with(|active| {
		active
			.borrow()
			.as_ref()
			.is_some_and(|targets| targets.borrow().contains(&binding.target()))
	})
}

fn record_hydration_target_adoption(binding: &ControlBinding) {
	ACTIVE_HYDRATION_ADOPTED_TARGETS.with(|active| {
		if let Some(targets) = active.borrow().as_ref()
			&& !targets.borrow().contains(&binding.target())
		{
			targets.borrow_mut().push(binding.target());
		}
	});
}

fn stage_rejected_number_snapshot(
	binding: &ControlBinding,
	position: Option<usize>,
	raw: String,
	selection: Option<EditorSelection>,
) {
	if binding.kind() != ControlKind::Number {
		return;
	}
	ACTIVE_REJECTED_NUMBER_SNAPSHOTS.with(|active| {
		if let Some(snapshots) = active.borrow().as_ref() {
			snapshots.borrow_mut().push(RejectedNumberSnapshot {
				target: binding.target(),
				position,
				raw,
				selection,
			});
		}
	});
}

fn stage_rejected_number_hydration_snapshot(
	element: &Element,
	binding: &ControlBinding,
	position: Option<usize>,
) {
	let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() else {
		return;
	};
	stage_rejected_number_snapshot(binding, position, input.value(), input_selection(input));
}

fn take_rejected_number_snapshot(
	binding: &ControlBinding,
	position: Option<usize>,
) -> Option<RejectedNumberSnapshot> {
	if binding.kind() != ControlKind::Number {
		return None;
	}
	ACTIVE_REJECTED_NUMBER_SNAPSHOTS.with(|active| {
		let active = active.borrow();
		let snapshots = active.as_ref()?;
		let mut snapshots = snapshots.borrow_mut();
		let index = snapshots.iter().position(|snapshot| {
			snapshot.target == binding.target() && snapshot.position == position
		})?;
		Some(snapshots.remove(index))
	})
}

fn restore_rejected_number_snapshot(element: &Element, snapshot: &RejectedNumberSnapshot) {
	let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() else {
		return;
	};
	input.set_value(&snapshot.raw);
	if let Some(selection) = snapshot.selection {
		let selection = selection.clamped(snapshot.raw.len());
		let _ = input.set_selection_range(selection.start as u32, selection.end as u32);
	}
}

pub(crate) struct ControlBindingController {
	effect: Effect,
	_listeners: Vec<EventHandle>,
	_option_observer: Option<SelectOptionObserver>,
	_number_binding_registration: Option<NumberBindingRegistration>,
}

impl Drop for ControlBindingController {
	fn drop(&mut self) {
		self.effect.dispose();
	}
}

struct MountedNumberBinding {
	target: NodeId,
	element: web_sys::Element,
	position: usize,
}

struct NumberBindingRegistration {
	target: NodeId,
	element: web_sys::Element,
	position: usize,
}

impl NumberBindingRegistration {
	fn register(element: &Element, binding: &ControlBinding) -> Option<Self> {
		if binding.kind() != ControlKind::Number {
			return None;
		}
		ACTIVE_NUMBER_BINDINGS.with(|registered| {
			let mut registered = registered.borrow_mut();
			let position = (0..)
				.find(|position| {
					!registered.iter().any(|candidate| {
						candidate.target == binding.target() && candidate.position == *position
					})
				})
				.expect("number binding position overflow");
			registered.push(MountedNumberBinding {
				target: binding.target(),
				element: element.as_web_sys().clone(),
				position,
			});
			Some(Self {
				target: binding.target(),
				element: element.as_web_sys().clone(),
				position,
			})
		})
	}
}

impl Drop for NumberBindingRegistration {
	fn drop(&mut self) {
		let registration_node: web_sys::Node = self.element.clone().unchecked_into();
		ACTIVE_NUMBER_BINDINGS.with(|registered| {
			registered.borrow_mut().retain(|candidate| {
				candidate.target != self.target
					|| !candidate
						.element
						.clone()
						.unchecked_into::<web_sys::Node>()
						.is_same_node(Some(&registration_node))
			});
		});
	}
}

struct SelectOptionObserver {
	observer: web_sys::MutationObserver,
	_callback: Closure<dyn FnMut(js_sys::Array, web_sys::MutationObserver)>,
}

impl Drop for SelectOptionObserver {
	fn drop(&mut self) {
		self.observer.disconnect();
	}
}

impl std::fmt::Debug for ControlBindingController {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("ControlBindingController")
			.finish_non_exhaustive()
	}
}

#[derive(Default)]
struct CompositionState {
	composing: bool,
	applying_input: bool,
	skip_next_input: Option<CompletedComposition>,
	number_editor: Option<NumberEditorState>,
	number_position: Option<usize>,
}

#[derive(Default)]
struct NumberEditorState {
	raw: String,
	selection: Option<EditorSelection>,
	pending_edit: Option<PendingNumberEdit>,
}

#[derive(Clone, Copy)]
struct EditorSelection {
	start: usize,
	end: usize,
	anchor: usize,
	focus: usize,
}

struct PendingNumberEdit {
	raw: String,
	selection: EditorSelection,
}

struct RejectedNumberSnapshot {
	target: NodeId,
	position: Option<usize>,
	raw: String,
	selection: Option<EditorSelection>,
}

struct CompletedComposition {
	value: ControlValue,
	signal_baseline: ControlValue,
}

impl ControlBindingController {
	pub(crate) fn mount(
		element: Element,
		binding: ControlBinding,
	) -> Result<Self, ControlBindingError> {
		validate_control(&element, binding.kind())?;
		write_radio_value(&element, &binding)?;
		let number_binding_registration = NumberBindingRegistration::register(&element, &binding);
		let number_position = number_binding_registration
			.as_ref()
			.map(|registration| registration.position);
		let rejected_number_snapshot = take_rejected_number_snapshot(&binding, number_position);
		let initial_value = untracked(|| binding.read());
		write_control_and_reconcile(&element, &binding, &initial_value)?;
		if let Some(snapshot) = rejected_number_snapshot.as_ref() {
			restore_rejected_number_snapshot(&element, snapshot);
		}
		let controller = Self::install(
			element,
			binding,
			rejected_number_snapshot.is_some(),
			rejected_number_snapshot.as_ref(),
			number_position,
			number_binding_registration,
		)?;
		Ok(controller)
	}

	pub(crate) fn hydrate(
		element: Element,
		binding: ControlBinding,
	) -> Result<(Self, bool), ControlBindingError> {
		validate_control(&element, binding.kind())?;
		write_radio_value(&element, &binding)?;
		let password_value_was_omitted = element
			.get_attribute(crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE)
			.as_deref()
			== Some("true");
		if password_value_was_omitted {
			let _ =
				element.remove_attribute(crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE);
		}
		let number_binding_registration = NumberBindingRegistration::register(&element, &binding);
		let number_position = number_binding_registration
			.as_ref()
			.map(|registration| registration.position);
		let (listeners, state) = install_listeners(&element, &binding, None, number_position);
		let live_value = read_control(&element, binding.kind())?;
		let password_value_was_omitted = password_value_was_omitted
			&& binding.kind() == ControlKind::Text
			&& element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlInputElement>()
				.is_some_and(|input| input.type_().eq_ignore_ascii_case("password"))
			&& matches!(&live_value, ControlValue::Text(value) if value.is_empty());
		let expected_value = untracked(|| binding.read());
		let should_restore_expected = password_value_was_omitted
			|| hydration_target_was_adopted(&binding)
			|| (matches!(
				binding.kind(),
				ControlKind::SelectOne | ControlKind::SelectMany
			) && !select_has_option_values(&element, &expected_value));
		let should_restore_expected = should_restore_expected
			|| (binding.kind() == ControlKind::SelectOne
				&& expected_value == live_value
				&& !select_one_matches_expected_option(&element, &expected_value));
		let refresh_required = if should_restore_expected {
			write_control_and_reconcile(&element, &binding, &expected_value)?;
			crate::component::into_page::initialize_control_default(&element, &binding);
			false
		} else if expected_value == live_value {
			false
		} else {
			let snapshot = binding.snapshot();
			let outcome = binding.write(live_value.clone())?;
			commit_or_stage_hydration_snapshot(snapshot);
			let adopted = matches!(outcome, ControlWriteOutcome::Committed);
			let rejected = matches!(outcome, ControlWriteOutcome::Rejected(_));
			if matches!(outcome, ControlWriteOutcome::Ignored) {
				write_control_and_reconcile(&element, &binding, &expected_value)?;
			}
			if rejected {
				stage_rejected_number_hydration_snapshot(&element, &binding, number_position);
				record_hydration_target_adoption(&binding);
			}
			if adopted {
				record_hydration_target_adoption(&binding);
				crate::component::into_page::initialize_control_default(&element, &binding);
			}
			adopted || rejected
		};
		let option_observer = install_select_option_observer(&element, &binding);
		let effect = install_effect(element, binding, true, state);
		Ok((
			Self {
				effect,
				_listeners: listeners,
				_option_observer: option_observer,
				_number_binding_registration: number_binding_registration,
			},
			refresh_required,
		))
	}

	fn install(
		element: Element,
		binding: ControlBinding,
		skip_first_write: bool,
		rejected_number_snapshot: Option<&RejectedNumberSnapshot>,
		number_position: Option<usize>,
		number_binding_registration: Option<NumberBindingRegistration>,
	) -> Result<Self, ControlBindingError> {
		let (listeners, state) = install_listeners(
			&element,
			&binding,
			rejected_number_snapshot,
			number_position,
		);
		let option_observer = install_select_option_observer(&element, &binding);
		let effect = install_effect(element, binding, skip_first_write, state);
		Ok(Self {
			effect,
			_listeners: listeners,
			_option_observer: option_observer,
			_number_binding_registration: number_binding_registration,
		})
	}
}

fn install_select_option_observer(
	element: &Element,
	binding: &ControlBinding,
) -> Option<SelectOptionObserver> {
	if !matches!(
		binding.kind(),
		ControlKind::SelectOne | ControlKind::SelectMany
	) {
		return None;
	}

	let observed_element = element.clone();
	let observed_binding = binding.clone();
	let callback = Closure::wrap(
		Box::new(move |_: js_sys::Array, _: web_sys::MutationObserver| {
			let value = untracked(|| observed_binding.read());
			let _ = write_control(&observed_element, observed_binding.kind(), &value);
			crate::component::into_page::initialize_control_default(
				&observed_element,
				&observed_binding,
			);
		}) as Box<dyn FnMut(js_sys::Array, web_sys::MutationObserver)>,
	);
	let observer = web_sys::MutationObserver::new(callback.as_ref().unchecked_ref()).ok()?;
	let options = web_sys::MutationObserverInit::new();
	options.set_child_list(true);
	options.set_subtree(true);
	observer
		.observe_with_options(element.as_web_sys(), &options)
		.ok()?;

	Some(SelectOptionObserver {
		observer,
		_callback: callback,
	})
}

fn select_has_option_values(element: &Element, value: &ControlValue) -> bool {
	let Some(select) = element.as_web_sys().dyn_ref::<web_sys::HtmlSelectElement>() else {
		return true;
	};
	let options = select.options();
	let available = (0..options.length())
		.filter_map(|index| {
			options
				.item(index)
				.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
				.map(|option| option.value())
		})
		.collect::<Vec<_>>();
	match value {
		ControlValue::Text(value) => available.iter().any(|option| option == value),
		ControlValue::SelectedValues(values) => values
			.iter()
			.all(|value| available.iter().any(|option| option == value)),
		ControlValue::Checked(_) => true,
	}
}

fn select_one_matches_expected_option(element: &Element, value: &ControlValue) -> bool {
	let ControlValue::Text(value) = value else {
		return true;
	};
	let Some(select) = element.as_web_sys().dyn_ref::<web_sys::HtmlSelectElement>() else {
		return true;
	};
	let options = select.options();
	for index in 0..options.length() {
		if let Some(option) = options
			.item(index)
			.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
			&& option.value() == *value
		{
			return option.selected();
		}
	}
	false
}

fn write_radio_value(
	element: &Element,
	binding: &ControlBinding,
) -> Result<(), ControlBindingError> {
	if binding.kind() != ControlKind::Radio {
		return Ok(());
	}
	let Some(value) = binding.radio_value() else {
		return Ok(());
	};
	let input = element
		.as_web_sys()
		.dyn_ref::<web_sys::HtmlInputElement>()
		.ok_or_else(|| missing(ControlKind::Radio, "value"))?;
	input.set_value(value);
	input.set_default_value(value);
	Ok(())
}

fn install_effect(
	element: Element,
	binding: ControlBinding,
	skip_first_write: bool,
	state: Rc<RefCell<CompositionState>>,
) -> Effect {
	let first_run = Rc::new(std::cell::Cell::new(skip_first_write));
	Effect::new_with_timing(
		move || {
			let value = binding.read();
			if first_run.replace(false) {
				return;
			}
			{
				let mut state = state.borrow_mut();
				if !state.applying_input
					&& let (ControlKind::Number, ControlValue::Text(raw)) = (binding.kind(), &value)
					&& let Some(editor) = &mut state.number_editor
				{
					editor.raw.clone_from(raw);
					editor.selection = Some(EditorSelection::collapsed(raw.len()));
					editor.pending_edit = None;
				}
			}
			if let Err(error) = write_control_and_reconcile(&element, &binding, &value) {
				web_sys::console::error_1(
					&format!("controlled input update failed: {error}").into(),
				);
			}
			crate::component::into_page::initialize_control_default(&element, &binding);
		},
		EffectTiming::Layout,
	)
}

fn install_listeners(
	element: &Element,
	binding: &ControlBinding,
	rejected_number_snapshot: Option<&RejectedNumberSnapshot>,
	number_position: Option<usize>,
) -> (Vec<EventHandle>, Rc<RefCell<CompositionState>>) {
	let number_editor = if binding.kind() == ControlKind::Number {
		rejected_number_snapshot
			.map(|snapshot| NumberEditorState {
				raw: snapshot.raw.clone(),
				selection: snapshot.selection,
				pending_edit: None,
			})
			.or_else(|| {
				read_control(element, ControlKind::Number)
					.ok()
					.and_then(|value| match value {
						ControlValue::Text(raw) => Some(NumberEditorState {
							selection: Some(EditorSelection::collapsed(raw.len())),
							raw,
							pending_edit: None,
						}),
						_ => None,
					})
			})
	} else {
		None
	};
	let state = Rc::new(RefCell::new(CompositionState {
		number_editor,
		number_position,
		..CompositionState::default()
	}));
	let mut listeners = Vec::new();

	match binding.kind() {
		ControlKind::Text | ControlKind::Number => {
			if binding.kind() == ControlKind::Number {
				let key_state = Rc::clone(&state);
				listeners.push(
					element.add_event_listener_with_event("keydown", move |event| {
						let Some(keyboard) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
							return;
						};
						let mut state = key_state.borrow_mut();
						let Some(editor) = &mut state.number_editor else {
							return;
						};
						if keyboard.default_prevented()
							|| keyboard.ctrl_key()
							|| keyboard.alt_key() || keyboard.meta_key()
						{
							editor.selection = None;
							editor.pending_edit = None;
							return;
						}
						move_number_selection(editor, &keyboard.key(), keyboard.shift_key());
					}),
				);

				for event_name in ["pointerdown", "mousedown"] {
					let pointer_state = Rc::clone(&state);
					listeners.push(element.add_event_listener_with_event(event_name, move |_| {
						let mut state = pointer_state.borrow_mut();
						if let Some(editor) = &mut state.number_editor {
							editor.selection = None;
							editor.pending_edit = None;
						}
					}));
				}

				let before_state = Rc::clone(&state);
				let before_element = element.clone();
				listeners.push(element.add_event_listener_with_event(
					"beforeinput",
					move |event| {
						let Some(input_event) = event.dyn_ref::<web_sys::InputEvent>() else {
							return;
						};
						let Some(input) = before_element
							.as_web_sys()
							.dyn_ref::<web_sys::HtmlInputElement>()
						else {
							return;
						};
						let mut state = before_state.borrow_mut();
						let Some(editor) = &mut state.number_editor else {
							return;
						};
						let live = input.value();
						if !live.is_empty() && live != editor.raw {
							editor.selection = infer_selection_after_live_edit(&editor.raw, &live);
							editor.raw = live;
						}
						let Some(selection) = input_selection(input).or(editor.selection) else {
							editor.pending_edit = None;
							return;
						};
						editor.pending_edit = edit_number_raw(
							&editor.raw,
							selection,
							&input_event.input_type(),
							input_event_text(input_event).as_deref(),
						);
					},
				));
			}

			let input_element = element.clone();
			let input_binding = binding.clone();
			let input_state = Rc::clone(&state);
			listeners.push(
				element.add_event_listener_with_event("input", move |event| {
					let browser_is_composing = event
						.dyn_ref::<web_sys::InputEvent>()
						.is_some_and(web_sys::InputEvent::is_composing);
					let composing = input_state.borrow().composing;
					if browser_is_composing || composing {
						capture_number_input_raw(&input_element, &input_state, false);
						input_state.borrow_mut().skip_next_input = None;
						return;
					}

					let allow_editor_fallback = input_state
						.borrow()
						.skip_next_input
						.as_ref()
						.is_some_and(|completed| match &completed.value {
							ControlValue::Text(raw) => !raw.is_empty(),
							ControlValue::Checked(_) | ControlValue::SelectedValues(_) => true,
						});
					let Ok(value) = read_input_event_value(
						&input_element,
						input_binding.kind(),
						&input_state,
						allow_editor_fallback,
					) else {
						return;
					};
					let completed = input_state.borrow_mut().skip_next_input.take();
					if let Some(completed) = completed
						&& completed.value == value
					{
						let current_value = untracked(|| input_binding.read());
						if current_value != completed.signal_baseline {
							let _ =
								write_control(&input_element, input_binding.kind(), &current_value);
						}
						return;
					}
					let _ = write_binding_from_input(&input_binding, &input_state, value);
				}),
			);

			let start_state = Rc::clone(&state);
			listeners.push(
				element.add_event_listener_with_event("compositionstart", move |_| {
					let mut state = start_state.borrow_mut();
					state.composing = true;
					state.skip_next_input = None;
				}),
			);

			let end_element = element.clone();
			let end_binding = binding.clone();
			let end_state = Rc::clone(&state);
			listeners.push(
				element.add_event_listener_with_event("compositionend", move |_| {
					{
						let mut state = end_state.borrow_mut();
						state.composing = false;
						state.skip_next_input = None;
					}
					let Ok(value) =
						read_input_event_value(&end_element, end_binding.kind(), &end_state, true)
					else {
						return;
					};
					let Ok(_) = write_binding_from_input(&end_binding, &end_state, value.clone())
					else {
						return;
					};
					let signal_baseline = untracked(|| end_binding.read());
					end_state.borrow_mut().skip_next_input = Some(CompletedComposition {
						value,
						signal_baseline,
					});
				}),
			);
		}
		ControlKind::Checkbox
		| ControlKind::Radio
		| ControlKind::SelectOne
		| ControlKind::SelectMany => {
			let change_element = element.clone();
			let change_binding = binding.clone();
			listeners.push(element.add_event_listener_with_event("change", move |_| {
				let Ok(value) = read_control(&change_element, change_binding.kind()) else {
					return;
				};
				let _ = change_binding.write(value);
			}));
		}
	}

	(listeners, state)
}

struct ApplyingInputGuard {
	state: Rc<RefCell<CompositionState>>,
}

impl ApplyingInputGuard {
	fn new(state: &Rc<RefCell<CompositionState>>) -> Self {
		state.borrow_mut().applying_input = true;
		Self {
			state: Rc::clone(state),
		}
	}
}

impl Drop for ApplyingInputGuard {
	fn drop(&mut self) {
		self.state.borrow_mut().applying_input = false;
	}
}

fn write_binding_from_input(
	binding: &ControlBinding,
	state: &Rc<RefCell<CompositionState>>,
	value: ControlValue,
) -> Result<crate::component::ControlWriteOutcome, ControlBindingError> {
	with_rejected_number_snapshot_transaction(|| {
		let snapshot = {
			let state = state.borrow();
			state
				.number_editor
				.as_ref()
				.map(|editor| (state.number_position, editor.raw.clone(), editor.selection))
		};
		let _guard = ApplyingInputGuard::new(state);
		batch(|| {
			let outcome = binding.write(value)?;
			if matches!(outcome, ControlWriteOutcome::Rejected(_))
				&& let Some((position, raw, selection)) = snapshot
			{
				stage_rejected_number_snapshot(binding, position, raw, selection);
			}
			Ok(outcome)
		})
	})
}

impl EditorSelection {
	fn collapsed(position: usize) -> Self {
		Self {
			start: position,
			end: position,
			anchor: position,
			focus: position,
		}
	}

	fn clamped(self, len: usize) -> Self {
		let start = self.start.min(len);
		let end = self.end.min(len).max(start);
		Self {
			start,
			end,
			anchor: self.anchor.min(len),
			focus: self.focus.min(len),
		}
	}
}

fn input_selection(input: &web_sys::HtmlInputElement) -> Option<EditorSelection> {
	let start = input.selection_start().ok().flatten()? as usize;
	let end = input.selection_end().ok().flatten()? as usize;
	Some(EditorSelection {
		start,
		end,
		anchor: start,
		focus: end,
	})
}

fn move_number_selection(editor: &mut NumberEditorState, key: &str, shift: bool) {
	let Some(selection) = editor.selection else {
		if key == "Home" {
			editor.selection = Some(EditorSelection::collapsed(0));
		} else if key == "End" {
			editor.selection = Some(EditorSelection::collapsed(editor.raw.len()));
		}
		return;
	};
	let position = if shift {
		match key {
			"ArrowLeft" => previous_char_boundary(&editor.raw, selection.focus),
			"ArrowRight" => next_char_boundary(&editor.raw, selection.focus),
			"Home" => 0,
			"End" => editor.raw.len(),
			_ => return,
		}
	} else {
		match key {
			"ArrowLeft" if selection.start != selection.end => selection.start,
			"ArrowLeft" => previous_char_boundary(&editor.raw, selection.focus),
			"ArrowRight" if selection.start != selection.end => selection.end,
			"ArrowRight" => next_char_boundary(&editor.raw, selection.focus),
			"Home" => 0,
			"End" => editor.raw.len(),
			_ => return,
		}
	};
	if shift {
		editor.selection = Some(EditorSelection {
			start: selection.anchor.min(position),
			end: selection.anchor.max(position),
			anchor: selection.anchor,
			focus: position,
		});
	} else {
		editor.selection = Some(EditorSelection::collapsed(position));
	}
}

fn infer_selection_after_live_edit(old: &str, new: &str) -> Option<EditorSelection> {
	if old == new {
		return None;
	}
	let prefix = old
		.bytes()
		.zip(new.bytes())
		.take_while(|(old, new)| old == new)
		.count();
	let max_suffix = old.len().min(new.len()).saturating_sub(prefix);
	let suffix = old
		.bytes()
		.rev()
		.zip(new.bytes().rev())
		.take(max_suffix)
		.take_while(|(old, new)| old == new)
		.count();
	Some(EditorSelection::collapsed(new.len() - suffix))
}

fn input_event_text(event: &web_sys::InputEvent) -> Option<String> {
	event.data().or_else(|| {
		event
			.data_transfer()
			.and_then(|transfer| transfer.get_data("text/plain").ok())
			.filter(|data| !data.is_empty())
	})
}

fn edit_number_raw(
	raw: &str,
	selection: EditorSelection,
	input_type: &str,
	data: Option<&str>,
) -> Option<PendingNumberEdit> {
	let selection = selection.clamped(raw.len());
	let mut edited = raw.to_owned();
	let (start, end) = (selection.start, selection.end);
	if input_type.starts_with("insert") {
		let data = data?;
		edited.replace_range(start..end, data);
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(start + data.len()),
		})
	} else if input_type == "deleteContentBackward" {
		let delete_start = if start == end {
			previous_char_boundary(raw, start)
		} else {
			start
		};
		edited.replace_range(delete_start..end, "");
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(delete_start),
		})
	} else if input_type == "deleteContentForward" {
		let delete_end = if start == end {
			next_char_boundary(raw, end)
		} else {
			end
		};
		edited.replace_range(start..delete_end, "");
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(start),
		})
	} else if matches!(
		input_type,
		"deleteWordBackward" | "deleteSoftLineBackward" | "deleteHardLineBackward"
	) {
		let delete_start = if start == end { 0 } else { start };
		edited.replace_range(delete_start..end, "");
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(delete_start),
		})
	} else if matches!(
		input_type,
		"deleteWordForward" | "deleteSoftLineForward" | "deleteHardLineForward"
	) {
		let delete_end = if start == end { raw.len() } else { end };
		edited.replace_range(start..delete_end, "");
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(start),
		})
	} else if input_type.starts_with("delete") && start != end {
		edited.replace_range(start..end, "");
		Some(PendingNumberEdit {
			raw: edited,
			selection: EditorSelection::collapsed(start),
		})
	} else {
		None
	}
}

fn previous_char_boundary(raw: &str, position: usize) -> usize {
	raw[..position]
		.char_indices()
		.next_back()
		.map_or(0, |(index, _)| index)
}

fn next_char_boundary(raw: &str, position: usize) -> usize {
	raw[position..]
		.char_indices()
		.nth(1)
		.map_or(raw.len(), |(index, _)| position + index)
}

fn capture_number_input_raw(
	element: &Element,
	state: &Rc<RefCell<CompositionState>>,
	allow_editor_fallback: bool,
) -> Option<ControlValue> {
	let input = element
		.as_web_sys()
		.dyn_ref::<web_sys::HtmlInputElement>()?;
	let live = input.value();
	let mut state = state.borrow_mut();
	let editor = state.number_editor.as_mut()?;
	let raw = if !live.is_empty() {
		editor.pending_edit = None;
		editor.selection = input_selection(input)
			.or_else(|| infer_selection_after_live_edit(&editor.raw, &live))
			.or(editor.selection);
		live
	} else if let Some(pending) = editor.pending_edit.take() {
		editor.selection = Some(pending.selection);
		pending.raw
	} else if allow_editor_fallback {
		editor.raw.clone()
	} else {
		editor.selection = None;
		String::new()
	};
	editor.raw.clone_from(&raw);
	Some(ControlValue::Text(raw))
}

fn read_input_event_value(
	element: &Element,
	kind: ControlKind,
	state: &Rc<RefCell<CompositionState>>,
	allow_editor_fallback: bool,
) -> Result<ControlValue, ControlBindingError> {
	if kind == ControlKind::Number
		&& let Some(value) = capture_number_input_raw(element, state, allow_editor_fallback)
	{
		return Ok(value);
	}
	read_control(element, kind)
}

pub(crate) fn validate_control(
	element: &Element,
	kind: ControlKind,
) -> Result<(), ControlBindingError> {
	let tag = element.as_web_sys().tag_name().to_ascii_lowercase();
	let supported = match kind {
		ControlKind::Text => {
			tag == "textarea"
				|| (tag == "input"
					&& element
						.as_web_sys()
						.dyn_ref::<web_sys::HtmlInputElement>()
						.is_some_and(|input| {
							crate::control_binding::is_text_input_type(&input.type_())
						}))
		}
		ControlKind::Number => {
			tag == "input"
				&& element
					.as_web_sys()
					.dyn_ref::<web_sys::HtmlInputElement>()
					.is_some_and(|input| {
						crate::control_binding::is_number_input_type(&input.type_())
					})
		}
		ControlKind::Checkbox => input_has_type(element, &tag, "checkbox"),
		ControlKind::Radio => input_has_type(element, &tag, "radio"),
		ControlKind::SelectOne => select_has_multiple(element, &tag, false),
		ControlKind::SelectMany => select_has_multiple(element, &tag, true),
	};
	if supported {
		Ok(())
	} else {
		Err(ControlBindingError::UnsupportedElement {
			control: kind,
			actual_tag: tag,
		})
	}
}

fn input_has_type(element: &Element, tag: &str, expected: &str) -> bool {
	tag == "input"
		&& element
			.as_web_sys()
			.dyn_ref::<web_sys::HtmlInputElement>()
			.is_some_and(|input| input.type_() == expected)
}

fn select_has_multiple(element: &Element, tag: &str, expected: bool) -> bool {
	tag == "select"
		&& element
			.as_web_sys()
			.dyn_ref::<web_sys::HtmlSelectElement>()
			.is_some_and(|select| select.multiple() == expected)
}

pub(crate) fn read_control(
	element: &Element,
	kind: ControlKind,
) -> Result<ControlValue, ControlBindingError> {
	validate_control(element, kind)?;
	match kind {
		ControlKind::Text => {
			if let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() {
				Ok(ControlValue::Text(input.value()))
			} else if let Some(textarea) = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlTextAreaElement>()
			{
				Ok(ControlValue::Text(textarea.value()))
			} else {
				Err(missing(kind, "value"))
			}
		}
		ControlKind::Number => element
			.as_web_sys()
			.dyn_ref::<web_sys::HtmlInputElement>()
			.map(|input| ControlValue::Text(input.value()))
			.ok_or_else(|| missing(kind, "value")),
		ControlKind::Checkbox | ControlKind::Radio => element
			.as_web_sys()
			.dyn_ref::<web_sys::HtmlInputElement>()
			.map(|input| ControlValue::Checked(input.checked()))
			.ok_or_else(|| missing(kind, "checked")),
		ControlKind::SelectOne => element
			.as_web_sys()
			.dyn_ref::<web_sys::HtmlSelectElement>()
			.map(|select| ControlValue::Text(select.value()))
			.ok_or_else(|| missing(kind, "value")),
		ControlKind::SelectMany => {
			let select = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlSelectElement>()
				.ok_or_else(|| missing(kind, "selectedOptions"))?;
			let options = select.options();
			let mut values = Vec::new();
			for index in 0..options.length() {
				if let Some(option) = options.item(index)
					&& let Ok(option) = option.dyn_into::<web_sys::HtmlOptionElement>()
					&& option.selected()
				{
					values.push(option.value());
				}
			}
			Ok(ControlValue::SelectedValues(values))
		}
	}
}

pub(crate) fn write_control(
	element: &Element,
	kind: ControlKind,
	value: &ControlValue,
) -> Result<bool, ControlBindingError> {
	validate_control(element, kind)?;
	match (kind, value) {
		(ControlKind::Text, ControlValue::Text(value)) => {
			if let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() {
				if input.type_().eq_ignore_ascii_case("password") {
					let _ = element.remove_attribute("value");
				}
				if input.value() == *value {
					Ok(false)
				} else {
					input.set_value(value);
					Ok(true)
				}
			} else if let Some(textarea) = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlTextAreaElement>()
			{
				if textarea.value() == *value {
					Ok(false)
				} else {
					textarea.set_value(value);
					Ok(true)
				}
			} else {
				Err(missing(kind, "value"))
			}
		}
		(ControlKind::Number, ControlValue::Text(value)) => {
			let input = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlInputElement>()
				.ok_or_else(|| missing(kind, "value"))?;
			if input.value() == *value {
				Ok(false)
			} else {
				input.set_value(value);
				Ok(true)
			}
		}
		(ControlKind::Checkbox | ControlKind::Radio, ControlValue::Checked(value)) => {
			let input = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlInputElement>()
				.ok_or_else(|| missing(kind, "checked"))?;
			if input.checked() == *value {
				Ok(false)
			} else {
				input.set_checked(*value);
				Ok(true)
			}
		}
		(ControlKind::SelectOne, ControlValue::Text(value)) => {
			let select = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlSelectElement>()
				.ok_or_else(|| missing(kind, "value"))?;
			let options = select.options();
			let mut selected = false;
			let mut changed = false;
			for index in 0..options.length() {
				if let Some(option) = options
					.item(index)
					.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
				{
					let matches = !selected && option.value() == *value;
					selected |= matches;
					if option.selected() != matches {
						option.set_selected(matches);
						changed = true;
					}
				}
			}
			Ok(changed)
		}
		(ControlKind::SelectMany, ControlValue::SelectedValues(values)) => {
			let select = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlSelectElement>()
				.ok_or_else(|| missing(kind, "selectedOptions"))?;
			let options = select.options();
			let mut changed = false;
			for index in 0..options.length() {
				if let Some(option) = options.item(index)
					&& let Ok(option) = option.dyn_into::<web_sys::HtmlOptionElement>()
				{
					let selected = values.iter().any(|value| value == &option.value());
					if option.selected() != selected {
						option.set_selected(selected);
						changed = true;
					}
				}
			}
			Ok(changed)
		}
		(_, actual) => Err(ControlBindingError::ValueKindMismatch {
			control: kind,
			actual: match actual {
				ControlValue::Text(_) => "text",
				ControlValue::Checked(_) => "checked",
				ControlValue::SelectedValues(_) => "selected-values",
			},
		}),
	}
}

fn write_control_and_reconcile(
	element: &Element,
	binding: &ControlBinding,
	value: &ControlValue,
) -> Result<(), ControlBindingError> {
	write_control(element, binding.kind(), value)?;
	if matches!(binding.kind(), ControlKind::Text | ControlKind::Number) {
		let live_value = read_control(element, binding.kind())?;
		if live_value != *value {
			if preserves_unrepresentable_temporal_value(element, value, &live_value) {
				return Ok(());
			}
			if has_conflicting_range_binding(element, binding) {
				// Disjoint range controls cannot both normalize one shared signal.
				// Keep their browser values local; writing back here would make their
				// layout effects alternate forever.
				return Ok(());
			}
			if let ControlWriteOutcome::Rejected(error) = binding.write(live_value)? {
				return Err(ControlBindingError::RejectedValue {
					control: binding.kind(),
					error,
				});
			}
		}
	}
	Ok(())
}

fn range_constraints(element: &web_sys::Element) -> Option<(f64, f64, Option<f64>, f64)> {
	let input = element.dyn_ref::<web_sys::HtmlInputElement>()?;
	if !input.type_().eq_ignore_ascii_case("range") {
		return None;
	}
	let min_attribute = element
		.get_attribute("min")
		.and_then(|value| crate::control_binding::parse_html_number(&value));
	let min = min_attribute.unwrap_or(0.0);
	let max = element
		.get_attribute("max")
		.and_then(|value| crate::control_binding::parse_html_number(&value))
		.unwrap_or(100.0);
	let step = match element.get_attribute("step").as_deref() {
		Some(value) if value.eq_ignore_ascii_case("any") => None,
		Some(value) => crate::control_binding::parse_html_number(value)
			.filter(|value| value.is_finite() && *value > 0.0)
			.or(Some(1.0)),
		None => Some(1.0),
	};
	let step_base = min_attribute
		.or_else(|| {
			element
				.get_attribute("value")
				.and_then(|value| crate::control_binding::parse_html_number(&value))
		})
		.unwrap_or(0.0);
	Some((min, max.max(min), step, step_base))
}

fn has_conflicting_range_binding(element: &Element, binding: &ControlBinding) -> bool {
	if binding.kind() != ControlKind::Number {
		return false;
	}
	let Some((min, max, step, step_base)) = range_constraints(element.as_web_sys()) else {
		return false;
	};
	let node: web_sys::Node = element.as_web_sys().clone().unchecked_into();
	ACTIVE_NUMBER_BINDINGS.with(|registered| {
		registered.borrow().iter().any(|candidate| {
			candidate.target == binding.target()
				&& !candidate
					.element
					.clone()
					.unchecked_into::<web_sys::Node>()
					.is_same_node(Some(&node))
				&& range_constraints(&candidate.element).is_some_and(|candidate_constraints| {
					crate::control_binding::range_constraints_conflict(
						(min, max, step, step_base),
						candidate_constraints,
					)
				})
		})
	})
}

fn preserves_unrepresentable_temporal_value(
	element: &Element,
	expected: &ControlValue,
	live: &ControlValue,
) -> bool {
	let (ControlValue::Text(expected), ControlValue::Text(live)) = (expected, live) else {
		return false;
	};
	if expected.is_empty() || !live.is_empty() {
		return false;
	}
	let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() else {
		return false;
	};
	["date", "datetime-local", "month", "time", "week"]
		.iter()
		.any(|input_type| input.type_().eq_ignore_ascii_case(input_type))
}

pub(crate) fn reconcile_control_binding(
	element: &Element,
	binding: &ControlBinding,
) -> Result<(), ControlBindingError> {
	let value = untracked(|| binding.read());
	write_control_and_reconcile(element, binding, &value)?;
	crate::component::into_page::initialize_control_default(element, binding);
	Ok(())
}

fn missing(control: ControlKind, property: &'static str) -> ControlBindingError {
	ControlBindingError::MissingProperty { control, property }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::{
		ControlBinding, ControlBindingError, ControlKind, NumberParseErrorKind,
	};
	use crate::dom::Element;
	use crate::reactive::{ReactiveScope, Signal};
	use wasm_bindgen::JsCast;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	fn element(tag: &str) -> Element {
		let raw = web_sys::window()
			.expect("window")
			.document()
			.expect("document")
			.create_element(tag)
			.expect("element");
		Element::new(raw)
	}

	#[wasm_bindgen_test]
	fn mounted_text_control_synchronizes_both_directions() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new("signal".to_owned());
			let _controller =
				ControlBindingController::mount(element, ControlBinding::text(signal.clone()))
					.expect("binding");

			assert_eq!(input.value(), "signal");
			input.set_value("dom");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
				.expect("dispatch");
			assert_eq!(signal.get(), "dom");
			signal.set("updated".to_owned());
			assert_eq!(input.value(), "updated");
		});
	}

	#[wasm_bindgen_test]
	fn hydration_adopts_live_dom_without_initial_write() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_value("restored");
			input.set_selection_range(3, 3).expect("selection");
			let signal = Signal::new("server".to_owned());

			let _controller =
				ControlBindingController::hydrate(element, ControlBinding::text(signal.clone()))
					.expect("binding");

			assert_eq!(signal.get(), "restored");
			assert_eq!(input.selection_start().expect("selection"), Some(3));
		});
	}

	#[wasm_bindgen_test]
	fn hydration_adoption_updates_text_default_for_form_reset() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let form = element("form");
			let input = element("input");
			let raw_input: web_sys::HtmlInputElement = input.as_web_sys().clone().unchecked_into();
			raw_input.set_default_value("server");
			raw_input.set_value("restored");
			form.as_web_sys()
				.append_child(input.as_web_sys())
				.expect("append");
			let signal = Signal::new("server".to_owned());

			let _controller =
				ControlBindingController::hydrate(input, ControlBinding::text(signal.clone()))
					.expect("binding");
			form.as_web_sys()
				.clone()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();

			assert_eq!(signal.get(), "restored");
			assert_eq!(raw_input.value(), "restored");
		});
	}

	#[wasm_bindgen_test]
	fn hydration_adoption_updates_checked_default_for_form_reset() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let form = element("form");
			let input = element("input");
			let raw_input: web_sys::HtmlInputElement = input.as_web_sys().clone().unchecked_into();
			raw_input.set_type("checkbox");
			raw_input.set_default_checked(false);
			raw_input.set_checked(true);
			form.as_web_sys()
				.append_child(input.as_web_sys())
				.expect("append");
			let signal = Signal::new(false);

			let _controller =
				ControlBindingController::hydrate(input, ControlBinding::checkbox(signal.clone()))
					.expect("binding");
			form.as_web_sys()
				.clone()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();

			assert!(signal.get());
			assert!(raw_input.checked());
		});
	}

	#[wasm_bindgen_test]
	fn hydration_adoption_updates_option_defaults_for_form_reset() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let form = element("form");
			let select = element("select");
			let raw_select: web_sys::HtmlSelectElement =
				select.as_web_sys().clone().unchecked_into();
			for (value, selected) in [("server", true), ("restored", false)] {
				let option: web_sys::HtmlOptionElement = web_sys::window()
					.expect("window")
					.document()
					.expect("document")
					.create_element("option")
					.expect("option")
					.unchecked_into();
				option.set_value(value);
				option.set_default_selected(selected);
				raw_select.append_child(&option).expect("append option");
			}
			raw_select.set_value("restored");
			form.as_web_sys()
				.append_child(select.as_web_sys())
				.expect("append");
			let signal = Signal::new("server".to_owned());

			let _controller = ControlBindingController::hydrate(
				select,
				ControlBinding::select_one(signal.clone()),
			)
			.expect("binding");
			form.as_web_sys()
				.clone()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();

			assert_eq!(signal.get(), "restored");
			assert_eq!(raw_select.value(), "restored");
		});
	}

	#[wasm_bindgen_test]
	fn hydration_restores_the_first_matching_duplicate_select_option() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let select = element("select");
			let raw_select: web_sys::HtmlSelectElement =
				select.as_web_sys().clone().unchecked_into();
			for index in 0..2 {
				let option: web_sys::HtmlOptionElement = web_sys::window()
					.expect("window")
					.document()
					.expect("document")
					.create_element("option")
					.expect("option")
					.unchecked_into();
				option.set_value("duplicate");
				option.set_text(&format!("Option {index}"));
				raw_select.append_child(&option).expect("append option");
			}
			raw_select.set_selected_index(1);
			let signal = Signal::new("duplicate".to_owned());

			let _controller = ControlBindingController::hydrate(
				select,
				ControlBinding::select_one(signal.clone()),
			)
			.expect("binding");

			assert_eq!(signal.get(), "duplicate");
			assert_eq!(raw_select.selected_index(), 0);
		});
	}

	#[wasm_bindgen_test]
	fn same_value_signal_write_preserves_caret() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new("hello".to_owned());
			let _controller =
				ControlBindingController::mount(element, ControlBinding::text(signal.clone()))
					.expect("binding");
			input.set_selection_range(2, 2).expect("selection");

			signal.set("hello".to_owned());

			assert_eq!(input.selection_start().expect("selection"), Some(2));
		});
	}

	#[wasm_bindgen_test]
	fn invalid_numeric_raw_is_preserved_until_signal_changes() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("number");
			let signal = Signal::new(12_i32);
			let _controller =
				ControlBindingController::mount(element, ControlBinding::number(signal.clone()))
					.expect("binding");
			input.set_value("2147483648");

			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
				.expect("dispatch");

			assert_eq!(signal.get(), 12);
			assert_eq!(input.value(), "2147483648");
			signal.set(13);
			assert_eq!(input.value(), "13");
		});
	}

	#[wasm_bindgen_test]
	fn rejected_numeric_raw_restores_to_its_original_control_position() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			// Arrange
			let value = Signal::new(12_i32);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("number");
			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("number");
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value))
					.expect("first binding");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value))
					.expect("second binding");

			// Act
			with_rejected_number_snapshot_transaction(|| {
				second_input.set_value("2147483648");
				second_input
					.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
					.expect("dispatch");
				drop(first_controller);
				drop(second_controller);

				let replacement_first = element("input");
				let replacement_first_input: web_sys::HtmlInputElement =
					replacement_first.as_web_sys().clone().unchecked_into();
				replacement_first_input.set_type("number");
				let _replacement_first_controller = ControlBindingController::mount(
					replacement_first,
					ControlBinding::number(value),
				)
				.expect("replacement first binding");

				let replacement_second = element("input");
				let replacement_second_input: web_sys::HtmlInputElement =
					replacement_second.as_web_sys().clone().unchecked_into();
				replacement_second_input.set_type("number");
				let _replacement_second_controller = ControlBindingController::mount(
					replacement_second,
					ControlBinding::number(value),
				)
				.expect("replacement second binding");

				// Assert
				assert_eq!(replacement_first_input.value(), "12");
				assert_eq!(replacement_second_input.value(), "2147483648");
				Ok::<_, ControlBindingError>(())
			})
			.expect("rejected snapshot transaction");
		});
	}

	#[wasm_bindgen_test]
	fn rejected_numeric_raw_keeps_position_when_another_control_remounts() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			// Arrange
			let value = Signal::new(12_i32);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("number");
			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("number");
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value.clone()))
					.expect("first binding");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value.clone()))
					.expect("second binding");

			// Act: remount only the first control, then remount the second one.
			with_rejected_number_snapshot_transaction(|| {
				second_input.set_value("2147483648");
				second_input
					.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
					.expect("dispatch");
				drop(first_controller);
				let replacement_first = element("input");
				let replacement_first_input: web_sys::HtmlInputElement =
					replacement_first.as_web_sys().clone().unchecked_into();
				replacement_first_input.set_type("number");
				let replacement_first_controller = ControlBindingController::mount(
					replacement_first,
					ControlBinding::number(value.clone()),
				)
				.expect("replacement first binding");

				drop(second_controller);
				let replacement_second = element("input");
				let replacement_second_input: web_sys::HtmlInputElement =
					replacement_second.as_web_sys().clone().unchecked_into();
				replacement_second_input.set_type("number");
				let _replacement_second_controller = ControlBindingController::mount(
					replacement_second,
					ControlBinding::number(value),
				)
				.expect("replacement second binding");

				// Assert
				assert_eq!(replacement_first_input.value(), "12");
				assert_eq!(replacement_second_input.value(), "2147483648");
				drop(replacement_first_controller);
				Ok::<_, ControlBindingError>(())
			})
			.expect("rejected snapshot transaction");
		});
	}

	#[wasm_bindgen_test]
	fn committed_numeric_raw_does_not_restore_on_remount() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let value = Signal::new(1_i32);
			let input = element("input");
			let raw_input: web_sys::HtmlInputElement = input.as_web_sys().clone().unchecked_into();
			raw_input.set_type("number");
			let controller =
				ControlBindingController::mount(input, ControlBinding::number(value.clone()))
					.expect("binding");

			with_rejected_number_snapshot_transaction(|| {
				raw_input.set_value("001");
				raw_input
					.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
					.expect("dispatch");
				drop(controller);

				let replacement = element("input");
				let replacement_input: web_sys::HtmlInputElement =
					replacement.as_web_sys().clone().unchecked_into();
				replacement_input.set_type("number");
				let _replacement_controller = ControlBindingController::mount(
					replacement,
					ControlBinding::number(value.clone()),
				)
				.expect("replacement binding");

				assert_eq!(value.get(), 1);
				assert_eq!(replacement_input.value(), "1");
				Ok::<_, ControlBindingError>(())
			})
			.expect("rejected snapshot transaction");
		});
	}

	#[wasm_bindgen_test]
	fn checkbox_radio_and_select_kinds_synchronize() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let checkbox = element("input");
			let checkbox_input: web_sys::HtmlInputElement =
				checkbox.as_web_sys().clone().unchecked_into();
			checkbox_input.set_type("checkbox");
			let checked = Signal::new(false);
			let _checkbox_controller = ControlBindingController::mount(
				checkbox,
				ControlBinding::checkbox(checked.clone()),
			)
			.expect("checkbox");
			checkbox_input.set_checked(true);
			checkbox_input
				.dispatch_event(&web_sys::Event::new("change").expect("change"))
				.expect("dispatch");
			assert!(checked.get());

			let radio = element("input");
			let radio_input: web_sys::HtmlInputElement =
				radio.as_web_sys().clone().unchecked_into();
			radio_input.set_type("radio");
			let selected = Signal::new("other".to_owned());
			let _radio_controller = ControlBindingController::mount(
				radio,
				ControlBinding::radio(selected.clone(), "choice".to_owned()),
			)
			.expect("radio");
			radio_input.set_checked(true);
			radio_input
				.dispatch_event(&web_sys::Event::new("change").expect("change"))
				.expect("dispatch");
			assert_eq!(selected.get(), "choice");

			let select = element("select");
			let select_input: web_sys::HtmlSelectElement =
				select.as_web_sys().clone().unchecked_into();
			select_input.set_multiple(true);
			for value in ["a", "b", "c"] {
				let option = web_sys::window()
					.expect("window")
					.document()
					.expect("document")
					.create_element("option")
					.expect("option");
				let option: web_sys::HtmlOptionElement = option.unchecked_into();
				option.set_value(value);
				select_input.append_child(&option).expect("append option");
			}
			let selected_many = Signal::new(vec!["b".to_owned()]);
			let _select_controller = ControlBindingController::mount(
				select,
				ControlBinding::select_many(selected_many.clone()),
			)
			.expect("select");
			assert!(
				select_input
					.options()
					.item(1)
					.expect("option")
					.unchecked_into::<web_sys::HtmlOptionElement>()
					.selected()
			);

			let select_one = element("select");
			let select_one_input: web_sys::HtmlSelectElement =
				select_one.as_web_sys().clone().unchecked_into();
			for value in ["first", "second"] {
				let option = web_sys::window()
					.expect("window")
					.document()
					.expect("document")
					.create_element("option")
					.expect("option");
				let option: web_sys::HtmlOptionElement = option.unchecked_into();
				option.set_value(value);
				select_one_input
					.append_child(&option)
					.expect("append option");
			}
			let selected_one = Signal::new("second".to_owned());
			let _select_one_controller = ControlBindingController::mount(
				select_one,
				ControlBinding::select_one(selected_one.clone()),
			)
			.expect("select one");
			assert_eq!(select_one_input.value(), "second");
		});
	}

	#[wasm_bindgen_test]
	fn hydration_restores_an_ignored_false_radio_to_the_expected_state() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			// Arrange
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("radio");
			input.set_checked(false);
			let selected = Signal::new("choice".to_owned());

			// Act
			let _controller = ControlBindingController::hydrate(
				element,
				ControlBinding::radio(selected.clone(), "choice".to_owned()),
			)
			.expect("binding");

			// Assert
			assert!(input.checked());
			assert_eq!(selected.get(), "choice");
		});
	}

	#[wasm_bindgen_test]
	fn dropping_controller_detaches_listeners_and_effect() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new("initial".to_owned());
			let controller =
				ControlBindingController::mount(element, ControlBinding::text(signal.clone()))
					.expect("binding");
			drop(controller);

			input.set_value("dom");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input"))
				.expect("dispatch");
			assert_eq!(signal.get(), "initial");
			signal.set("signal".to_owned());
			assert_eq!(input.value(), "dom");
		});
	}

	#[wasm_bindgen_test]
	fn composition_commits_once_and_deduplicates_final_input() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new(String::new());
			let commits = Rc::new(std::cell::Cell::new(0));
			let effect_signal = signal.clone();
			let effect_commits = Rc::clone(&commits);
			let _commit_observer = Effect::new_with_timing(
				move || {
					let _ = effect_signal.get();
					effect_commits.set(effect_commits.get() + 1);
				},
				EffectTiming::Layout,
			);
			let _controller =
				ControlBindingController::mount(element, ControlBinding::text(signal.clone()))
					.expect("binding");
			input
				.dispatch_event(&web_sys::CompositionEvent::new("compositionstart").expect("start"))
				.expect("dispatch");
			input.set_value("あ");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input"))
				.expect("dispatch");
			assert_eq!(signal.get(), "");
			input
				.dispatch_event(&web_sys::CompositionEvent::new("compositionend").expect("end"))
				.expect("dispatch");
			assert_eq!(signal.get(), "あ");
			input.set_selection_range(0, 0).expect("selection");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input"))
				.expect("dispatch");
			assert_eq!(signal.get(), "あ");
			assert_eq!(commits.get(), 2);
			assert_eq!(input.selection_start().expect("selection"), Some(0));
		});
	}

	#[wasm_bindgen_test]
	fn isolated_composing_input_invalidates_stale_composition_dedupe() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new(String::new());
			let commits = Rc::new(std::cell::Cell::new(0));
			let effect_signal = signal.clone();
			let effect_commits = Rc::clone(&commits);
			let _commit_observer = Effect::new_with_timing(
				move || {
					let _ = effect_signal.get();
					effect_commits.set(effect_commits.get() + 1);
				},
				EffectTiming::Layout,
			);
			let _controller =
				ControlBindingController::mount(element, ControlBinding::text(signal.clone()))
					.expect("binding");
			input.set_value("same");
			input
				.dispatch_event(&web_sys::CompositionEvent::new("compositionend").expect("end"))
				.expect("dispatch");
			input
				.dispatch_event(&{
					let init = web_sys::InputEventInit::new();
					init.set_is_composing(true);
					web_sys::InputEvent::new_with_event_init_dict("input", &init)
						.expect("composing input")
						.into()
				})
				.expect("dispatch");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("input"))
				.expect("dispatch");
			assert_eq!(signal.get(), "same");
			assert_eq!(commits.get(), 3);
		});
	}

	#[wasm_bindgen_test]
	fn actual_tag_mismatch_is_structured() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let signal = Signal::new(false);
			let error = ControlBindingController::mount(
				element("select"),
				ControlBinding::checkbox(signal),
			)
			.expect_err("mismatch");
			assert_eq!(
				error,
				ControlBindingError::UnsupportedElement {
					control: ControlKind::Checkbox,
					actual_tag: "select".to_owned(),
				}
			);
		});
	}

	#[wasm_bindgen_test]
	fn text_binding_accepts_supported_input_types() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			for (input_type, value) in [
				("text", "non-empty"),
				("search", "non-empty"),
				("tel", "non-empty"),
				("url", "non-empty"),
				("email", "non-empty"),
				("password", "non-empty"),
				("color", "#123456"),
				("date", "2026-09-01"),
				("datetime-local", "2026-09-01T12:34"),
				("month", "2026-09"),
				("week", "2026-W36"),
				("time", "12:34"),
			] {
				let element = element("input");
				let input: web_sys::HtmlInputElement =
					element.as_web_sys().clone().unchecked_into();
				input.set_type(input_type);
				let _controller = ControlBindingController::mount(
					element,
					ControlBinding::text(Signal::new(value.to_owned())),
				)
				.expect("supported text input type should mount");
				assert_eq!(input.value(), value);
			}
		});
	}

	#[wasm_bindgen_test]
	fn hydration_restores_a_password_value_omitted_by_ssr() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("password");
			element
				.set_attribute(
					crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE,
					"true",
				)
				.expect("marker");
			let value = Signal::new("secret".to_owned());

			let (controller, refresh_required) =
				ControlBindingController::hydrate(element, ControlBinding::text(value.clone()))
					.expect("password binding");

			assert!(!refresh_required);
			assert_eq!(value.get(), "secret");
			assert_eq!(input.value(), "secret");
			assert_eq!(
				input.get_attribute(crate::control_binding::SSR_OMITTED_PASSWORD_ATTRIBUTE),
				None
			);
			drop(controller);
		});
	}

	#[wasm_bindgen_test]
	fn bound_password_initialization_only_sets_the_live_value() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("password");
			let binding = ControlBinding::text(Signal::new("secret".to_owned()));

			crate::component::into_page::initialize_control_default(&element, &binding);

			assert_eq!(input.value(), "secret");
			assert_eq!(input.default_value(), "");
			assert_eq!(input.get_attribute("value"), None);
		});
	}

	#[wasm_bindgen_test]
	fn mount_rejects_unrepresentable_browser_normalized_range_value() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("range");
			input.set_min("300");
			input.set_max("400");

			let error = ControlBindingController::mount(
				element,
				ControlBinding::number(Signal::new(100_u8)),
			)
			.expect_err("range value should be representable by the binding");

			assert!(matches!(
				error,
				ControlBindingError::RejectedValue {
					control: ControlKind::Number,
					error
				} if error.raw() == "300" && error.kind() == NumberParseErrorKind::OutOfRange
			));
		});
	}

	#[wasm_bindgen_test]
	fn conflicting_range_bindings_do_not_write_back_forever() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let value = Signal::new(150_i32);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("range");
			first_input.set_min("0");
			first_input.set_max("100");
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value.clone()))
					.expect("first range binding");

			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("range");
			second_input.set_min("200");
			second_input.set_max("400");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value.clone()))
					.expect("second range binding");

			assert_eq!(value.get(), 100);
			assert_eq!(first_input.value(), "100");
			assert_eq!(second_input.value(), "200");

			value.set(150);

			assert_eq!(value.get(), 150);
			assert_eq!(first_input.value(), "100");
			assert_eq!(second_input.value(), "200");
			drop(second_controller);
			drop(first_controller);
		});
	}

	#[wasm_bindgen_test]
	fn incompatible_range_step_grids_do_not_write_back_forever() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let value = Signal::new(3_i32);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("range");
			first_input.set_min("0");
			first_input.set_max("10");
			first_input.set_step("2");
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value.clone()))
					.expect("first range binding");

			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("range");
			second_input.set_min("1");
			second_input.set_max("10");
			second_input.set_step("2");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value.clone()))
					.expect("second range binding");

			assert_eq!(value.get(), 4);
			assert_eq!(first_input.value(), "4");
			drop(second_controller);
			drop(first_controller);
		});
	}

	#[wasm_bindgen_test]
	fn overlapping_range_step_grids_without_a_common_value_do_not_write_back_forever() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			// Arrange
			let value = Signal::new(3_i32);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("range");
			first_input.set_min("0");
			first_input.set_max("5");
			first_input.set_step("4");
			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("range");
			second_input.set_min("2");
			second_input.set_max("5");
			second_input.set_step("6");

			// Act
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value.clone()))
					.expect("first range binding");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value.clone()))
					.expect("second range binding");

			// Assert
			assert_eq!(value.get(), 4);
			assert_eq!(first_input.value(), "4");
			assert_eq!(second_input.value(), "2");
			drop(second_controller);
			drop(first_controller);
		});
	}

	#[wasm_bindgen_test]
	fn continuous_and_stepped_ranges_without_a_shared_value_keep_browser_values_local() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			// Arrange
			let value = Signal::new(0.5_f64);
			let first = element("input");
			let first_input: web_sys::HtmlInputElement =
				first.as_web_sys().clone().unchecked_into();
			first_input.set_type("range");
			first_input.set_min("0.5");
			first_input.set_max("0.6");
			first_input.set_step("any");
			let second = element("input");
			let second_input: web_sys::HtmlInputElement =
				second.as_web_sys().clone().unchecked_into();
			second_input.set_type("range");
			second_input.set_min("0");
			second_input.set_max("0.6");
			second_input.set_step("1");

			// Act
			let first_controller =
				ControlBindingController::mount(first, ControlBinding::number(value))
					.expect("continuous range binding");
			let second_controller =
				ControlBindingController::mount(second, ControlBinding::number(value))
					.expect("stepped range binding");

			// Assert
			assert_eq!(
				(value.get(), first_input.value(), second_input.value()),
				(0.5, "0.5".to_owned(), "0".to_owned())
			);
			drop(second_controller);
			drop(first_controller);
		});
	}

	#[wasm_bindgen_test]
	fn text_binding_rejects_unsupported_input_types() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			for input_type in ["file", "range"] {
				let element = element("input");
				let input: web_sys::HtmlInputElement =
					element.as_web_sys().clone().unchecked_into();
				input.set_type(input_type);
				let error = ControlBindingController::mount(
					element,
					ControlBinding::text(Signal::new("non-empty".to_owned())),
				)
				.expect_err("unsupported input type should fail");

				assert_eq!(
					error,
					ControlBindingError::UnsupportedElement {
						control: ControlKind::Text,
						actual_tag: "input".to_owned(),
					}
				);
				if input_type == "file" {
					assert_eq!(input.value(), "");
				}
			}
		});
	}

	#[wasm_bindgen_test]
	fn number_binding_accepts_range_input_type() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("input");
			let input: web_sys::HtmlInputElement = element.as_web_sys().clone().unchecked_into();
			input.set_type("range");
			let _controller = ControlBindingController::mount(
				element,
				ControlBinding::number(Signal::new(10_i32)),
			)
			.expect("range input type should mount");
			assert_eq!(input.value(), "10");
		});
	}

	#[wasm_bindgen_test]
	fn text_binding_accepts_textarea() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			let element = element("textarea");
			let textarea: web_sys::HtmlTextAreaElement =
				element.as_web_sys().clone().unchecked_into();
			let signal = Signal::new("bound".to_owned());

			let _controller =
				ControlBindingController::mount(element, ControlBinding::text(signal))
					.expect("textarea binding");

			assert_eq!(textarea.value(), "bound");
		});
	}
}
