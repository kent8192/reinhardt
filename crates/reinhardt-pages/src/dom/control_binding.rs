//! Retained property synchronization for generated form controls.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use wasm_bindgen::{JsCast, closure::Closure};

use crate::component::{ControlBinding, ControlKind, ControlValue};
use crate::dom::Element;
use crate::reactive::{Effect, EffectTiming, batch};

thread_local! {
	static MOUNTED_CONTROLS: RefCell<Vec<Weak<MountedControl>>> = const { RefCell::new(Vec::new()) };
	static RESET_LISTENER: RefCell<Weak<FormResetListener>> = const { RefCell::new(Weak::new()) };
}

struct MountedControl {
	element: Element,
	binding: ControlBinding,
	active: Cell<bool>,
}

/// Owns the subscriptions and browser callbacks for one mounted control.
pub(crate) struct ControlBindingController {
	control: Rc<MountedControl>,
	_effect: Effect,
	_reset_listener: Rc<FormResetListener>,
	_option_observer: Option<SelectOptionObserver>,
}

impl Drop for ControlBindingController {
	fn drop(&mut self) {
		self.control.active.set(false);
		MOUNTED_CONTROLS.with(|controls| {
			controls.borrow_mut().retain(|control| {
				control
					.upgrade()
					.is_some_and(|control| control.active.get())
			});
		});
	}
}

impl ControlBindingController {
	pub(crate) fn mount(element: Element, binding: ControlBinding) -> Self {
		Self::install(element, binding, false)
	}

	fn install(element: Element, binding: ControlBinding, skip_first_write: bool) -> Self {
		let control = Rc::new(MountedControl {
			element,
			binding,
			active: Cell::new(true),
		});
		let effect_control = control.clone();
		let mut first_run = true;
		let effect = Effect::new_with_timing(
			move || {
				let value = effect_control.binding.read();
				let initial_run = std::mem::take(&mut first_run);
				if !(initial_run && skip_first_write) {
					write_control(
						&effect_control.element,
						effect_control.binding.kind(),
						&value,
					);
				}
			},
			EffectTiming::Layout,
		);
		MOUNTED_CONTROLS.with(|controls| controls.borrow_mut().push(Rc::downgrade(&control)));
		let reset_listener = FormResetListener::shared();
		let option_observer = SelectOptionObserver::install(&control);
		Self {
			control,
			_effect: effect,
			_reset_listener: reset_listener,
			_option_observer: option_observer,
		}
	}
}

/// Snapshots all controls before adopting any signal, preserving radio/group ordering.
pub(crate) fn hydrate_controls(
	controls: Vec<(Element, ControlBinding)>,
) -> Vec<ControlBindingController> {
	let snapshots = controls
		.iter()
		.map(|(element, binding)| {
			let prefer_source = binding.source_preferred_on_hydration()
				|| !select_contains_source(element, &binding.read_untracked());
			(read_control(element, binding.kind()), prefer_source)
		})
		.collect::<Vec<_>>();
	batch(|| {
		for ((_, binding), (snapshot, prefer_source)) in controls.iter().zip(&snapshots) {
			if !prefer_source
				&& let Some(snapshot) = snapshot
				&& *snapshot != binding.read_untracked()
			{
				binding.write(snapshot.clone());
			}
		}
	});
	controls
		.into_iter()
		.zip(snapshots)
		.map(|((element, binding), (_, prefer_source))| {
			ControlBindingController::install(element, binding, !prefer_source)
		})
		.collect()
}

fn select_contains_source(element: &Element, value: &ControlValue) -> bool {
	let Some(select) = element.as_web_sys().dyn_ref::<web_sys::HtmlSelectElement>() else {
		return true;
	};
	let options = select.options();
	let values = (0..options.length())
		.filter_map(|index| {
			options
				.item(index)?
				.dyn_into::<web_sys::HtmlOptionElement>()
				.ok()
		})
		.map(|option| option.value())
		.collect::<Vec<_>>();
	match value {
		ControlValue::Text(value) => values.contains(value),
		ControlValue::SelectedValues(selected) => {
			selected.iter().all(|value| values.contains(value))
		}
		ControlValue::Checked(_) => true,
	}
}

fn control_form(element: &Element) -> Option<web_sys::HtmlFormElement> {
	let element = element.as_web_sys();
	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		input.form()
	} else if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		textarea.form()
	} else {
		element
			.dyn_ref::<web_sys::HtmlSelectElement>()
			.and_then(web_sys::HtmlSelectElement::form)
	}
}

struct FormResetListener {
	document: web_sys::Document,
	callback: Closure<dyn FnMut(web_sys::Event)>,
}

impl FormResetListener {
	fn shared() -> Rc<Self> {
		RESET_LISTENER.with(|registered| {
			if let Some(listener) = registered.borrow().upgrade() {
				return listener;
			}
			let document = web_sys::window()
				.and_then(|window| window.document())
				.expect("mounted controls require a document");
			let listener = Rc::new_cyclic(|listener: &Weak<Self>| {
				let listener = listener.clone();
				let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
					let Some(form) = event
						.target()
						.and_then(|target| target.dyn_into::<web_sys::HtmlFormElement>().ok())
					else {
						return;
					};
					let listener = listener.clone();
					// Native event dispatch can flush microtasks before the default action.
					// A task waits for both cancellation and browser reset to finish.
					let after_reset = gloo_timers::future::TimeoutFuture::new(0);
					crate::platform::spawn_task(async move {
						after_reset.await;
						if event.default_prevented() || listener.upgrade().is_none() {
							return;
						}
						let snapshots = MOUNTED_CONTROLS.with(|controls| {
							controls
								.borrow()
								.iter()
								.filter_map(Weak::upgrade)
								.filter(|control| {
									control.active.get()
										&& control_form(&control.element).as_ref() == Some(&form)
								})
								.filter_map(|control| {
									read_control(&control.element, control.binding.kind())
										.map(|value| (control, value))
								})
								.collect::<Vec<_>>()
						});
						batch(|| {
							for (control, value) in snapshots {
								if control.active.get() && value != control.binding.read_untracked()
								{
									control.binding.write(value);
								}
							}
						});
					});
				}) as Box<dyn FnMut(web_sys::Event)>);
				document
					.add_event_listener_with_callback_and_bool(
						"reset",
						callback.as_ref().unchecked_ref(),
						true,
					)
					.expect("should attach the form reset listener");
				Self { document, callback }
			});
			registered.replace(Rc::downgrade(&listener));
			listener
		})
	}
}

