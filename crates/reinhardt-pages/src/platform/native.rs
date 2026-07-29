//! Platform implementation for native (non-WASM) targets.
//!
//! Compiled only under `#[cfg(native)]` via the dispatch in
//! `crate::platform`. These stub types maintain API compatibility with
//! WASM code without requiring `web-sys` dependencies on native targets,
//! and task spawning degrades to a no-op since there is no browser event
//! loop.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;

use reinhardt_event_catalog::{EventInterface, KnownEvent};

use super::{InputSnapshot, KeyboardSnapshot, MouseSnapshot, PointerSnapshot};
use crate::event::EventTarget;

pub use reinhardt_core::types::page::NativeEvent as Event;

pub(crate) fn event_type(event: &Event) -> String {
	event.event_type().to_owned()
}

pub(crate) fn event_name_matches(event: &Event, expected: KnownEvent) -> bool {
	event.name().known() == Some(expected)
}

pub(crate) fn event_interface(
	event: &Event,
	primary: EventInterface,
	fallbacks: &'static [EventInterface],
) -> Result<EventInterface, EventInterface> {
	let actual = event.payload().interface();
	if actual == primary || fallbacks.contains(&actual) {
		Ok(actual)
	} else {
		Err(actual)
	}
}

pub(crate) fn target(event: &Event) -> Option<EventTarget> {
	event.target().map(EventTarget::from_native)
}

pub(crate) fn current_target(event: &Event) -> Option<EventTarget> {
	event.current_target().map(EventTarget::from_native)
}

pub(crate) fn bubbles(event: &Event) -> bool {
	event.base().bubbles
}

pub(crate) fn cancelable(event: &Event) -> bool {
	event.base().cancelable
}

pub(crate) fn composed(event: &Event) -> bool {
	event.base().composed
}

pub(crate) fn time_stamp(event: &Event) -> f64 {
	event.base().time_stamp
}

pub(crate) fn is_trusted(event: &Event) -> bool {
	event.base().is_trusted
}

pub(crate) fn input_snapshot(event: &Event) -> InputSnapshot {
	let reinhardt_core::types::page::NativeEventPayload::Input(data) = event.payload() else {
		return InputSnapshot {
			data: None,
			input_type: None,
			is_composing: false,
		};
	};
	InputSnapshot {
		data: data.data.clone(),
		input_type: data.input_type.clone(),
		is_composing: data.is_composing,
	}
}

pub(crate) fn keyboard_snapshot(event: &Event) -> KeyboardSnapshot {
	let reinhardt_core::types::page::NativeEventPayload::Keyboard(data) = event.payload() else {
		unreachable!("validated keyboard payload changed interface")
	};
	KeyboardSnapshot {
		key: data.key.clone(),
		code: data.code.clone(),
		location: data.location,
		repeat: data.repeat,
		is_composing: data.is_composing,
		alt: data.modifiers.alt,
		control: data.modifiers.control,
		meta: data.modifiers.meta,
		shift: data.modifiers.shift,
	}
}

fn mouse_data(data: &reinhardt_core::types::page::MouseEventData) -> MouseSnapshot {
	MouseSnapshot {
		client_x: data.client_x,
		client_y: data.client_y,
		screen_x: data.screen_x,
		screen_y: data.screen_y,
		page_x: data.page_x,
		page_y: data.page_y,
		offset_x: data.offset_x,
		offset_y: data.offset_y,
		button: data.button,
		buttons: data.buttons,
		detail: data.detail,
		alt: data.modifiers.alt,
		control: data.modifiers.control,
		meta: data.modifiers.meta,
		shift: data.modifiers.shift,
	}
}

pub(crate) fn mouse_snapshot(event: &Event) -> MouseSnapshot {
	use reinhardt_core::types::page::NativeEventPayload;
	match event.payload() {
		NativeEventPayload::Mouse(data) => mouse_data(data),
		NativeEventPayload::Pointer(data) => mouse_data(&data.mouse),
		NativeEventPayload::Drag(data) => mouse_data(&data.mouse),
		NativeEventPayload::Wheel(data) => mouse_data(&data.mouse),
		_ => unreachable!("validated mouse-compatible payload changed interface"),
	}
}

