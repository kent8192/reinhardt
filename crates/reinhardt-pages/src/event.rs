//! Typed payloads for standardized page events.

mod custom;
mod payload;
mod target;
mod value;

use std::fmt;

pub use custom::{CustomEvent, CustomEventDetailError};
pub use payload::*;
pub use reinhardt_event_catalog::{EventInterface, EventName, KnownEvent};
pub use target::{EventTarget, EventTargetError};
pub use value::{EventFile, Modifiers, MouseButton, MouseButtons, Point, PointerKind};

pub use crate::callback::{
	IntoTypedEventHandler, raw_async_event_handler, raw_event_handler,
	typed_async_custom_event_handler, typed_async_event_handler, typed_custom_event_handler,
	typed_event_handler,
};

use crate::platform;

/// Converts a raw platform event into one exact standardized payload.
pub trait EventPayload: Sized + 'static {
	/// Standard event represented by this payload type.
	const EVENT: KnownEvent;

	/// Validates the exact event name and interface before wrapping the raw event.
	fn try_from_raw(event: platform::Event) -> Result<Self, EventConversionError>;
}

/// Failure to convert a raw event into an exact typed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventConversionError {
	/// The raw event name differs from the payload's catalog name.
	UnexpectedName {
		/// Expected exact DOM event name.
		expected: &'static str,
		/// Actual raw event name.
		actual: String,
	},
	/// The raw event does not implement a permitted browser interface family.
	UnexpectedInterface {
		/// Exact event name.
		event: &'static str,
		/// Preferred interface family.
		primary: EventInterface,
		/// Accepted compatibility interface families.
		fallbacks: &'static [EventInterface],
		/// Interface family supplied by the raw transport.
		actual: EventInterface,
	},
}

fn interface_name(interface: EventInterface) -> &'static str {
	match interface {
		EventInterface::Generic => "Generic",
		EventInterface::Animation => "Animation",
		EventInterface::Clipboard => "Clipboard",
		EventInterface::Command => "Command",
		EventInterface::Composition => "Composition",
		EventInterface::Drag => "Drag",
		EventInterface::Focus => "Focus",
		EventInterface::Input => "Input",
		EventInterface::Keyboard => "Keyboard",
		EventInterface::MediaEncrypted => "MediaEncrypted",
		EventInterface::Mouse => "Mouse",
		EventInterface::PictureInPicture => "PictureInPicture",
		EventInterface::Pointer => "Pointer",
		EventInterface::SecurityPolicyViolation => "SecurityPolicyViolation",
		EventInterface::Submit => "Submit",
		EventInterface::Time => "Time",
		EventInterface::Toggle => "Toggle",
		EventInterface::Touch => "Touch",
		EventInterface::Transition => "Transition",
		EventInterface::Wheel => "Wheel",
		EventInterface::XrInputSource => "XrInputSource",
		_ => "Unknown",
	}
}

impl fmt::Display for EventConversionError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::UnexpectedName { expected, actual } => write!(
				formatter,
				"cannot convert `{actual}` into `{expected}` event payload"
			),
			Self::UnexpectedInterface {
				event,
				primary,
				fallbacks,
				actual,
			} => {
				write!(formatter, "`{event}` requires {}", interface_name(*primary))?;
				for fallback in *fallbacks {
					write!(formatter, " or {}", interface_name(*fallback))?;
				}
				write!(formatter, " event data, got {}", interface_name(*actual))
			}
		}
	}
}

impl std::error::Error for EventConversionError {}

#[cfg(all(test, native))]
mod tests {
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use reinhardt_core::types::page::{
		BaseEventData, InputEventData, KeyboardEventData, ModifierState, MouseEventData,
		NativeEvent, NativeEventFile, NativeEventPayload, NativeEventTarget, PointerEventData,
	};

	use super::{
		ChangeEvent, ClickEvent, EventPayload, EventTargetError, InputEvent, KeyDownEvent,
		MouseButton, MouseButtons, PointerKind, PointerMoveEvent, typed_event_handler,
	};
	use crate::Callback;

	fn raw_event(
		event: reinhardt_core::types::page::EventType,
		payload: NativeEventPayload,
	) -> NativeEvent {
		NativeEvent::for_known(event, payload)
	}

	#[test]
	fn event_payload_accepts_its_exact_catalog_name() {
		let raw = NativeEvent::for_known(
			reinhardt_core::types::page::EventType::Click,
			NativeEventPayload::Pointer(PointerEventData::default()),
		);

		let payload = ClickEvent::try_from_raw(raw).expect("click payload must convert");

		assert_eq!(payload.event_type(), "click");
	}

