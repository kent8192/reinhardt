//! Platform implementation for browser-WASM targets.
//!
//! Compiled only under `#[cfg(wasm)]` via the dispatch in
//! `crate::platform`. The items here back the cross-target API
//! re-exported from `crate::platform` and `crate::prelude`, mapping each
//! abstract type onto its `web_sys` counterpart and implementing task
//! spawning on top of `wasm_bindgen_futures`.

use std::future::Future;

use reinhardt_event_catalog::{EventInterface, KnownEvent};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::wasm_bindgen;

use super::{InputSnapshot, KeyboardSnapshot, MouseSnapshot, PointerSnapshot};
use crate::event::EventTarget;

/// DOM Event type (`web_sys::Event` on WASM).
pub type Event = web_sys::Event;

#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(extends = web_sys::Event, js_name = PictureInPictureEvent)]
	#[derive(Clone, Debug, PartialEq, Eq)]
	type PictureInPictureEventIdentity;

	#[wasm_bindgen(extends = web_sys::Event, js_name = XRInputSourceEvent)]
	#[derive(Clone, Debug, PartialEq, Eq)]
	type XrInputSourceEventIdentity;
}

pub(crate) fn event_type(event: &Event) -> String {
	event.type_()
}

pub(crate) fn event_name_matches(event: &Event, expected: KnownEvent) -> bool {
	event.type_() == expected.as_str()
}

fn strict_instance_of<T>(event: &Event, global_name: &str) -> bool
where
	T: JsCast,
{
	let constructor = js_sys::Reflect::get(
		&js_sys::global(),
		&wasm_bindgen::JsValue::from_str(global_name),
	);
	constructor.is_ok_and(|constructor| constructor.is_function()) && event.is_instance_of::<T>()
}

fn matches_interface(event: &Event, interface: EventInterface) -> bool {
	match interface {
		EventInterface::Generic => actual_event_interface(event) == EventInterface::Generic,
		EventInterface::Animation => event.is_instance_of::<web_sys::AnimationEvent>(),
		EventInterface::Clipboard => event.is_instance_of::<web_sys::ClipboardEvent>(),
		EventInterface::Command => event.is_instance_of::<web_sys::CommandEvent>(),
		EventInterface::Composition => event.is_instance_of::<web_sys::CompositionEvent>(),
		EventInterface::Drag => event.is_instance_of::<web_sys::DragEvent>(),
		EventInterface::Focus => event.is_instance_of::<web_sys::FocusEvent>(),
		EventInterface::Input => event.is_instance_of::<web_sys::InputEvent>(),
		EventInterface::Keyboard => event.is_instance_of::<web_sys::KeyboardEvent>(),
		EventInterface::MediaEncrypted => event.is_instance_of::<web_sys::MediaEncryptedEvent>(),
		EventInterface::Mouse => event.is_instance_of::<web_sys::MouseEvent>(),
		EventInterface::PictureInPicture => {
			strict_instance_of::<PictureInPictureEventIdentity>(event, "PictureInPictureEvent")
		}
		EventInterface::Pointer => event.is_instance_of::<web_sys::PointerEvent>(),
		EventInterface::SecurityPolicyViolation => {
			event.is_instance_of::<web_sys::SecurityPolicyViolationEvent>()
		}
		EventInterface::Submit => event.is_instance_of::<web_sys::SubmitEvent>(),
		EventInterface::Time => event.is_instance_of::<web_sys::TimeEvent>(),
		EventInterface::Toggle => event.is_instance_of::<web_sys::ToggleEvent>(),
		EventInterface::Touch => event.is_instance_of::<web_sys::TouchEvent>(),
		EventInterface::Transition => event.is_instance_of::<web_sys::TransitionEvent>(),
		EventInterface::Wheel => event.is_instance_of::<web_sys::WheelEvent>(),
		EventInterface::XrInputSource => {
			strict_instance_of::<XrInputSourceEventIdentity>(event, "XRInputSourceEvent")
		}
		_ => false,
	}
}

fn actual_event_interface(event: &Event) -> EventInterface {
	[
		EventInterface::Pointer,
		EventInterface::Wheel,
		EventInterface::Drag,
		EventInterface::Mouse,
		EventInterface::Animation,
		EventInterface::Clipboard,
		EventInterface::Command,
		EventInterface::Composition,
		EventInterface::Focus,
		EventInterface::Input,
		EventInterface::Keyboard,
		EventInterface::MediaEncrypted,
		EventInterface::PictureInPicture,
		EventInterface::SecurityPolicyViolation,
		EventInterface::Submit,
		EventInterface::Time,
		EventInterface::Toggle,
		EventInterface::Touch,
		EventInterface::Transition,
		EventInterface::XrInputSource,
	]
	.into_iter()
	.find(|interface| matches_interface(event, *interface))
	.unwrap_or(EventInterface::Generic)
}

