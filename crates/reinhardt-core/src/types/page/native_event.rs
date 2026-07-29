//! Owned raw event transport for native page rendering and testing.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use reinhardt_event_catalog::{EventInterface, EventName, KnownEvent};

/// Platform-independent base flags captured for a native event.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BaseEventData {
	/// Whether the event traverses ancestor listeners.
	pub bubbles: bool,
	/// Whether `prevent_default` may cancel the event.
	pub cancelable: bool,
	/// Whether the event crosses a shadow DOM boundary.
	pub composed: bool,
	/// Event creation time in milliseconds.
	pub time_stamp: f64,
	/// Whether the user agent, rather than application code, created the event.
	pub is_trusted: bool,
}

/// Keyboard modifier flags shared by mouse, pointer, keyboard, and touch data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierState {
	/// Whether the Alt key was active.
	pub alt: bool,
	/// Whether the Control key was active.
	pub control: bool,
	/// Whether the Meta key was active.
	pub meta: bool,
	/// Whether the Shift key was active.
	pub shift: bool,
}

/// Owned metadata snapshot for a file selected by a native event target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEventFile {
	/// File name without a path.
	pub name: String,
	/// Reported media type.
	pub media_type: String,
	/// File size in bytes.
	pub size: u64,
	/// Last-modified timestamp in milliseconds since the Unix epoch.
	pub last_modified: i64,
}

impl NativeEventFile {
	/// Creates an owned file metadata snapshot.
	pub fn new(
		name: impl Into<String>,
		media_type: impl Into<String>,
		size: u64,
		last_modified: i64,
	) -> Self {
		Self {
			name: name.into(),
			media_type: media_type.into(),
			size,
			last_modified,
		}
	}

	/// Returns the file name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Returns the reported media type.
	pub fn media_type(&self) -> &str {
		&self.media_type
	}
}

/// Owned snapshot of the stable state exposed by an event target.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeEventTarget {
	tag_name: String,
	attributes: BTreeMap<String, String>,
	value: Option<String>,
	checked: Option<bool>,
	selected_values: Vec<String>,
	files: Vec<NativeEventFile>,
	text_content: Option<String>,
	content_editable: bool,
}

impl NativeEventTarget {
	/// Creates a target snapshot with a normalized lowercase tag name.
	pub fn new(tag_name: impl Into<String>) -> Self {
		Self {
			tag_name: tag_name.into().to_ascii_lowercase(),
			..Self::default()
		}
	}

	/// Returns the normalized tag name.
	pub fn tag_name(&self) -> &str {
		&self.tag_name
	}

	/// Returns all captured attributes.
	pub fn attributes(&self) -> &BTreeMap<String, String> {
		&self.attributes
	}

	/// Returns one captured attribute.
	pub fn attribute(&self, name: &str) -> Option<&str> {
		self.attributes.get(name).map(String::as_str)
	}

	/// Returns the captured control value.
	pub fn value(&self) -> Option<&str> {
		self.value.as_deref()
	}

	/// Returns the captured checked state.
	pub const fn checked(&self) -> Option<bool> {
		self.checked
	}

	/// Returns the selected control values.
	pub fn selected_values(&self) -> &[String] {
		&self.selected_values
	}

	/// Returns captured selected-file metadata.
	pub fn files(&self) -> &[NativeEventFile] {
		&self.files
	}

	/// Returns captured descendant text.
	pub fn text_content(&self) -> Option<&str> {
		self.text_content.as_deref()
	}

	/// Returns whether the target was contenteditable.
	pub const fn is_content_editable(&self) -> bool {
		self.content_editable
	}

	/// Adds or replaces a captured attribute.
	pub fn with_attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.attributes.insert(name.into(), value.into());
		self
	}

	/// Sets the captured control value.
	pub fn with_value(mut self, value: impl Into<String>) -> Self {
		self.value = Some(value.into());
		self
	}

	/// Sets the captured checked state.
	pub const fn with_checked(mut self, checked: bool) -> Self {
		self.checked = Some(checked);
		self
	}

	/// Replaces the captured selected values.
	pub fn with_selected_values<I, V>(mut self, values: I) -> Self
	where
		I: IntoIterator<Item = V>,
		V: Into<String>,
	{
		self.selected_values = values.into_iter().map(Into::into).collect();
		self
	}

	/// Adds captured file metadata.
	pub fn with_file(mut self, file: NativeEventFile) -> Self {
		self.files.push(file);
		self
	}

	/// Replaces all captured file metadata.
	pub fn with_files(mut self, files: impl IntoIterator<Item = NativeEventFile>) -> Self {
		self.files = files.into_iter().collect();
		self
	}

	/// Sets captured descendant text.
	pub fn with_text_content(mut self, text_content: impl Into<String>) -> Self {
		self.text_content = Some(text_content.into());
		self
	}

	/// Sets whether the target was contenteditable.
	pub const fn with_content_editable(mut self, content_editable: bool) -> Self {
		self.content_editable = content_editable;
		self
	}
}

