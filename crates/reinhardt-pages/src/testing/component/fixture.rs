//! Validated synthetic event fixtures for native component tests.

use std::borrow::Cow;
use std::fmt;

use reinhardt_core::types::page::{
	BaseEventData, ModifierState, MouseEventData, NativeEvent, NativeEventFile, NativeEventPayload,
	NativeEventTarget, TouchPointData,
};
use reinhardt_event_catalog::{EventFixtureDefaults, EventInterface, EventName, KnownEvent};

use crate::event::{
	Modifiers, MouseButton, MouseButtons, PointerKind, SecurityPolicyViolationDetails, TouchPoint,
	XrInputSourceDescriptor,
};

/// Failure to validate or apply a native event fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventFixtureError {
	/// A custom event name was empty.
	InvalidName {
		/// Invalid event name.
		name: String,
	},
	/// The selected payload family is not accepted by the event catalog.
	IncompatibleFamily {
		/// Exact event name.
		event: String,
		/// Catalog primary interface.
		expected: EventInterface,
		/// Catalog fallback interfaces.
		fallbacks: &'static [EventInterface],
		/// Fixture interface.
		actual: EventInterface,
	},
	/// A fixture field does not belong to the selected payload family.
	IncompatibleField {
		/// Exact event name.
		event: String,
		/// Incompatible builder field.
		field: &'static str,
		/// Fixture interface.
		actual: EventInterface,
	},
	/// Custom detail serialization failed.
	DetailSerialization {
		/// Exact event name.
		event: String,
		/// Type that could not be serialized.
		target_type: &'static str,
		/// Serialization error message.
		message: String,
	},
	/// Target state cannot be represented by the selected element.
	UnsupportedTargetState {
		/// Unsupported target property.
		property: &'static str,
		/// Actual target tag.
		actual_tag: String,
	},
}

impl fmt::Display for EventFixtureError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidName { name } => write!(formatter, "invalid custom event name `{name}`"),
			Self::IncompatibleFamily {
				event,
				expected,
				fallbacks,
				actual,
			} => {
				write!(formatter, "`{event}` requires {expected:?} fixture data")?;
				for fallback in *fallbacks {
					write!(formatter, " or {fallback:?}")?;
				}
				write!(formatter, ", got {actual:?}")
			}
			Self::IncompatibleField {
				event,
				field,
				actual,
			} => write!(
				formatter,
				"`{field}` is incompatible with {actual:?} fixture data for `{event}`"
			),
			Self::DetailSerialization {
				event,
				target_type,
				message,
			} => write!(
				formatter,
				"failed to serialize custom detail for `{event}` as `{target_type}`: {message}"
			),
			Self::UnsupportedTargetState {
				property,
				actual_tag,
			} => write!(
				formatter,
				"target `{actual_tag}` does not support fixture property `{property}`"
			),
		}
	}
}

impl std::error::Error for EventFixtureError {}

#[derive(Debug, Clone, Default)]
pub(crate) struct TargetStatePatch {
	pub value: Option<String>,
	pub checked: Option<bool>,
	pub selected_values: Option<Vec<String>>,
	pub files: Option<Vec<NativeEventFile>>,
	pub content_editable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetailSerializationFailure {
	target_type: &'static str,
	message: String,
}

/// Builder for one validated native synthetic event.
#[derive(Debug, Clone)]
pub struct EventFixture {
	name: EventName,
	base: BaseEventData,
	payload: NativeEventPayload,
	custom_detail: Option<serde_json::Value>,
	detail_serialization_error: Option<DetailSerializationFailure>,
	target: TargetStatePatch,
	invalid_field: Option<&'static str>,
}

impl EventFixture {
	/// Creates a fixture for any standardized catalog event.
	#[must_use]
	pub fn new(event: KnownEvent) -> Self {
		let behavior = event.spec().behavior;
		let interface = event.spec().primary_interface;
		Self {
			name: event.into(),
			base: BaseEventData {
				bubbles: behavior.bubbles,
				cancelable: behavior.cancelable,
				composed: behavior.composed,
				time_stamp: 0.0,
				is_trusted: false,
			},
			payload: default_payload(Some(event), interface),
			custom_detail: None,
			detail_serialization_error: None,
			target: TargetStatePatch::default(),
			invalid_field: None,
		}
	}