pub(crate) fn event_interface(
	event: &Event,
	primary: EventInterface,
	fallbacks: &'static [EventInterface],
) -> Result<EventInterface, EventInterface> {
	if matches_interface(event, primary) {
		return Ok(primary);
	}
	if let Some(interface) = fallbacks
		.iter()
		.copied()
		.find(|interface| matches_interface(event, *interface))
	{
		return Ok(interface);
	}
	Err(actual_event_interface(event))
}

pub(crate) fn target(event: &Event) -> Option<EventTarget> {
	event.target().and_then(EventTarget::from_web_target)
}

pub(crate) fn current_target(event: &Event) -> Option<EventTarget> {
	event
		.current_target()
		.and_then(EventTarget::from_web_target)
}

pub(crate) fn bubbles(event: &Event) -> bool {
	event.bubbles()
}

pub(crate) fn cancelable(event: &Event) -> bool {
	event.cancelable()
}

pub(crate) fn composed(event: &Event) -> bool {
	event.composed()
}

pub(crate) fn time_stamp(event: &Event) -> f64 {
	event.time_stamp()
}

pub(crate) fn is_trusted(event: &Event) -> bool {
	event.is_trusted()
}

pub(crate) fn input_snapshot(event: &Event) -> InputSnapshot {
	event.dyn_ref::<web_sys::InputEvent>().map_or(
		InputSnapshot {
			data: None,
			input_type: None,
			is_composing: false,
		},
		|event| InputSnapshot {
			data: event.data(),
			input_type: Some(event.input_type()),
			is_composing: event.is_composing(),
		},
	)
}

pub(crate) fn keyboard_snapshot(event: &Event) -> KeyboardSnapshot {
	let event = event
		.dyn_ref::<web_sys::KeyboardEvent>()
		.expect("validated keyboard event changed interface");
	KeyboardSnapshot {
		key: event.key(),
		code: event.code(),
		location: event.location(),
		repeat: event.repeat(),
		is_composing: event.is_composing(),
		alt: event.alt_key(),
		control: event.ctrl_key(),
		meta: event.meta_key(),
		shift: event.shift_key(),
	}
}

fn mouse_data(event: &web_sys::MouseEvent) -> MouseSnapshot {
	MouseSnapshot {
		client_x: f64::from(event.client_x()),
		client_y: f64::from(event.client_y()),
		screen_x: f64::from(event.screen_x()),
		screen_y: f64::from(event.screen_y()),
		page_x: f64::from(event.page_x()),
		page_y: f64::from(event.page_y()),
		offset_x: f64::from(event.offset_x()),
		offset_y: f64::from(event.offset_y()),
		button: event.button(),
		buttons: event.buttons(),
		detail: event.detail(),
		alt: event.alt_key(),
		control: event.ctrl_key(),
		meta: event.meta_key(),
		shift: event.shift_key(),
	}
}

pub(crate) fn mouse_snapshot(event: &Event) -> MouseSnapshot {
	let event = event
		.dyn_ref::<web_sys::MouseEvent>()
		.expect("validated mouse event changed interface");
	mouse_data(event)
}