/// Data carried by the base DOM `Event` interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericEventData;

/// Data carried by the CSS animation event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationEventData {
	/// Animation name.
	pub animation_name: String,
	/// Elapsed animation time in seconds.
	pub elapsed_time: f64,
	/// Pseudo-element name, when applicable.
	pub pseudo_element: String,
}

/// Data carried by the clipboard event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipboardEventData {
	/// Plain-text clipboard snapshot, when available.
	pub text: Option<String>,
}

/// Data carried by the HTML command event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandEventData {
	/// Command name.
	pub command: String,
	/// Element that invoked the command, when available.
	pub source: Option<NativeEventTarget>,
}

/// Data carried by the composition event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompositionEventData {
	/// Characters produced by the input method.
	pub data: String,
}

/// Shared mouse-interface data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MouseEventData {
	/// Horizontal viewport coordinate.
	pub client_x: f64,
	/// Vertical viewport coordinate.
	pub client_y: f64,
	/// Horizontal screen coordinate.
	pub screen_x: f64,
	/// Vertical screen coordinate.
	pub screen_y: f64,
	/// Horizontal document coordinate.
	pub page_x: f64,
	/// Vertical document coordinate.
	pub page_y: f64,
	/// Horizontal coordinate relative to the target.
	pub offset_x: f64,
	/// Vertical coordinate relative to the target.
	pub offset_y: f64,
	/// Changed button number.
	pub button: i16,
	/// Bitmask of currently pressed buttons.
	pub buttons: u16,
	/// Click count or interface-specific detail.
	pub detail: i32,
	/// Keyboard modifiers active during the event.
	pub modifiers: ModifierState,
}

/// Data carried by the drag-and-drop event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DragEventData {
	/// Mouse portion of the drag event.
	pub mouse: MouseEventData,
	/// Plain-text data-transfer snapshot, when available.
	pub data: Option<String>,
}

/// Data carried by the focus event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusEventData {
	/// Focus target related to the transition.
	pub related_target: Option<NativeEventTarget>,
}

/// Data carried by the input event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputEventData {
	/// Inserted characters, when the operation has textual data.
	pub data: Option<String>,
	/// Input operation type.
	pub input_type: Option<String>,
	/// Whether the operation occurs inside an active composition session.
	pub is_composing: bool,
}

/// Data carried by the keyboard event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyboardEventData {
	/// Logical key value.
	pub key: String,
	/// Physical key code.
	pub code: String,
	/// Keyboard location value.
	pub location: u32,
	/// Whether this event is an automatic repeat.
	pub repeat: bool,
	/// Whether the key event occurs during composition.
	pub is_composing: bool,
	/// Keyboard modifiers active during the event.
	pub modifiers: ModifierState,
}

/// Data carried by the encrypted-media event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaEncryptedEventData {
	/// Initialization-data format.
	pub init_data_type: String,
	/// Owned initialization bytes.
	pub init_data: Vec<u8>,
}

/// Data carried by the picture-in-picture event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PictureInPictureEventData {
	/// Picture-in-picture window width in CSS pixels.
	pub width: u32,
	/// Picture-in-picture window height in CSS pixels.
	pub height: u32,
}

/// Data carried by the pointer event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointerEventData {
	/// Mouse-compatible portion of the pointer event.
	pub mouse: MouseEventData,
	/// Stable pointer identifier for the active pointer.
	pub pointer_id: i32,
	/// Pointer device kind such as `mouse`, `pen`, or `touch`.
	pub pointer_kind: String,
	/// Normalized pressure in the range supported by the device.
	pub pressure: f32,
	/// Contact geometry width.
	pub width: f64,
	/// Contact geometry height.
	pub height: f64,
	/// Barrel pressure reported by compatible pen devices.
	pub tangential_pressure: f32,
	/// Pen tilt on the x-axis.
	pub tilt_x: i32,
	/// Pen tilt on the y-axis.
	pub tilt_y: i32,
	/// Pen rotation in degrees.
	pub twist: i32,
	/// Whether this is the primary pointer of its kind.
	pub is_primary: bool,
}