	#[test]
	fn event_payload_rejects_a_different_catalog_name() {
		let raw = raw_event(
			reinhardt_core::types::page::EventType::KeyDown,
			NativeEventPayload::Keyboard(KeyboardEventData::default()),
		);

		let error = ClickEvent::try_from_raw(raw).expect_err("keydown must not become click");

		assert_eq!(
			error.to_string(),
			"cannot convert `keydown` into `click` event payload"
		);
	}

	#[test]
	fn event_payload_rejects_an_incompatible_interface() {
		let raw = raw_event(
			reinhardt_core::types::page::EventType::Click,
			NativeEventPayload::Keyboard(KeyboardEventData::default()),
		);

		let error = ClickEvent::try_from_raw(raw).expect_err("keyboard data must be rejected");

		assert_eq!(
			error.to_string(),
			"`click` requires Pointer or Mouse event data, got Keyboard"
		);
	}

	#[test]
	fn event_payload_accepts_a_catalog_fallback_interface() {
		let raw = raw_event(
			reinhardt_core::types::page::EventType::Click,
			NativeEventPayload::Mouse(MouseEventData::default()),
		);

		let payload = ClickEvent::try_from_raw(raw).expect("mouse fallback must convert");

		assert_eq!(payload.event_type(), "click");
		assert_eq!(payload.pointer_type(), PointerKind::Mouse);
	}

	macro_rules! assert_catalog_payloads_are_public {
		(
			$(
				$kind:ident,
				$dom_name:literal,
				$payload:ident,
				$interface:ident,
				[$($fallback:ident),* $(,)?],
				[$($capability:ident),* $(,)?],
				$bubbles:literal,
				$cancelable:literal,
				$composed:literal,
				$fixture_defaults:ident;
			)*
		) => {
			// Catalog coverage intentionally includes deprecated compatibility payloads.
			#[allow(deprecated)]
			#[test]
			fn event_catalog_generates_every_distinct_payload_type() {
				fn assert_payload<P: EventPayload>() {}
				let mut generated = 0_usize;
				$(
					assert_payload::<super::$payload>();
					generated += 1;
				)*
				assert_eq!(generated, 115);
			}
		};
	}

	reinhardt_event_catalog::__reinhardt_event_catalog!(assert_catalog_payloads_are_public);

	#[test]
	fn event_payload_exposes_base_state_and_distinct_targets() {
		let target = NativeEventTarget::new("span").with_text_content("Save");
		let current_target = NativeEventTarget::new("button").with_attribute("type", "submit");
		let raw = NativeEvent::new(
			reinhardt_core::types::page::EventName::Known(
				reinhardt_core::types::page::EventType::Click,
			),
			BaseEventData {
				bubbles: true,
				cancelable: true,
				composed: true,
				time_stamp: 12.5,
				is_trusted: false,
			},
			NativeEventPayload::Pointer(PointerEventData::default()),
		)
		.with_target(target)
		.with_current_target(current_target);
		let payload = ClickEvent::try_from_raw(raw).expect("click payload must convert");

		assert_eq!(payload.target().expect("origin target").tag_name(), "span");
		assert_eq!(
			payload
				.current_target()
				.expect("listener target")
				.tag_name(),
			"button"
		);
		assert!(payload.bubbles());
		assert!(payload.cancelable());
		assert!(payload.composed());
		assert_eq!(payload.time_stamp(), 12.5);
		assert!(!payload.is_trusted());
		assert!(!payload.default_prevented());

		payload.prevent_default();
		payload.stop_immediate_propagation();

		assert!(payload.default_prevented());
		assert!(payload.raw().propagation_stopped());
		assert!(payload.raw().immediate_propagation_stopped());
	}

	#[test]
	fn target_error_display_includes_structured_context() {
		let missing = EventTargetError::MissingCurrentTarget { event: "input" };
		let unsupported = EventTargetError::UnsupportedProperty {
			event: "change",
			property: "checked",
			actual_tag: "input".to_owned(),
		};

		assert_eq!(missing.to_string(), "`input` event has no current target");
		assert_eq!(
			unsupported.to_string(),
			"`change` event target `input` does not expose `checked`"
		);
	}