	/// Creates a click fixture.
	#[must_use]
	pub fn click() -> Self {
		Self::new(KnownEvent::Click)
	}

	/// Creates a submit fixture.
	#[must_use]
	pub fn submit() -> Self {
		Self::new(KnownEvent::Submit)
	}

	/// Creates an input fixture.
	#[must_use]
	pub fn input() -> Self {
		Self::new(KnownEvent::Input)
	}

	/// Creates a change fixture.
	#[must_use]
	pub fn change() -> Self {
		Self::new(KnownEvent::Change)
	}

	/// Creates a key-down fixture.
	#[must_use]
	pub fn key_down() -> Self {
		Self::new(KnownEvent::KeyDown)
	}

	/// Creates a pointer-move fixture.
	#[must_use]
	pub fn pointer_move() -> Self {
		Self::new(KnownEvent::PointerMove)
	}

	/// Creates a raw custom-event fixture with generic payload data.
	#[must_use]
	pub fn custom(name: impl Into<String>) -> Self {
		let name = name.into();
		Self {
			name: EventName::Custom(Cow::Owned(name)),
			base: BaseEventData::default(),
			payload: NativeEventPayload::default(),
			custom_detail: None,
			detail_serialization_error: None,
			target: TargetStatePatch::default(),
			invalid_field: None,
		}
	}

	/// Replaces the payload interface while deferring catalog validation.
	#[must_use]
	pub fn interface(mut self, interface: EventInterface) -> Self {
		self.payload = default_payload(self.name.known(), interface);
		self
	}

	/// Overrides whether the event traverses ancestor listeners.
	#[must_use]
	pub const fn bubbles(mut self, bubbles: bool) -> Self {
		self.base.bubbles = bubbles;
		self
	}

	/// Overrides whether the event permits `prevent_default`.
	#[must_use]
	pub const fn cancelable(mut self, cancelable: bool) -> Self {
		self.base.cancelable = cancelable;
		self
	}

	/// Overrides whether the event crosses a shadow boundary.
	#[must_use]
	pub const fn composed(mut self, composed: bool) -> Self {
		self.base.composed = composed;
		self
	}

	/// Overrides the deterministic event timestamp.
	#[must_use]
	pub const fn time_stamp(mut self, time_stamp: f64) -> Self {
		self.base.time_stamp = time_stamp;
		self
	}

	/// Overrides the trusted-event flag.
	#[must_use]
	pub const fn is_trusted(mut self, is_trusted: bool) -> Self {
		self.base.is_trusted = is_trusted;
		self
	}

	/// Serializes and sets a custom event detail payload.
	#[must_use]
	pub fn custom_detail<T>(mut self, detail: &T) -> Self
	where
		T: serde::Serialize + ?Sized,
	{
		if self.name.known().is_some() {
			self.reject_field("custom_detail");
			return self;
		}
		self.custom_detail = None;
		self.detail_serialization_error = None;
		match serde_json::to_value(detail) {
			Ok(value) => self.custom_detail = Some(value),
			Err(error) => {
				self.detail_serialization_error = Some(DetailSerializationFailure {
					target_type: std::any::type_name::<T>(),
					message: error.to_string(),
				});
			}
		}
		self
	}

	/// Sets a pre-serialized custom event detail payload.
	#[must_use]
	pub fn custom_detail_value(mut self, detail: serde_json::Value) -> Self {
		if self.name.known().is_some() {
			self.reject_field("custom_detail");
			return self;
		}
		self.custom_detail = Some(detail);
		self.detail_serialization_error = None;
		self
	}

	/// Updates the target value before invoking listeners.
	#[must_use]
	pub fn value(mut self, value: impl Into<String>) -> Self {
		self.target.value = Some(value.into());
		self
	}

	/// Updates the target checked state before invoking listeners.
	#[must_use]
	pub const fn checked(mut self, checked: bool) -> Self {
		self.target.checked = Some(checked);
		self
	}

	/// Replaces selected target values before invoking listeners.
	#[must_use]
	pub fn selected_values<I, V>(mut self, values: I) -> Self
	where
		I: IntoIterator<Item = V>,
		V: Into<String>,
	{
		self.target.selected_values = Some(values.into_iter().map(Into::into).collect());
		self
	}