/// Data carried by the Content Security Policy violation interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecurityPolicyViolationEventData {
	/// URI blocked by the policy.
	pub blocked_uri: String,
	/// Source column number.
	pub column_number: u32,
	/// Policy disposition.
	pub disposition: String,
	/// URI of the document where the violation occurred.
	pub document_uri: String,
	/// Effective directive that was violated.
	pub effective_directive: String,
	/// Source line number.
	pub line_number: u32,
	/// Full policy text.
	pub original_policy: String,
	/// Referrer associated with the violation.
	pub referrer: String,
	/// Source sample associated with the violation.
	pub sample: String,
	/// Source file associated with the violation.
	pub source_file: String,
	/// HTTP status code associated with the protected resource.
	pub status_code: u16,
	/// Original directive that was violated.
	pub violated_directive: String,
}

/// Data carried by the submit event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubmitEventData {
	/// Element that initiated submission, when available.
	pub submitter: Option<NativeEventTarget>,
}

/// Data carried by the SVG time event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeEventData {
	/// Repeat or timing detail supplied by the event.
	pub detail: i32,
}

/// Data carried by the HTML toggle event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToggleEventData {
	/// State before the toggle.
	pub old_state: String,
	/// State after the toggle.
	pub new_state: String,
}

/// One contact point captured from a touch event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TouchPointData {
	/// Stable touch identifier.
	pub identifier: i32,
	/// Horizontal viewport coordinate.
	pub client_x: f64,
	/// Vertical viewport coordinate.
	pub client_y: f64,
	/// Horizontal screen coordinate.
	pub screen_x: f64,
	/// Vertical screen coordinate.
	pub screen_y: f64,
	/// Horizontal document coordinate.
	pub page_x: f64,
	/// Vertical document coordinate.
	pub page_y: f64,
	/// Horizontal contact radius.
	pub radius_x: f64,
	/// Vertical contact radius.
	pub radius_y: f64,
	/// Contact rotation angle.
	pub rotation_angle: f64,
	/// Normalized contact force.
	pub force: f32,
}

/// Data carried by the touch event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TouchEventData {
	/// All current touch points.
	pub touches: Vec<TouchPointData>,
	/// Current touch points that began on the target.
	pub target_touches: Vec<TouchPointData>,
	/// Touch points changed by this event.
	pub changed_touches: Vec<TouchPointData>,
	/// Keyboard modifiers active during the event.
	pub modifiers: ModifierState,
}

/// Data carried by the CSS transition event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransitionEventData {
	/// Transitioned CSS property name.
	pub property_name: String,
	/// Elapsed transition time in seconds.
	pub elapsed_time: f64,
	/// Pseudo-element name, when applicable.
	pub pseudo_element: String,
}

/// Data carried by the wheel event interface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WheelEventData {
	/// Mouse-compatible portion of the wheel event.
	pub mouse: MouseEventData,
	/// Horizontal scroll delta.
	pub delta_x: f64,
	/// Vertical scroll delta.
	pub delta_y: f64,
	/// Depth-axis scroll delta.
	pub delta_z: f64,
	/// Unit used by the delta values.
	pub delta_mode: u32,
}

/// Data carried by the WebXR input-source event interface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XrInputSourceEventData {
	/// Input-side classification such as `left`, `right`, or `none`.
	pub handedness: String,
	/// Target-ray mode such as `tracked-pointer` or `gaze`.
	pub target_ray_mode: String,
	/// Ordered WebXR interaction profiles.
	pub profiles: Vec<String>,
}

/// Owned native event payload grouped by browser interface family.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum NativeEventPayload {
	/// Base DOM event data.
	Generic(GenericEventData),
	/// CSS animation data.
	Animation(AnimationEventData),
	/// Clipboard data.
	Clipboard(ClipboardEventData),
	/// HTML command data.
	Command(CommandEventData),
	/// Input-method composition data.
	Composition(CompositionEventData),
	/// Drag-and-drop data.
	Drag(DragEventData),
	/// Focus transition data.
	Focus(FocusEventData),
	/// Text input operation data.
	Input(InputEventData),
	/// Keyboard data.
	Keyboard(KeyboardEventData),
	/// Encrypted-media initialization data.
	MediaEncrypted(MediaEncryptedEventData),
	/// Mouse data.
	Mouse(MouseEventData),
	/// Picture-in-picture window data.
	PictureInPicture(PictureInPictureEventData),
	/// Pointer data.
	Pointer(PointerEventData),
	/// Content Security Policy violation data.
	SecurityPolicyViolation(SecurityPolicyViolationEventData),
	/// Form submission data.
	Submit(SubmitEventData),
	/// SVG time data.
	Time(TimeEventData),
	/// HTML toggle state data.
	Toggle(ToggleEventData),
	/// Touch data.
	Touch(TouchEventData),
	/// CSS transition data.
	Transition(TransitionEventData),
	/// Wheel data.
	Wheel(WheelEventData),
	/// WebXR input-source data.
	XrInputSource(XrInputSourceEventData),
}