	#[test]
	fn keyboard_and_pointer_payloads_use_deterministic_defaults() {
		let keyboard = KeyDownEvent::try_from_raw(raw_event(
			reinhardt_core::types::page::EventType::KeyDown,
			NativeEventPayload::Keyboard(KeyboardEventData::default()),
		))
		.expect("keyboard payload must convert");
		let pointer = PointerMoveEvent::try_from_raw(raw_event(
			reinhardt_core::types::page::EventType::PointerMove,
			NativeEventPayload::Pointer(PointerEventData::default()),
		))
		.expect("pointer payload must convert");

		assert_eq!(keyboard.key(), "");
		assert_eq!(keyboard.code(), "");
		assert!(!keyboard.repeat());
		assert_eq!(keyboard.modifiers(), super::Modifiers::default());
		assert_eq!(pointer.client_position(), super::Point::default());
		assert_eq!(pointer.button(), MouseButton::Primary);
		assert_eq!(pointer.buttons(), MouseButtons::NONE);
		assert_eq!(pointer.pointer_id(), 0);
		assert_eq!(pointer.pointer_type(), PointerKind::Mouse);
		assert_eq!(pointer.pressure(), 0.0);
		assert_eq!(pointer.modifiers(), super::Modifiers::default());
	}

	#[test]
	fn payload_capabilities_read_current_target_state() {
		let input = NativeEventTarget::new("input")
			.with_attribute("type", "text")
			.with_value("Ada");
		let input_payload = InputEvent::try_from_raw(
			raw_event(
				reinhardt_core::types::page::EventType::Input,
				NativeEventPayload::Input(InputEventData {
					data: Some("a".to_owned()),
					input_type: Some("insertText".to_owned()),
					is_composing: false,
				}),
			)
			.with_current_target(input),
		)
		.expect("input payload must convert");
		let checkbox_payload = ChangeEvent::try_from_raw(
			raw_event(
				reinhardt_core::types::page::EventType::Change,
				NativeEventPayload::default(),
			)
			.with_current_target(
				NativeEventTarget::new("input")
					.with_attribute("type", "checkbox")
					.with_checked(true),
			),
		)
		.expect("change payload must convert");
		let select_payload = ChangeEvent::try_from_raw(
			raw_event(
				reinhardt_core::types::page::EventType::Change,
				NativeEventPayload::default(),
			)
			.with_current_target(
				NativeEventTarget::new("select").with_selected_values(["red", "blue"]),
			),
		)
		.expect("change payload must convert");
		let file_payload = ChangeEvent::try_from_raw(
			raw_event(
				reinhardt_core::types::page::EventType::Change,
				NativeEventPayload::default(),
			)
			.with_current_target(
				NativeEventTarget::new("input")
					.with_attribute("type", "file")
					.with_file(NativeEventFile::new("avatar.png", "image/png", 128, 42)),
			),
		)
		.expect("change payload must convert");

		assert_eq!(input_payload.value(), Ok("Ada".to_owned()));
		assert_eq!(input_payload.data(), Some("a".to_owned()));
		assert_eq!(input_payload.input_type(), Some("insertText".to_owned()));
		assert_eq!(checkbox_payload.checked(), Ok(true));
		assert_eq!(
			select_payload.selected_values(),
			Ok(vec!["red".to_owned(), "blue".to_owned()])
		);
		let files = file_payload.files().expect("file input must expose files");
		assert_eq!(files.len(), 1);
		assert_eq!(files[0].name(), "avatar.png");
		assert_eq!(files[0].media_type(), "image/png");
		assert_eq!(files[0].size(), 128);
		assert_eq!(files[0].last_modified(), 42);
	}

	#[test]
	fn typed_handler_accepts_callback_payload() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let calls = Arc::new(AtomicUsize::new(0));
			let callback = Callback::<ClickEvent, ()>::new({
				let calls = Arc::clone(&calls);
				move |payload| {
					assert_eq!(payload.event_type(), "click");
					calls.fetch_add(1, Ordering::SeqCst);
				}
			});
			let handler = typed_event_handler::<ClickEvent, _>(callback);

			handler(raw_event(
				reinhardt_core::types::page::EventType::Click,
				NativeEventPayload::Pointer(PointerEventData {
					mouse: MouseEventData {
						modifiers: ModifierState::default(),
						..MouseEventData::default()
					},
					..PointerEventData::default()
				}),
			));

			assert_eq!(calls.load(Ordering::SeqCst), 1);
		});
	}

	#[test]
	fn typed_handler_does_not_invoke_for_a_misleading_interface() {
		let calls = Arc::new(AtomicUsize::new(0));
		let handler = typed_event_handler::<ClickEvent, _>({
			let calls = Arc::clone(&calls);
			move |_payload| {
				calls.fetch_add(1, Ordering::SeqCst);
			}
		});

		handler(raw_event(
			reinhardt_core::types::page::EventType::Click,
			NativeEventPayload::Keyboard(KeyboardEventData::default()),
		));

		assert_eq!(calls.load(Ordering::SeqCst), 0);
	}
}