	/// Adds one file snapshot to the target before invoking listeners.
	#[must_use]
	pub fn file(
		mut self,
		name: impl Into<String>,
		media_type: impl Into<String>,
		size: u64,
		last_modified: i64,
	) -> Self {
		self.target
			.files
			.get_or_insert_default()
			.push(NativeEventFile::new(name, media_type, size, last_modified));
		self
	}

	/// Overrides the target's contenteditable state.
	#[must_use]
	pub const fn content_editable(mut self, content_editable: bool) -> Self {
		self.target.content_editable = Some(content_editable);
		self
	}

	/// Sets the logical keyboard key.
	#[must_use]
	pub fn key(mut self, key: impl Into<String>) -> Self {
		if let NativeEventPayload::Keyboard(data) = &mut self.payload {
			data.key = key.into();
		} else {
			self.reject_field("key");
		}
		self
	}

	/// Sets the physical keyboard code.
	#[must_use]
	pub fn code(mut self, code: impl Into<String>) -> Self {
		if let NativeEventPayload::Keyboard(data) = &mut self.payload {
			data.code = code.into();
		} else {
			self.reject_field("code");
		}
		self
	}

	/// Sets the keyboard location.
	#[must_use]
	pub fn location(mut self, location: u32) -> Self {
		if let NativeEventPayload::Keyboard(data) = &mut self.payload {
			data.location = location;
		} else {
			self.reject_field("location");
		}
		self
	}

	/// Sets the automatic-repeat flag.
	#[must_use]
	pub fn repeat(mut self, repeat: bool) -> Self {
		if let NativeEventPayload::Keyboard(data) = &mut self.payload {
			data.repeat = repeat;
		} else {
			self.reject_field("repeat");
		}
		self
	}

	/// Sets the input or keyboard composition flag.
	#[must_use]
	pub fn is_composing(mut self, is_composing: bool) -> Self {
		match &mut self.payload {
			NativeEventPayload::Input(data) => data.is_composing = is_composing,
			NativeEventPayload::Keyboard(data) => data.is_composing = is_composing,
			_ => self.reject_field("is_composing"),
		}
		self
	}

	/// Sets keyboard modifiers for compatible payload families.
	#[must_use]
	pub fn modifiers(mut self, modifiers: Modifiers) -> Self {
		let modifiers = ModifierState {
			alt: modifiers.alt,
			control: modifiers.control,
			meta: modifiers.meta,
			shift: modifiers.shift,
		};
		match &mut self.payload {
			NativeEventPayload::Keyboard(data) => data.modifiers = modifiers,
			NativeEventPayload::Touch(data) => data.modifiers = modifiers,
			payload => {
				if let Some(mouse) = mouse_data_mut(payload) {
					mouse.modifiers = modifiers;
				} else {
					self.reject_field("modifiers");
				}
			}
		}
		self
	}

	/// Sets inserted input data.
	#[must_use]
	pub fn input_data(mut self, data: impl Into<String>) -> Self {
		if let NativeEventPayload::Input(input) = &mut self.payload {
			input.data = Some(data.into());
		} else {
			self.reject_field("input_data");
		}
		self
	}

	/// Sets the standardized input operation type.
	#[must_use]
	pub fn input_type(mut self, input_type: impl Into<String>) -> Self {
		if let NativeEventPayload::Input(input) = &mut self.payload {
			input.input_type = Some(input_type.into());
		} else {
			self.reject_field("input_type");
		}
		self
	}