pub(crate) fn pointer_snapshot(event: &Event) -> PointerSnapshot {
	if let reinhardt_core::types::page::NativeEventPayload::Pointer(data) = event.payload() {
		PointerSnapshot {
			pointer_id: data.pointer_id,
			pointer_kind: data.pointer_kind.clone(),
			pressure: data.pressure,
			width: data.width,
			height: data.height,
			tangential_pressure: data.tangential_pressure,
			tilt_x: data.tilt_x,
			tilt_y: data.tilt_y,
			twist: data.twist,
			is_primary: data.is_primary,
		}
	} else {
		PointerSnapshot {
			pointer_id: 0,
			pointer_kind: "mouse".to_owned(),
			pressure: 0.0,
			width: 0.0,
			height: 0.0,
			tangential_pressure: 0.0,
			tilt_x: 0,
			tilt_y: 0,
			twist: 0,
			is_primary: true,
		}
	}
}

type BoxedTask = Pin<Box<dyn Future<Output = ()> + 'static>>;
type TaskSink = Box<dyn Fn(BoxedTask) + 'static>;

thread_local! {
	static TASK_SINK: RefCell<Option<TaskSink>> = const { RefCell::new(None) };
}

#[cfg(any(feature = "testing", test))]
pub(crate) struct NativeTaskSinkGuard {
	previous: Option<TaskSink>,
}

#[cfg(any(feature = "testing", test))]
impl Drop for NativeTaskSinkGuard {
	fn drop(&mut self) {
		let previous = self.previous.take();
		TASK_SINK.with(|slot| {
			*slot.borrow_mut() = previous;
		});
	}
}

#[cfg(any(feature = "testing", test))]
pub(crate) fn install_task_sink(sink: impl Fn(BoxedTask) + 'static) -> NativeTaskSinkGuard {
	let previous = TASK_SINK.with(|slot| slot.borrow_mut().replace(Box::new(sink)));
	NativeTaskSinkGuard { previous }
}

pub(crate) fn try_spawn_task<F>(fut: F) -> bool
where
	F: Future<Output = ()> + 'static,
{
	TASK_SINK.with(|slot| {
		if let Some(sink) = slot.borrow().as_ref() {
			sink(Box::pin(fut));
			true
		} else {
			false
		}
	})
}

#[cfg(test)]
pub(crate) fn has_task_sink() -> bool {
	TASK_SINK.with(|slot| slot.borrow().is_some())
}

/// Stub Window type for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct Window;

/// Stub HtmlInputElement for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct HtmlInputElement {
	value: String,
}

impl HtmlInputElement {
	/// Get the input value.
	pub fn value(&self) -> String {
		self.value.clone()
	}

	/// Set the input value.
	pub fn set_value(&mut self, value: &str) {
		self.value = value.to_string();
	}
}

/// Stub HtmlTextAreaElement for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct HtmlTextAreaElement {
	value: String,
}

impl HtmlTextAreaElement {
	/// Get the textarea value.
	pub fn value(&self) -> String {
		self.value.clone()
	}

	/// Set the textarea value.
	pub fn set_value(&mut self, value: &str) {
		self.value = value.to_string();
	}
}

/// Stub HtmlSelectElement for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct HtmlSelectElement {
	value: String,
}

impl HtmlSelectElement {
	/// Get the selected value.
	pub fn value(&self) -> String {
		self.value.clone()
	}

	/// Set the selected value.
	pub fn set_value(&mut self, value: &str) {
		self.value = value.to_string();
	}
}

/// Stub HtmlFormElement for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct HtmlFormElement;

/// Stub HtmlButtonElement for SSR compatibility.
#[derive(Debug, Clone, Default)]
pub struct HtmlButtonElement;

/// No-op task spawn for native targets.
///
/// On native (SSR) targets there is no browser event loop, so the future
/// is dropped. Keeping `spawn_task` cross-target lets `form!`-generated
/// submission handlers and reactive hooks compile identically on both
/// targets.
pub fn spawn_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	let _ = try_spawn_task(fut);
}

/// No-op microtask yield for native targets.
pub async fn defer_yield() {}