impl NativeEventPayload {
	/// Creates deterministic empty data for a catalog interface family.
	pub fn for_interface(interface: EventInterface) -> Self {
		match interface {
			EventInterface::Generic => Self::Generic(GenericEventData),
			EventInterface::Animation => Self::Animation(AnimationEventData::default()),
			EventInterface::Clipboard => Self::Clipboard(ClipboardEventData::default()),
			EventInterface::Command => Self::Command(CommandEventData::default()),
			EventInterface::Composition => Self::Composition(CompositionEventData::default()),
			EventInterface::Drag => Self::Drag(DragEventData::default()),
			EventInterface::Focus => Self::Focus(FocusEventData::default()),
			EventInterface::Input => Self::Input(InputEventData::default()),
			EventInterface::Keyboard => Self::Keyboard(KeyboardEventData::default()),
			EventInterface::MediaEncrypted => {
				Self::MediaEncrypted(MediaEncryptedEventData::default())
			}
			EventInterface::Mouse => Self::Mouse(MouseEventData::default()),
			EventInterface::PictureInPicture => {
				Self::PictureInPicture(PictureInPictureEventData::default())
			}
			EventInterface::Pointer => Self::Pointer(PointerEventData::default()),
			EventInterface::SecurityPolicyViolation => {
				Self::SecurityPolicyViolation(SecurityPolicyViolationEventData::default())
			}
			EventInterface::Submit => Self::Submit(SubmitEventData::default()),
			EventInterface::Time => Self::Time(TimeEventData::default()),
			EventInterface::Toggle => Self::Toggle(ToggleEventData::default()),
			EventInterface::Touch => Self::Touch(TouchEventData::default()),
			EventInterface::Transition => Self::Transition(TransitionEventData::default()),
			EventInterface::Wheel => Self::Wheel(WheelEventData::default()),
			EventInterface::XrInputSource => Self::XrInputSource(XrInputSourceEventData::default()),
			_ => Self::Generic(GenericEventData),
		}
	}

	/// Returns the catalog interface represented by this payload.
	pub const fn interface(&self) -> EventInterface {
		match self {
			Self::Generic(_) => EventInterface::Generic,
			Self::Animation(_) => EventInterface::Animation,
			Self::Clipboard(_) => EventInterface::Clipboard,
			Self::Command(_) => EventInterface::Command,
			Self::Composition(_) => EventInterface::Composition,
			Self::Drag(_) => EventInterface::Drag,
			Self::Focus(_) => EventInterface::Focus,
			Self::Input(_) => EventInterface::Input,
			Self::Keyboard(_) => EventInterface::Keyboard,
			Self::MediaEncrypted(_) => EventInterface::MediaEncrypted,
			Self::Mouse(_) => EventInterface::Mouse,
			Self::PictureInPicture(_) => EventInterface::PictureInPicture,
			Self::Pointer(_) => EventInterface::Pointer,
			Self::SecurityPolicyViolation(_) => EventInterface::SecurityPolicyViolation,
			Self::Submit(_) => EventInterface::Submit,
			Self::Time(_) => EventInterface::Time,
			Self::Toggle(_) => EventInterface::Toggle,
			Self::Touch(_) => EventInterface::Touch,
			Self::Transition(_) => EventInterface::Transition,
			Self::Wheel(_) => EventInterface::Wheel,
			Self::XrInputSource(_) => EventInterface::XrInputSource,
		}
	}
}

impl Default for NativeEventPayload {
	fn default() -> Self {
		Self::Generic(GenericEventData)
	}
}

#[derive(Debug, Default)]
struct NativeDispatchState {
	default_prevented: AtomicBool,
	propagation_stopped: AtomicBool,
	immediate_propagation_stopped: AtomicBool,
}

/// Owned native raw event with shared dispatch state.
#[derive(Debug, Clone)]
pub struct NativeEvent {
	name: EventName,
	current_target: Option<NativeEventTarget>,
	target: Option<NativeEventTarget>,
	base: BaseEventData,
	payload: NativeEventPayload,
	custom_detail: Option<serde_json::Value>,
	dispatch_state: Arc<NativeDispatchState>,
}