	/// Sets viewport-relative pointer coordinates.
	#[must_use]
	pub fn client_position(mut self, x: f64, y: f64) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.client_x = x;
			mouse.client_y = y;
		} else {
			self.reject_field("client_position");
		}
		self
	}

	/// Sets screen-relative pointer coordinates.
	#[must_use]
	pub fn screen_position(mut self, x: f64, y: f64) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.screen_x = x;
			mouse.screen_y = y;
		} else {
			self.reject_field("screen_position");
		}
		self
	}

	/// Sets document-relative pointer coordinates.
	#[must_use]
	pub fn page_position(mut self, x: f64, y: f64) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.page_x = x;
			mouse.page_y = y;
		} else {
			self.reject_field("page_position");
		}
		self
	}

	/// Sets target-relative pointer coordinates.
	#[must_use]
	pub fn offset_position(mut self, x: f64, y: f64) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.offset_x = x;
			mouse.offset_y = y;
		} else {
			self.reject_field("offset_position");
		}
		self
	}

	/// Sets the changed mouse button.
	#[must_use]
	pub fn button(mut self, button: MouseButton) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.button = mouse_button_code(button);
		} else {
			self.reject_field("button");
		}
		self
	}

	/// Sets the pressed mouse-button mask.
	#[must_use]
	pub fn buttons(mut self, buttons: MouseButtons) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.buttons = buttons.bits();
		} else {
			self.reject_field("buttons");
		}
		self
	}

	/// Sets mouse-compatible interface detail.
	#[must_use]
	pub fn detail(mut self, detail: i32) -> Self {
		if let Some(mouse) = mouse_data_mut(&mut self.payload) {
			mouse.detail = detail;
		} else {
			self.reject_field("detail");
		}
		self
	}

	/// Sets the stable pointer identifier.
	#[must_use]
	pub fn pointer_id(mut self, pointer_id: i32) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.pointer_id = pointer_id;
		} else {
			self.reject_field("pointer_id");
		}
		self
	}

	/// Sets the pointer device kind.
	#[must_use]
	pub fn pointer_kind(mut self, pointer_kind: PointerKind) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.pointer_kind = match pointer_kind {
				PointerKind::Mouse => "mouse".to_string(),
				PointerKind::Pen => "pen".to_string(),
				PointerKind::Touch => "touch".to_string(),
				PointerKind::Other(kind) => kind,
			};
		} else {
			self.reject_field("pointer_kind");
		}
		self
	}

	/// Sets normalized pointer pressure.
	#[must_use]
	pub fn pressure(mut self, pressure: f32) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.pressure = pressure;
		} else {
			self.reject_field("pressure");
		}
		self
	}

	/// Sets pointer contact dimensions.
	#[must_use]
	pub fn contact_size(mut self, width: f64, height: f64) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.width = width;
			data.height = height;
		} else {
			self.reject_field("contact_size");
		}
		self
	}

	/// Sets pointer barrel pressure.
	#[must_use]
	pub fn tangential_pressure(mut self, pressure: f32) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.tangential_pressure = pressure;
		} else {
			self.reject_field("tangential_pressure");
		}
		self
	}

	/// Sets pointer tilt.
	#[must_use]
	pub fn tilt(mut self, x: i32, y: i32) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.tilt_x = x;
			data.tilt_y = y;
		} else {
			self.reject_field("tilt");
		}
		self
	}

	/// Sets pointer rotation.
	#[must_use]
	pub fn twist(mut self, twist: i32) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.twist = twist;
		} else {
			self.reject_field("twist");
		}
		self
	}

	/// Sets whether this is the primary pointer.
	#[must_use]
	pub fn is_primary(mut self, is_primary: bool) -> Self {
		if let NativeEventPayload::Pointer(data) = &mut self.payload {
			data.is_primary = is_primary;
		} else {
			self.reject_field("is_primary");
		}
		self
	}

	/// Sets CSS animation interface data.
	#[must_use]
	pub fn animation(
		mut self,
		name: impl Into<String>,
		elapsed_time: f64,
		pseudo_element: impl Into<String>,
	) -> Self {
		if let NativeEventPayload::Animation(data) = &mut self.payload {
			data.animation_name = name.into();
			data.elapsed_time = elapsed_time;
			data.pseudo_element = pseudo_element.into();
		} else {
			self.reject_field("animation");
		}
		self
	}

	/// Sets plain-text clipboard data.
	#[must_use]
	pub fn clipboard_text(mut self, text: impl Into<String>) -> Self {
		if let NativeEventPayload::Clipboard(data) = &mut self.payload {
			data.text = Some(text.into());
		} else {
			self.reject_field("clipboard_text");
		}
		self
	}

	/// Sets the HTML command name.
	#[must_use]
	pub fn command(mut self, command: impl Into<String>) -> Self {
		if let NativeEventPayload::Command(data) = &mut self.payload {
			data.command = command.into();
		} else {
			self.reject_field("command");
		}
		self
	}

	/// Sets the command source to a target snapshot with the supplied tag.
	#[must_use]
	pub fn command_source(mut self, tag_name: impl Into<String>) -> Self {
		if let NativeEventPayload::Command(data) = &mut self.payload {
			data.source = Some(NativeEventTarget::new(tag_name));
		} else {
			self.reject_field("command_source");
		}
		self
	}

	/// Sets input-method composition data.
	#[must_use]
	pub fn composition_data(mut self, value: impl Into<String>) -> Self {
		if let NativeEventPayload::Composition(data) = &mut self.payload {
			data.data = value.into();
		} else {
			self.reject_field("composition_data");
		}
		self
	}

	/// Sets plain-text drag data.
	#[must_use]
	pub fn drag_data(mut self, value: impl Into<String>) -> Self {
		if let NativeEventPayload::Drag(data) = &mut self.payload {
			data.data = Some(value.into());
		} else {
			self.reject_field("drag_data");
		}
		self
	}

	/// Sets a focus-related target snapshot with the supplied tag.
	#[must_use]
	pub fn related_target(mut self, tag_name: impl Into<String>) -> Self {
		if let NativeEventPayload::Focus(data) = &mut self.payload {
			data.related_target = Some(NativeEventTarget::new(tag_name));
		} else {
			self.reject_field("related_target");
		}
		self
	}

	/// Sets encrypted-media initialization data.
	#[must_use]
	pub fn encrypted_data(
		mut self,
		init_data_type: impl Into<String>,
		init_data: impl IntoIterator<Item = u8>,
	) -> Self {
		if let NativeEventPayload::MediaEncrypted(data) = &mut self.payload {
			data.init_data_type = init_data_type.into();
			data.init_data = init_data.into_iter().collect();
		} else {
			self.reject_field("encrypted_data");
		}
		self
	}

	/// Sets picture-in-picture window dimensions.
	#[must_use]
	pub fn picture_in_picture_size(mut self, width: u32, height: u32) -> Self {
		if let NativeEventPayload::PictureInPicture(data) = &mut self.payload {
			data.width = width;
			data.height = height;
		} else {
			self.reject_field("picture_in_picture_size");
		}
		self
	}

	/// Sets complete Content Security Policy violation details.
	#[must_use]
	pub fn security_policy_violation(mut self, details: SecurityPolicyViolationDetails) -> Self {
		if let NativeEventPayload::SecurityPolicyViolation(data) = &mut self.payload {
			data.blocked_uri = details.blocked_uri;
			data.column_number = details.column_number;
			data.disposition = details.disposition;
			data.document_uri = details.document_uri;
			data.effective_directive = details.effective_directive;
			data.line_number = details.line_number;
			data.original_policy = details.original_policy;
			data.referrer = details.referrer;
			data.sample = details.sample;
			data.source_file = details.source_file;
			data.status_code = details.status_code;
			data.violated_directive = details.violated_directive;
		} else {
			self.reject_field("security_policy_violation");
		}
		self
	}

	/// Sets the submitter to a target snapshot with the supplied tag.
	#[must_use]
	pub fn submitter(mut self, tag_name: impl Into<String>) -> Self {
		if let NativeEventPayload::Submit(data) = &mut self.payload {
			data.submitter = Some(NativeEventTarget::new(tag_name));
		} else {
			self.reject_field("submitter");
		}
		self
	}

	/// Sets SVG timing detail.
	#[must_use]
	pub fn time_detail(mut self, detail: i32) -> Self {
		if let NativeEventPayload::Time(data) = &mut self.payload {
			data.detail = detail;
		} else {
			self.reject_field("time_detail");
		}
		self
	}

	/// Sets old and new HTML toggle states.
	#[must_use]
	pub fn toggle_states(
		mut self,
		old_state: impl Into<String>,
		new_state: impl Into<String>,
	) -> Self {
		if let NativeEventPayload::Toggle(data) = &mut self.payload {
			data.old_state = old_state.into();
			data.new_state = new_state.into();
		} else {
			self.reject_field("toggle_states");
		}
		self
	}

	/// Replaces all current touch points.
	#[must_use]
	pub fn touches(mut self, touches: impl IntoIterator<Item = TouchPoint>) -> Self {
		if let NativeEventPayload::Touch(data) = &mut self.payload {
			data.touches = touches.into_iter().map(native_touch_point).collect();
		} else {
			self.reject_field("touches");
		}
		self
	}

	/// Replaces touch points that began on the target.
	#[must_use]
	pub fn target_touches(mut self, touches: impl IntoIterator<Item = TouchPoint>) -> Self {
		if let NativeEventPayload::Touch(data) = &mut self.payload {
			data.target_touches = touches.into_iter().map(native_touch_point).collect();
		} else {
			self.reject_field("target_touches");
		}
		self
	}

	/// Replaces touch points changed by this event.
	#[must_use]
	pub fn changed_touches(mut self, touches: impl IntoIterator<Item = TouchPoint>) -> Self {
		if let NativeEventPayload::Touch(data) = &mut self.payload {
			data.changed_touches = touches.into_iter().map(native_touch_point).collect();
		} else {
			self.reject_field("changed_touches");
		}
		self
	}

	/// Sets CSS transition interface data.
	#[must_use]
	pub fn transition(
		mut self,
		property_name: impl Into<String>,
		elapsed_time: f64,
		pseudo_element: impl Into<String>,
	) -> Self {
		if let NativeEventPayload::Transition(data) = &mut self.payload {
			data.property_name = property_name.into();
			data.elapsed_time = elapsed_time;
			data.pseudo_element = pseudo_element.into();
		} else {
			self.reject_field("transition");
		}
		self
	}

	/// Sets wheel delta data.
	#[must_use]
	pub fn wheel_delta(
		mut self,
		delta_x: f64,
		delta_y: f64,
		delta_z: f64,
		delta_mode: u32,
	) -> Self {
		if let NativeEventPayload::Wheel(data) = &mut self.payload {
			data.delta_x = delta_x;
			data.delta_y = delta_y;
			data.delta_z = delta_z;
			data.delta_mode = delta_mode;
		} else {
			self.reject_field("wheel_delta");
		}
		self
	}

	/// Sets owned WebXR input-source metadata.
	#[must_use]
	pub fn xr_input_source(mut self, descriptor: XrInputSourceDescriptor) -> Self {
		if let NativeEventPayload::XrInputSource(data) = &mut self.payload {
			data.handedness = descriptor.handedness;
			data.target_ray_mode = descriptor.target_ray_mode;
			data.profiles = descriptor.profiles;
		} else {
			self.reject_field("xr_input_source");
		}
		self
	}

	/// Validates the fixture and constructs its raw native event.
	pub fn build(&self) -> Result<NativeEvent, EventFixtureError> {
		self.validate()?;
		let event = NativeEvent::new(self.name.clone(), self.base, self.payload.clone());
		Ok(match &self.custom_detail {
			Some(detail) => event.with_custom_detail(detail.clone()),
			None => event,
		})
	}

	pub(crate) fn name(&self) -> &EventName {
		&self.name
	}

	pub(crate) const fn target(&self) -> &TargetStatePatch {
		&self.target
	}

	fn validate(&self) -> Result<(), EventFixtureError> {
		if self.name.as_str().is_empty() {
			return Err(EventFixtureError::InvalidName {
				name: String::new(),
			});
		}
		let actual = self.payload.interface();
		if let Some(event) = self.name.known() {
			let spec = event.spec();
			if actual != spec.primary_interface && !spec.fallback_interfaces.contains(&actual) {
				return Err(EventFixtureError::IncompatibleFamily {
					event: spec.dom_name.to_string(),
					expected: spec.primary_interface,
					fallbacks: spec.fallback_interfaces,
					actual,
				});
			}
		} else if actual != EventInterface::Generic {
			return Err(EventFixtureError::IncompatibleFamily {
				event: self.name.as_str().to_string(),
				expected: EventInterface::Generic,
				fallbacks: &[],
				actual,
			});
		}
		if let Some(field) = self.invalid_field {
			return Err(EventFixtureError::IncompatibleField {
				event: self.name.as_str().to_string(),
				field,
				actual,
			});
		}
		if let Some(error) = &self.detail_serialization_error {
			return Err(EventFixtureError::DetailSerialization {
				event: self.name.as_str().to_string(),
				target_type: error.target_type,
				message: error.message.clone(),
			});
		}
		Ok(())
	}

	fn reject_field(&mut self, field: &'static str) {
		self.invalid_field.get_or_insert(field);
	}
}

