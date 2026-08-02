use std::cell::OnceCell;
use std::fmt;

use serde::de::DeserializeOwned;

use super::EventTarget;
use crate::platform;

/// Typed wrapper for a browser `CustomEvent` detail payload.
///
/// This is a P2 API. Native events decode the stored JSON detail, while WASM
/// events deserialize the browser `CustomEvent.detail` value.
///
/// Detail decoding is deferred until [`Self::detail`] or [`Self::into_detail`]
/// is called and its result, including a decoding failure, is cached.
pub struct CustomEvent<T> {
	raw: platform::Event,
	event_type: String,
	target: Option<EventTarget>,
	current_target: Option<EventTarget>,
	decoded: OnceCell<Result<T, CustomEventDetailError>>,
}

impl<T> CustomEvent<T> {
	pub(crate) fn from_raw(raw: platform::Event) -> Self {
		let event_type = platform::event_type(&raw);
		let target = platform::target(&raw);
		let current_target = platform::current_target(&raw);
		Self {
			raw,
			event_type,
			target,
			current_target,
			decoded: OnceCell::new(),
		}
	}

	/// Returns the unmodified cross-target raw event.
	#[must_use]
	pub const fn raw(&self) -> &platform::Event {
		&self.raw
	}

	/// Returns the dispatched event name.
	#[must_use]
	pub fn event_type(&self) -> &str {
		&self.event_type
	}

	/// Prevents the default action when the event is cancelable.
	pub fn prevent_default(&self) {
		self.raw.prevent_default();
	}

	/// Stops dispatch before the next ancestor listener.
	pub fn stop_propagation(&self) {
		self.raw.stop_propagation();
	}

	/// Stops later listeners on this target and ancestor traversal.
	pub fn stop_immediate_propagation(&self) {
		self.raw.stop_immediate_propagation();
	}

	/// Returns whether the default action has been prevented.
	#[must_use]
	pub fn default_prevented(&self) -> bool {
		self.raw.default_prevented()
	}

	/// Returns the originating event target snapshot.
	#[must_use]
	pub fn target(&self) -> Option<EventTarget> {
		self.target.clone()
	}

	/// Returns the listener target snapshot captured during conversion.
	#[must_use]
	pub fn current_target(&self) -> Option<EventTarget> {
		self.current_target.clone()
	}

	/// Returns whether the event bubbles.
	#[must_use]
	pub fn bubbles(&self) -> bool {
		platform::bubbles(&self.raw)
	}

	/// Returns whether the event can be canceled.
	#[must_use]
	pub fn cancelable(&self) -> bool {
		platform::cancelable(&self.raw)
	}

	/// Returns whether the event crosses shadow boundaries.
	#[must_use]
	pub fn composed(&self) -> bool {
		platform::composed(&self.raw)
	}

	/// Returns the event timestamp in milliseconds.
	#[must_use]
	pub fn time_stamp(&self) -> f64 {
		platform::time_stamp(&self.raw)
	}

	/// Returns whether the user agent created the event.
	#[must_use]
	pub fn is_trusted(&self) -> bool {
		platform::is_trusted(&self.raw)
	}
}

impl<T> CustomEvent<T>
where
	T: DeserializeOwned,
{
	/// Returns the cached decoded custom-event detail.
	pub fn detail(&self) -> Result<&T, &CustomEventDetailError> {
		self.decoded.get_or_init(|| self.decode_detail()).as_ref()
	}

	/// Consumes the event and returns its decoded detail without cloning it.
	pub fn into_detail(mut self) -> Result<T, CustomEventDetailError> {
		self.decoded.take().unwrap_or_else(|| self.decode_detail())
	}

	#[cfg(native)]
	fn decode_detail(&self) -> Result<T, CustomEventDetailError> {
		let Some(detail) = self.raw.custom_detail() else {
			return Err(CustomEventDetailError::NotCustomEvent {
				event_type: self.event_type.clone(),
			});
		};
		serde_json::from_value(detail.clone()).map_err(|error| {
			CustomEventDetailError::Deserialize {
				event_type: self.event_type.clone(),
				target_type: std::any::type_name::<T>(),
				message: error.to_string(),
			}
		})
	}

	#[cfg(wasm)]
	fn decode_detail(&self) -> Result<T, CustomEventDetailError> {
		use wasm_bindgen::JsCast;

		let Some(event) = self.raw.dyn_ref::<web_sys::CustomEvent>() else {
			return Err(CustomEventDetailError::NotCustomEvent {
				event_type: self.event_type.clone(),
			});
		};
		serde_wasm_bindgen::from_value(event.detail()).map_err(|error| {
			CustomEventDetailError::Deserialize {
				event_type: self.event_type.clone(),
				target_type: std::any::type_name::<T>(),
				message: error.to_string(),
			}
		})
	}
}

/// Failure to decode a typed custom-event detail payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CustomEventDetailError {
	/// The raw event does not implement the browser `CustomEvent` interface.
	NotCustomEvent {
		/// Dispatched event name.
		event_type: String,
	},
	/// The custom-event detail could not be decoded into the requested type.
	Deserialize {
		/// Dispatched event name.
		event_type: String,
		/// Fully qualified Rust type requested by the caller.
		target_type: &'static str,
		/// Decoder-provided diagnostic message.
		message: String,
	},
}

