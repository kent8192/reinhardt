//! Event System
//!
//! Provides type-safe event handling with automatic cleanup.
//!
//! ## Event Types
//!
//! This module defines enums for common event types to provide type safety
//! when working with DOM events.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::types::page::EventType;
//!
//! let event_type = EventType::Click;
//! assert_eq!(event_type.as_str(), "click");
//! ```

/// Common DOM event types
///
/// This enum represents the most commonly used DOM event types.
/// For a complete list, see: <https://developer.mozilla.org/en-US/docs/Web/Events>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
	// Mouse events
	/// Mouse button is clicked (mousedown + mouseup)
	Click,
	/// Mouse button is double-clicked
	DblClick,
	/// Mouse button is pressed down
	MouseDown,
	/// Mouse button is released
	MouseUp,
	/// Mouse enters element
	MouseEnter,
	/// Mouse leaves element
	MouseLeave,
	/// Mouse moves over element
	MouseMove,
	/// Mouse moves over element or its children
	MouseOver,
	/// Mouse leaves element or its children
	MouseOut,

	// Keyboard events
	/// Key is pressed down
	KeyDown,
	/// Key is released
	KeyUp,
	/// Key is pressed (deprecated but still used)
	KeyPress,

	// Form events
	/// Input value changes
	Input,
	/// Input loses focus and value changed
	Change,
	/// Form is submitted
	Submit,
	/// Element gains focus
	Focus,
	/// Element loses focus
	Blur,

	// Touch events
	/// Touch point is placed on the touch surface
	TouchStart,
	/// Touch point is removed from the touch surface
	TouchEnd,
	/// Touch point is moved along the touch surface
	TouchMove,
	/// Touch event is interrupted
	TouchCancel,

	// Drag events
	/// Element starts being dragged
	DragStart,
	/// Element is being dragged
	Drag,
	/// Element is dropped
	Drop,
	/// Dragged element enters drop target
	DragEnter,
	/// Dragged element leaves drop target
	DragLeave,
	/// Dragged element is over drop target
	DragOver,
	/// Drag operation ends
	DragEnd,

	// Other events
	/// Page/resource finishes loading
	Load,
	/// Page/resource loading is interrupted
	Error,
	/// Element content is scrolled
	Scroll,
	/// Window/tab is resized
	Resize,
}

impl EventType {
	/// Convert event type to string
	///
	/// Returns the event name as used in JavaScript addEventListener.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::types::page::EventType;
	///
	/// assert_eq!(EventType::Click.as_str(), "click");
	/// assert_eq!(EventType::MouseDown.as_str(), "mousedown");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			// Mouse events
			EventType::Click => "click",
			EventType::DblClick => "dblclick",
			EventType::MouseDown => "mousedown",
			EventType::MouseUp => "mouseup",
			EventType::MouseEnter => "mouseenter",
			EventType::MouseLeave => "mouseleave",
			EventType::MouseMove => "mousemove",
			EventType::MouseOver => "mouseover",
			EventType::MouseOut => "mouseout",

			// Keyboard events
			EventType::KeyDown => "keydown",
			EventType::KeyUp => "keyup",
			EventType::KeyPress => "keypress",

			// Form events
			EventType::Input => "input",
			EventType::Change => "change",
			EventType::Submit => "submit",
			EventType::Focus => "focus",
			EventType::Blur => "blur",

			// Touch events
			EventType::TouchStart => "touchstart",
			EventType::TouchEnd => "touchend",
			EventType::TouchMove => "touchmove",
			EventType::TouchCancel => "touchcancel",

			// Drag events
			EventType::DragStart => "dragstart",
			EventType::Drag => "drag",
			EventType::Drop => "drop",
			EventType::DragEnter => "dragenter",
			EventType::DragLeave => "dragleave",
			EventType::DragOver => "dragover",
			EventType::DragEnd => "dragend",

			// Other events
			EventType::Load => "load",
			EventType::Error => "error",
			EventType::Scroll => "scroll",
			EventType::Resize => "resize",
		}
	}
}

impl From<EventType> for &'static str {
	fn from(event_type: EventType) -> Self {
		event_type.as_str()
	}
}

impl AsRef<str> for EventType {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl std::str::FromStr for EventType {
	type Err = ();

	/// Parse event type from string
	///
	/// Returns `Ok(EventType)` for known event names, `Err(())` for unknown.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::types::page::EventType;
	///
	/// let click: EventType = "click".parse().unwrap();
	/// assert_eq!(click, EventType::Click);
	/// ```
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			// Mouse events
			"click" => Ok(EventType::Click),
			"dblclick" => Ok(EventType::DblClick),
			"mousedown" => Ok(EventType::MouseDown),
			"mouseup" => Ok(EventType::MouseUp),
			"mouseenter" => Ok(EventType::MouseEnter),
			"mouseleave" => Ok(EventType::MouseLeave),
			"mousemove" => Ok(EventType::MouseMove),
			"mouseover" => Ok(EventType::MouseOver),
			"mouseout" => Ok(EventType::MouseOut),

			// Keyboard events
			"keydown" => Ok(EventType::KeyDown),
			"keyup" => Ok(EventType::KeyUp),
			"keypress" => Ok(EventType::KeyPress),

			// Form events
			"input" => Ok(EventType::Input),
			"change" => Ok(EventType::Change),
			"submit" => Ok(EventType::Submit),
			"focus" => Ok(EventType::Focus),
			"blur" => Ok(EventType::Blur),

			// Touch events
			"touchstart" => Ok(EventType::TouchStart),
			"touchend" => Ok(EventType::TouchEnd),
			"touchmove" => Ok(EventType::TouchMove),
			"touchcancel" => Ok(EventType::TouchCancel),

			// Drag events
			"dragstart" => Ok(EventType::DragStart),
			"drag" => Ok(EventType::Drag),
			"drop" => Ok(EventType::Drop),
			"dragenter" => Ok(EventType::DragEnter),
			"dragleave" => Ok(EventType::DragLeave),
			"dragover" => Ok(EventType::DragOver),
			"dragend" => Ok(EventType::DragEnd),

			// Other events
			"load" => Ok(EventType::Load),
			"error" => Ok(EventType::Error),
			"scroll" => Ok(EventType::Scroll),
			"resize" => Ok(EventType::Resize),

			_ => Err(()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_event_type_as_str() {
		assert_eq!(EventType::Click.as_str(), "click");
		assert_eq!(EventType::MouseDown.as_str(), "mousedown");
		assert_eq!(EventType::Input.as_str(), "input");
		assert_eq!(EventType::KeyDown.as_str(), "keydown");
		assert_eq!(EventType::TouchStart.as_str(), "touchstart");
	}

	#[test]
	fn test_event_type_conversion() {
		let event_str: &str = EventType::Click.into();
		assert_eq!(event_str, "click");

		let event_ref: &str = EventType::MouseDown.as_ref();
		assert_eq!(event_ref, "mousedown");
	}
}