fn default_payload(event: Option<KnownEvent>, interface: EventInterface) -> NativeEventPayload {
	let mut payload = NativeEventPayload::for_interface(interface);
	let defaults = event.map_or(EventFixtureDefaults::NONE, |event| {
		event.spec().fixture_defaults
	});
	if let Some(mouse) = mouse_data_mut(&mut payload) {
		mouse.button = defaults.mouse_button;
		mouse.buttons = defaults.mouse_buttons;
	}
	if let NativeEventPayload::Pointer(data) = &mut payload {
		data.pointer_kind = "mouse".to_string();
		data.is_primary = true;
	}
	payload
}

fn native_touch_point(point: TouchPoint) -> TouchPointData {
	TouchPointData {
		identifier: point.identifier,
		client_x: point.client_position.x,
		client_y: point.client_position.y,
		screen_x: point.screen_position.x,
		screen_y: point.screen_position.y,
		page_x: point.page_position.x,
		page_y: point.page_position.y,
		radius_x: point.radius.x,
		radius_y: point.radius.y,
		rotation_angle: point.rotation_angle,
		force: point.force,
	}
}

fn mouse_data_mut(payload: &mut NativeEventPayload) -> Option<&mut MouseEventData> {
	match payload {
		NativeEventPayload::Mouse(data) => Some(data),
		NativeEventPayload::Pointer(data) => Some(&mut data.mouse),
		NativeEventPayload::Drag(data) => Some(&mut data.mouse),
		NativeEventPayload::Wheel(data) => Some(&mut data.mouse),
		_ => None,
	}
}