impl fmt::Display for CustomEventDetailError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NotCustomEvent { event_type } => {
				write!(formatter, "`{event_type}` is not a CustomEvent")
			}
			Self::Deserialize {
				event_type,
				target_type,
				message,
			} => write!(
				formatter,
				"failed to deserialize `{event_type}` detail as `{target_type}`: {message}"
			),
		}
	}
}

impl std::error::Error for CustomEventDetailError {}

#[cfg(all(test, native))]
mod tests {
	use std::borrow::Cow;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use reinhardt_core::types::page::{
		BaseEventData, NativeEvent, NativeEventPayload, NativeEventTarget,
	};
	use reinhardt_event_catalog::EventName;
	use serde::Deserialize;
	use serial_test::serial;

	use super::{CustomEvent, CustomEventDetailError};

	#[derive(Debug, PartialEq, Eq, Deserialize)]
	struct SelectedDetail {
		id: u64,
	}

	static SUCCESSFUL_DECODE_COUNT: AtomicUsize = AtomicUsize::new(0);
	static FAILING_DECODE_COUNT: AtomicUsize = AtomicUsize::new(0);

	struct DecodesOnce;

	impl<'de> Deserialize<'de> for DecodesOnce {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			SUCCESSFUL_DECODE_COUNT.fetch_add(1, Ordering::SeqCst);
			let _ = serde_json::Value::deserialize(deserializer)?;
			Ok(Self)
		}
	}

	struct AlwaysFails;

	impl<'de> Deserialize<'de> for AlwaysFails {
		fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			FAILING_DECODE_COUNT.fetch_add(1, Ordering::SeqCst);
			Err(serde::de::Error::custom("expected decoding failure"))
		}
	}

	fn custom_native_event(detail: serde_json::Value) -> NativeEvent {
		NativeEvent::new(
			EventName::Custom(Cow::Borrowed("item-selected")),
			BaseEventData {
				cancelable: true,
				..BaseEventData::default()
			},
			NativeEventPayload::default(),
		)
		.with_custom_detail(detail)
	}

	#[test]
	fn plain_named_event_reports_not_custom_event() {
		let raw = NativeEvent::new(
			EventName::Custom(Cow::Borrowed("item-selected")),
			BaseEventData::default(),
			NativeEventPayload::default(),
		);
		let event = CustomEvent::<SelectedDetail>::from_raw(raw);

		assert_eq!(
			event.detail(),
			Err(&CustomEventDetailError::NotCustomEvent {
				event_type: "item-selected".to_owned(),
			})
		);
	}

	#[test]
	fn custom_event_detail_borrows_and_into_detail_owns_the_same_payload() {
		let raw = custom_native_event(serde_json::json!({ "id": 42 }));
		let event = CustomEvent::<SelectedDetail>::from_raw(raw);
		assert_eq!(event.detail().expect("detail").id, 42);
		assert_eq!(event.into_detail().expect("owned detail").id, 42);
	}

	#[test]
	#[serial(custom_event_decode)]
	fn custom_event_detail_decodes_success_only_once() {
		SUCCESSFUL_DECODE_COUNT.store(0, Ordering::SeqCst);
		let event =
			CustomEvent::<DecodesOnce>::from_raw(custom_native_event(serde_json::json!({})));

		assert!(event.detail().is_ok());
		assert!(event.detail().is_ok());
		assert_eq!(SUCCESSFUL_DECODE_COUNT.load(Ordering::SeqCst), 1);
	}

	#[test]
	#[serial(custom_event_decode)]
	fn custom_event_detail_caches_deserialization_failure() {
		FAILING_DECODE_COUNT.store(0, Ordering::SeqCst);
		let event =
			CustomEvent::<AlwaysFails>::from_raw(custom_native_event(serde_json::json!({})));

		assert!(matches!(
			event.detail(),
			Err(CustomEventDetailError::Deserialize { .. })
		));
		assert!(matches!(
			event.detail(),
			Err(CustomEventDetailError::Deserialize { .. })
		));
		assert_eq!(FAILING_DECODE_COUNT.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn custom_event_exposes_base_event_state_and_target_snapshots() {
		let raw = custom_native_event(serde_json::json!({ "id": 42 }))
			.with_target(NativeEventTarget::new("span").with_text_content("Save"))
			.with_current_target(NativeEventTarget::new("button").with_attribute("type", "submit"));
		let event = CustomEvent::<SelectedDetail>::from_raw(raw);

		assert_eq!(event.event_type(), "item-selected");
		assert_eq!(
			event.raw().custom_detail(),
			Some(&serde_json::json!({ "id": 42 }))
		);
		assert_eq!(event.target().expect("origin target").tag_name(), "span");
		assert_eq!(
			event.current_target().expect("listener target").tag_name(),
			"button"
		);
		assert!(!event.default_prevented());

		event.prevent_default();
		event.stop_propagation();
		event.stop_immediate_propagation();

		assert!(event.default_prevented());
		assert!(event.raw().propagation_stopped());
		assert!(event.raw().immediate_propagation_stopped());
	}

	#[test]
	fn custom_event_detail_error_display_is_stable() {
		assert_eq!(
			CustomEventDetailError::NotCustomEvent {
				event_type: "item-selected".to_owned(),
			}
			.to_string(),
			"`item-selected` is not a CustomEvent"
		);
		assert_eq!(
			CustomEventDetailError::Deserialize {
				event_type: "item-selected".to_owned(),
				target_type: "crate::SelectedDetail",
				message: "decoder message".to_owned(),
			}
			.to_string(),
			"failed to deserialize `item-selected` detail as `crate::SelectedDetail`: decoder message"
		);
	}
}