impl NativeEvent {
	/// Creates a native event from explicit base flags and family data.
	pub fn new(
		name: impl Into<EventName>,
		base: BaseEventData,
		payload: NativeEventPayload,
	) -> Self {
		Self {
			name: name.into(),
			current_target: None,
			target: None,
			base,
			payload,
			custom_detail: None,
			dispatch_state: Arc::new(NativeDispatchState::default()),
		}
	}

	/// Creates a synthetic known event using catalog dispatch defaults.
	pub fn for_known(name: KnownEvent, payload: NativeEventPayload) -> Self {
		let behavior = name.spec().behavior;
		Self::new(
			name,
			BaseEventData {
				bubbles: behavior.bubbles,
				cancelable: behavior.cancelable,
				composed: behavior.composed,
				time_stamp: 0.0,
				is_trusted: false,
			},
			payload,
		)
	}

	/// Returns the runtime event name.
	pub const fn name(&self) -> &EventName {
		&self.name
	}

	/// Returns the exact event name used for listener matching.
	pub fn event_type(&self) -> &str {
		self.name.as_str()
	}

	/// Returns the originating target snapshot.
	pub const fn target(&self) -> Option<&NativeEventTarget> {
		self.target.as_ref()
	}

	/// Returns the current listener target snapshot.
	pub const fn current_target(&self) -> Option<&NativeEventTarget> {
		self.current_target.as_ref()
	}

	/// Returns the captured base event data.
	pub const fn base(&self) -> &BaseEventData {
		&self.base
	}

	/// Returns the captured interface-family payload.
	pub const fn payload(&self) -> &NativeEventPayload {
		&self.payload
	}

	/// Attaches a custom event detail payload.
	#[must_use]
	pub fn with_custom_detail(mut self, detail: serde_json::Value) -> Self {
		self.custom_detail = Some(detail);
		self
	}

	/// Returns the custom event detail payload when present.
	pub const fn custom_detail(&self) -> Option<&serde_json::Value> {
		self.custom_detail.as_ref()
	}

	/// Sets the originating target snapshot.
	pub fn with_target(mut self, target: NativeEventTarget) -> Self {
		self.target = Some(target);
		self
	}

	/// Returns a per-listener snapshot with shared dispatch state.
	pub fn with_current_target(&self, current_target: NativeEventTarget) -> Self {
		let mut event = self.clone();
		event.current_target = Some(current_target);
		event
	}

	/// Prevents the default action when this event is cancelable.
	pub fn prevent_default(&self) {
		if self.base.cancelable {
			self.dispatch_state
				.default_prevented
				.store(true, Ordering::SeqCst);
		}
	}

	/// Returns whether a listener prevented the default action.
	pub fn default_prevented(&self) -> bool {
		self.dispatch_state.default_prevented.load(Ordering::SeqCst)
	}

	/// Stops traversal before the next ancestor listener.
	pub fn stop_propagation(&self) {
		self.dispatch_state
			.propagation_stopped
			.store(true, Ordering::SeqCst);
	}

	/// Stops later handlers on this target and all ancestor traversal.
	pub fn stop_immediate_propagation(&self) {
		self.dispatch_state
			.immediate_propagation_stopped
			.store(true, Ordering::SeqCst);
		self.stop_propagation();
	}

	/// Returns whether propagation has been stopped.
	pub fn propagation_stopped(&self) -> bool {
		self.dispatch_state
			.propagation_stopped
			.load(Ordering::SeqCst)
	}

	/// Returns whether later handlers on the current target must be skipped.
	pub fn immediate_propagation_stopped(&self) -> bool {
		self.dispatch_state
			.immediate_propagation_stopped
			.load(Ordering::SeqCst)
	}
}

#[cfg(test)]
mod tests {
	use std::borrow::Cow;

	use super::{BaseEventData, NativeEvent, NativeEventPayload};
	use reinhardt_event_catalog::EventName;

	#[test]
	fn native_event_distinguishes_plain_event_from_null_custom_detail() {
		let plain = NativeEvent::new(
			EventName::Custom(Cow::Borrowed("item-selected")),
			BaseEventData::default(),
			NativeEventPayload::default(),
		);
		assert_eq!(plain.custom_detail(), None);

		let custom = plain.with_custom_detail(serde_json::Value::Null);
		assert_eq!(custom.custom_detail(), Some(&serde_json::Value::Null));
	}
}