pub(crate) fn pointer_snapshot(event: &Event) -> PointerSnapshot {
	if let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>() {
		PointerSnapshot {
			pointer_id: pointer.pointer_id(),
			pointer_kind: pointer.pointer_type(),
			pressure: pointer.pressure(),
			width: f64::from(pointer.width()),
			height: f64::from(pointer.height()),
			tangential_pressure: pointer.tangential_pressure(),
			tilt_x: pointer.tilt_x(),
			tilt_y: pointer.tilt_y(),
			twist: pointer.twist(),
			is_primary: pointer.is_primary(),
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

/// Window object type (`web_sys::Window` on WASM).
pub type Window = web_sys::Window;

/// HTML input element type (`web_sys::HtmlInputElement` on WASM).
pub type HtmlInputElement = web_sys::HtmlInputElement;

/// HTML textarea element type (`web_sys::HtmlTextAreaElement` on WASM).
pub type HtmlTextAreaElement = web_sys::HtmlTextAreaElement;

/// HTML select element type (`web_sys::HtmlSelectElement` on WASM).
pub type HtmlSelectElement = web_sys::HtmlSelectElement;

/// HTML form element type (`web_sys::HtmlFormElement` on WASM).
pub type HtmlFormElement = web_sys::HtmlFormElement;

/// HTML button element type (`web_sys::HtmlButtonElement` on WASM).
pub type HtmlButtonElement = web_sys::HtmlButtonElement;

/// Spawns a task on the current runtime.
///
/// On WASM, this uses `wasm_bindgen_futures::spawn_local` to schedule
/// the task on the browser's event loop.
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::prelude::spawn_task;
///
/// spawn_task(async move {
///     // async work
/// });
/// ```
pub fn spawn_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	if let Some(client) = crate::reactive::query::current_query_client() {
		wasm_bindgen_futures::spawn_local(crate::reactive::query::with_query_client_async(
			client, fut,
		));
	} else {
		wasm_bindgen_futures::spawn_local(fut);
	}
}

/// Yields to the event loop by queuing a microtask.
///
/// On WASM, this resolves a `JsFuture` wrapping a `Promise.resolve()`,
/// giving the browser event loop a chance to tick. This is necessary when
/// spawning async work during initialization (e.g. inside `main()`),
/// where `JsFuture`s from fetch would otherwise hang.
pub async fn defer_yield() {
	use wasm_bindgen::JsValue;
	let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
	let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(test)]
mod interface_tests {
	use reinhardt_event_catalog::EventInterface;
	use wasm_bindgen::{JsCast, JsValue};
	use wasm_bindgen_test::*;

	use super::{
		PictureInPictureEventIdentity, XrInputSourceEventIdentity, event_interface,
		matches_interface, strict_instance_of,
	};

	wasm_bindgen_test_configure!(run_in_browser);

	fn set_constructor(event: &web_sys::Event, constructor: &JsValue) {
		js_sys::Reflect::set(
			event.as_ref(),
			&JsValue::from_str("constructor"),
			constructor,
		)
		.expect("synthetic event constructor must be configurable");
	}

	fn named_constructor(name: &str) -> JsValue {
		let constructor = js_sys::Object::new();
		js_sys::Reflect::set(
			constructor.as_ref(),
			&JsValue::from_str("name"),
			&JsValue::from_str(name),
		)
		.expect("constructor name must be configurable");
		constructor.into()
	}

	#[wasm_bindgen_test]
	fn spoofed_constructor_names_do_not_match_unstable_interfaces() {
		let pip = web_sys::Event::new("enterpictureinpicture").expect("event construction");
		set_constructor(&pip, &named_constructor("PictureInPictureEvent"));
		let xr = web_sys::Event::new("beforexrselect").expect("event construction");
		set_constructor(&xr, &named_constructor("XRInputSourceEvent"));

		assert!(!matches_interface(&pip, EventInterface::PictureInPicture));
		assert!(!matches_interface(&xr, EventInterface::XrInputSource));
	}

	#[wasm_bindgen_test]
	fn missing_and_nonmatching_constructors_are_rejected() {
		let missing = web_sys::Event::new("enterpictureinpicture").expect("event construction");
		set_constructor(&missing, &JsValue::UNDEFINED);
		let nonmatching = web_sys::Event::new("beforexrselect").expect("event construction");
		set_constructor(&nonmatching, &named_constructor("KeyboardEvent"));

		assert!(!matches_interface(
			&missing,
			EventInterface::PictureInPicture
		));
		assert!(!matches_interface(
			&nonmatching,
			EventInterface::XrInputSource
		));
		assert!(!strict_instance_of::<PictureInPictureEventIdentity>(
			&missing,
			"__ReinhardtMissingEventConstructor"
		));
		assert!(!strict_instance_of::<XrInputSourceEventIdentity>(
			&nonmatching,
			"document"
		));
	}

	#[wasm_bindgen_test]
	fn interface_rejection_reports_the_specific_actual_family() {
		let keyboard = web_sys::KeyboardEvent::new("click").expect("keyboard event construction");
		let raw: web_sys::Event = keyboard.unchecked_into();

		let actual = event_interface(&raw, EventInterface::Pointer, &[EventInterface::Mouse])
			.expect_err("keyboard event must not satisfy click interfaces");

		assert_eq!(actual, EventInterface::Keyboard);
	}
}