impl Drop for FormResetListener {
	fn drop(&mut self) {
		let _ = self.document.remove_event_listener_with_callback_and_bool(
			"reset",
			self.callback.as_ref().unchecked_ref(),
			true,
		);
	}
}

struct SelectOptionObserver {
	observer: web_sys::MutationObserver,
	_callback: Closure<dyn FnMut(js_sys::Array, web_sys::MutationObserver)>,
}

impl SelectOptionObserver {
	fn install(control: &Rc<MountedControl>) -> Option<Self> {
		if !matches!(
			control.binding.kind(),
			ControlKind::SelectOne | ControlKind::SelectMany
		) {
			return None;
		}
		let observed = Rc::downgrade(control);
		let callback = Closure::wrap(Box::new(
			move |_: js_sys::Array, _: web_sys::MutationObserver| {
				if let Some(control) = observed.upgrade() {
					let value = control.binding.read_untracked();
					write_control(&control.element, control.binding.kind(), &value);
				}
			},
		)
			as Box<dyn FnMut(js_sys::Array, web_sys::MutationObserver)>);
		let observer = web_sys::MutationObserver::new(callback.as_ref().unchecked_ref()).ok()?;
		let options = web_sys::MutationObserverInit::new();
		options.set_child_list(true);
		options.set_subtree(true);
		options.set_character_data(true);
		options.set_attributes(true);
		observer
			.observe_with_options(control.element.as_web_sys(), &options)
			.ok()?;
		Some(Self {
			observer,
			_callback: callback,
		})
	}
}

impl Drop for SelectOptionObserver {
	fn drop(&mut self) {
		self.observer.disconnect();
	}
}

fn read_control(element: &Element, kind: ControlKind) -> Option<ControlValue> {
	let element = element.as_web_sys();
	match kind {
		ControlKind::Text => element
			.dyn_ref::<web_sys::HtmlInputElement>()
			.map(web_sys::HtmlInputElement::value)
			.or_else(|| {
				element
					.dyn_ref::<web_sys::HtmlTextAreaElement>()
					.map(web_sys::HtmlTextAreaElement::value)
			})
			.map(ControlValue::Text),
		ControlKind::Checkbox | ControlKind::Radio => element
			.dyn_ref::<web_sys::HtmlInputElement>()
			.map(|input| ControlValue::Checked(input.checked())),
		ControlKind::File => element.dyn_ref::<web_sys::HtmlInputElement>().map(|input| {
			ControlValue::Checked(input.files().is_some_and(|files| files.length() > 0))
		}),
		ControlKind::SelectOne => element
			.dyn_ref::<web_sys::HtmlSelectElement>()
			.map(|select| ControlValue::Text(select.value())),
		ControlKind::SelectMany => element
			.dyn_ref::<web_sys::HtmlSelectElement>()
			.map(|select| {
				let options = select.options();
				ControlValue::SelectedValues(
					(0..options.length())
						.filter_map(|index| {
							options
								.item(index)?
								.dyn_into::<web_sys::HtmlOptionElement>()
								.ok()
						})
						.filter(|option| option.selected())
						.map(|option| option.value())
						.collect(),
				)
			}),
	}
}

fn write_control(element: &Element, kind: ControlKind, value: &ControlValue) {
	let element = element.as_web_sys();
	match (kind, value) {
		(ControlKind::Text, ControlValue::Text(value)) => {
			if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
				if input.type_() != "file" && input.value() != *value {
					input.set_value(value);
				}
			} else if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>()
				&& textarea.value() != *value
			{
				textarea.set_value(value);
			}
		}
		(ControlKind::Checkbox | ControlKind::Radio, ControlValue::Checked(value)) => {
			if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>()
				&& input.checked() != *value
			{
				input.set_checked(*value);
			}
		}
		(ControlKind::File, ControlValue::Checked(false)) => {
			if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>()
				&& !input.value().is_empty()
			{
				input.set_value("");
			}
		}
		(ControlKind::SelectOne, ControlValue::Text(value)) => {
			if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
				let options = select.options();
				let index = (0..options.length())
					.find(|index| {
						options
							.item(*index)
							.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
							.is_some_and(|option| option.value() == *value)
					})
					.map_or(-1, |index| index as i32);
				if select.selected_index() != index {
					select.set_selected_index(index);
				}
			}
		}
		(ControlKind::SelectMany, ControlValue::SelectedValues(values)) => {
			if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
				let options = select.options();
				for index in 0..options.length() {
					if let Some(option) = options
						.item(index)
						.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
					{
						let selected = values.iter().any(|value| *value == option.value());
						if option.selected() != selected {
							option.set_selected(selected);
						}
					}
				}
			}
		}
		_ => {}
	}
}