fn mouse_button_code(button: MouseButton) -> i16 {
	match button {
		MouseButton::None => -1,
		MouseButton::Primary => 0,
		MouseButton::Auxiliary => 1,
		MouseButton::Secondary => 2,
		MouseButton::Fourth => 3,
		MouseButton::Fifth => 4,
		MouseButton::Other(code) => code,
	}
}

#[cfg(test)]
mod tests {
	use serde::ser::Error as _;

	use super::{EventFixture, EventFixtureError};

	#[derive(serde::Serialize)]
	struct SelectedDetail {
		id: u64,
	}

	struct FailingDetail;

	impl serde::Serialize for FailingDetail {
		fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			Err(S::Error::custom("fixture serialization failed"))
		}
	}

	#[test]
	fn custom_detail_serializes_into_native_event() {
		let event = EventFixture::custom("item-selected")
			.custom_detail(&SelectedDetail { id: 42 })
			.build()
			.expect("typed custom fixture");

		assert_eq!(
			event.custom_detail(),
			Some(&serde_json::json!({ "id": 42 }))
		);
	}

	#[test]
	fn raw_custom_fixture_has_no_custom_detail() {
		let event = EventFixture::custom("item-selected")
			.build()
			.expect("raw custom fixture");
		assert_eq!(event.custom_detail(), None);
	}

	#[test]
	fn custom_detail_reports_serialization_failure() {
		let error = EventFixture::custom("item-selected")
			.custom_detail(&FailingDetail)
			.build()
			.expect_err("failing detail serialization");

		assert_eq!(
			error,
			EventFixtureError::DetailSerialization {
				event: "item-selected".to_string(),
				target_type: std::any::type_name::<FailingDetail>(),
				message: "fixture serialization failed".to_string(),
			}
		);
		assert_eq!(
			error.to_string(),
			format!(
				"failed to serialize custom detail for `item-selected` as `{}`: fixture serialization failed",
				std::any::type_name::<FailingDetail>()
			)
		);
	}

	#[test]
	fn standard_fixture_rejects_custom_detail() {
		let error = EventFixture::click()
			.custom_detail_value(serde_json::Value::Null)
			.build()
			.expect_err("standard event custom detail");

		assert!(matches!(
			error,
			EventFixtureError::IncompatibleField {
				field: "custom_detail",
				..
			}
		));
	}

	#[test]
	fn second_custom_detail_setter_replaces_first() {
		let event = EventFixture::custom("item-selected")
			.custom_detail(&SelectedDetail { id: 1 })
			.custom_detail_value(serde_json::json!({ "id": 2 }))
			.build()
			.expect("replacement custom detail");

		assert_eq!(event.custom_detail(), Some(&serde_json::json!({ "id": 2 })));
	}
}
