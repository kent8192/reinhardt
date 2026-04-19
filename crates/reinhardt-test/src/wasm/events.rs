#![cfg(wasm)]

//! Event Simulation for WASM Frontend Testing
//!
//! This module provides utilities for simulating user interactions in WASM tests.
//! It includes both high-level user event simulation and low-level event firing.
//!
//! # User Events vs Fire Events
//!
//! - **UserEvent**: High-level API that simulates realistic user interactions
//!   (e.g., clicking, typing with proper event sequence)
//! - **fire_event**: Low-level API for dispatching specific DOM events
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::wasm::events::UserEvent;
//!
//! // Click a button
//! let button = screen.get_by_role("button").get();
//! UserEvent::click(&button);
//!
//! // Type into an input
//! let input = screen.get_by_label_text("Email").get();
//! UserEvent::type_text(&input.dyn_into().unwrap(), "test@example.com");
//!
//! // Submit a form
//! let form = screen.get_by_role("form").get();
//! UserEvent::submit(&form.dyn_into().unwrap());
//! ```

use wasm_bindgen::JsCast;
use web_sys::{
	CustomEvent, CustomEventInit, Element, Event, EventInit, FocusEvent, FocusEventInit,
	HtmlButtonElement, HtmlFormElement, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
	InputEvent, InputEventInit, KeyboardEvent, KeyboardEventInit, MouseEvent, MouseEventInit,
};

/// High-level user event simulation.
///
/// This struct provides static methods for simulating common user interactions.
/// The events are fired in a sequence that mimics real user behavior.
pub struct UserEvent;

impl UserEvent {
	/// Simulate a mouse click on an element.
	///
	/// Fires: mousedown -> mouseup -> click
	///
	/// # Arguments
	///
	/// * `element` - The element to click
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let button = screen.get_by_role("button").get();
	/// UserEvent::click(&button);
	/// ```
	pub fn click(element: &Element) {
		fire_event::focus(element);
		fire_event::mouse_down(element);
		fire_event::mouse_up(element);
		fire_event::click(element);
	}

	/// Simulate a double-click on an element.
	///
	/// Fires: mousedown -> mouseup -> click -> mousedown -> mouseup -> click -> dblclick
	pub fn dbl_click(element: &Element) {
		Self::click(element);
		Self::click(element);
		fire_event::dbl_click(element);
	}

	/// Simulate a right-click (context menu) on an element.
	///
	/// Fires: mousedown -> contextmenu -> mouseup
	pub fn right_click(element: &Element) {
		fire_event::mouse_down_button(element, 2);
		fire_event::context_menu(element);
		fire_event::mouse_up_button(element, 2);
	}

	/// Type text into an input element.
	///
	/// This clears any existing value and types the new text, firing
	/// appropriate input and change events.
	///
	/// # Arguments
	///
	/// * `element` - The input element
	/// * `text` - The text to type
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let input: HtmlInputElement = screen.get_by_label_text("Name")
	///     .get()
	///     .dyn_into()
	///     .unwrap();
	/// UserEvent::type_text(&input, "John Doe");
	/// ```
	pub fn type_text(element: &HtmlInputElement, text: &str) {
		// Focus the element
		let _ = element.focus();
		fire_event::focus_element(element.unchecked_ref());

		// Set the value
		let old_value = element.value();
		element.set_value(text);

		// Fire input event
		fire_event::input_element(element.unchecked_ref(), text);

		// Fire change event if value changed
		if old_value != text {
			fire_event::change_element(element.unchecked_ref());
		}
	}

	/// Type text into a textarea element.
	pub fn type_textarea(element: &HtmlTextAreaElement, text: &str) {
		let _ = element.focus();
		fire_event::focus_element(element.unchecked_ref());

		let old_value = element.value();
		element.set_value(text);

		fire_event::input_element(element.unchecked_ref(), text);

		if old_value != text {
			fire_event::change_element(element.unchecked_ref());
		}
	}

	/// Clear the value of an input element.
	///
	/// # Arguments
	///
	/// * `element` - The input element to clear
	pub fn clear(element: &HtmlInputElement) {
		let _ = element.focus();
		element.set_value("");
		fire_event::input_element(element.unchecked_ref(), "");
		fire_event::change_element(element.unchecked_ref());
	}

	/// Clear a textarea element.
	pub fn clear_textarea(element: &HtmlTextAreaElement) {
		let _ = element.focus();
		element.set_value("");
		fire_event::input_element(element.unchecked_ref(), "");
		fire_event::change_element(element.unchecked_ref());
	}

	/// Simulate pressing a key.
	///
	/// Fires: keydown -> keypress -> keyup
	///
	/// # Arguments
	///
	/// * `element` - The element receiving the key event
	/// * `key` - The key value (e.g., "Enter", "Escape", "a")
	pub fn keyboard_press(element: &Element, key: &str) {
		fire_event::key_down(element, key, KeyModifiers::default());
		fire_event::key_press(element, key, KeyModifiers::default());
		fire_event::key_up(element, key, KeyModifiers::default());
	}

	/// Simulate pressing a key with modifiers.
	pub fn keyboard_press_with_modifiers(element: &Element, key: &str, modifiers: KeyModifiers) {
		fire_event::key_down(element, key, modifiers);
		fire_event::key_press(element, key, modifiers);
		fire_event::key_up(element, key, modifiers);
	}

	/// Focus an element.
	///
	/// # Arguments
	///
	/// * `element` - The element to focus
	pub fn focus(element: &Element) {
		if let Some(html_element) = element.dyn_ref::<web_sys::HtmlElement>() {
			let _ = html_element.focus();
		}
		fire_event::focus(element);
	}

	/// Remove focus from an element.
	///
	/// # Arguments
	///
	/// * `element` - The element to blur
	pub fn blur(element: &Element) {
		if let Some(html_element) = element.dyn_ref::<web_sys::HtmlElement>() {
			html_element.blur();
		}
		fire_event::blur(element);
	}

	/// Simulate hovering over an element.
	///
	/// Fires: mouseenter -> mouseover
	pub fn hover(element: &Element) {
		fire_event::mouse_enter(element);
		fire_event::mouse_over(element);
	}

	/// Simulate moving the mouse away from an element.
	///
	/// Fires: mouseleave -> mouseout
	pub fn unhover(element: &Element) {
		fire_event::mouse_leave(element);
		fire_event::mouse_out(element);
	}

	/// Select an option in a select element.
	///
	/// # Arguments
	///
	/// * `element` - The select element
	/// * `option` - Which option to select
	pub fn select_option(element: &HtmlSelectElement, option: SelectOption) {
		let _ = element.focus();

		match option {
			SelectOption::ByValue(value) => {
				element.set_value(value);
			}
			SelectOption::ByText(text) => {
				let options = element.options();
				for i in 0..options.length() {
					if let Some(opt) = options.get_with_index(i) {
						if let Some(html_opt) = opt.dyn_ref::<web_sys::HtmlOptionElement>() {
							if html_opt.text() == text {
								element.set_selected_index(i as i32);
								break;
							}
						}
					}
				}
			}
			SelectOption::ByIndex(index) => {
				element.set_selected_index(index as i32);
			}
		}

		fire_event::change_element(element.unchecked_ref());
	}

	/// Check a checkbox or radio button.
	///
	/// # Arguments
	///
	/// * `element` - The checkbox or radio input
	pub fn check(element: &HtmlInputElement) {
		if !element.checked() {
			element.set_checked(true);
			Self::click(element.unchecked_ref());
			fire_event::change_element(element.unchecked_ref());
		}
	}

	/// Uncheck a checkbox.
	///
	/// # Arguments
	///
	/// * `element` - The checkbox input
	pub fn uncheck(element: &HtmlInputElement) {
		if element.checked() {
			element.set_checked(false);
			Self::click(element.unchecked_ref());
			fire_event::change_element(element.unchecked_ref());
		}
	}

	/// Toggle a checkbox.
	pub fn toggle(element: &HtmlInputElement) {
		if element.checked() {
			Self::uncheck(element);
		} else {
			Self::check(element);
		}
	}

	/// Submit a form.
	///
	/// # Arguments
	///
	/// * `form` - The form element to submit
	pub fn submit(form: &HtmlFormElement) {
		fire_event::submit(form);
	}

	/// Click a button element (convenience method).
	pub fn click_button(button: &HtmlButtonElement) {
		Self::click(button.unchecked_ref());
	}
}

/// Options for selecting from a dropdown.
#[derive(Debug, Clone)]
pub enum SelectOption<'a> {
	/// Select by the option's value attribute
	ByValue(&'a str),
	/// Select by the option's visible text
	ByText(&'a str),
	/// Select by the option's index (0-based)
	ByIndex(usize),
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
	/// Ctrl key is pressed
	pub ctrl: bool,
	/// Shift key is pressed
	pub shift: bool,
	/// Alt key is pressed
	pub alt: bool,
	/// Meta (Command/Windows) key is pressed
	pub meta: bool,
}

impl KeyModifiers {
	/// Create modifiers with Ctrl pressed.
	pub fn ctrl() -> Self {
		Self {
			ctrl: true,
			..Default::default()
		}
	}

	/// Create modifiers with Shift pressed.
	pub fn shift() -> Self {
		Self {
			shift: true,
			..Default::default()
		}
	}

	/// Create modifiers with Alt pressed.
	pub fn alt() -> Self {
		Self {
			alt: true,
			..Default::default()
		}
	}

	/// Create modifiers with Meta pressed.
	pub fn meta() -> Self {
		Self {
			meta: true,
			..Default::default()
		}
	}

	/// Create modifiers with Ctrl+Shift pressed.
	pub fn ctrl_shift() -> Self {
		Self {
			ctrl: true,
			shift: true,
			..Default::default()
		}
	}
}

/// Low-level event firing functions.
///
/// These functions dispatch specific DOM events to elements.
/// Use `UserEvent` for higher-level simulation of user interactions.
pub mod fire_event {
	use super::*;

	/// Dispatch a custom event to an element.
	pub fn dispatch(element: &Element, event: &Event) {
		let _ = element.dispatch_event(event);
	}

	/// Fire a click event.
	pub fn click(element: &Element) {
		let event = create_mouse_event("click", true, true);
		dispatch(element, &event);
	}

	/// Fire a double-click event.
	pub fn dbl_click(element: &Element) {
		let event = create_mouse_event("dblclick", true, true);
		dispatch(element, &event);
	}

	/// Fire a context menu event.
	pub fn context_menu(element: &Element) {
		let event = create_mouse_event("contextmenu", true, true);
		dispatch(element, &event);
	}

	/// Fire a mousedown event.
	pub fn mouse_down(element: &Element) {
		mouse_down_button(element, 0);
	}

	/// Fire a mousedown event with a specific button.
	pub fn mouse_down_button(element: &Element, button: i16) {
		let mut init = MouseEventInit::new();
		init.bubbles(true);
		init.cancelable(true);
		init.button(button);
		let event = MouseEvent::new_with_mouse_event_init_dict("mousedown", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire a mouseup event.
	pub fn mouse_up(element: &Element) {
		mouse_up_button(element, 0);
	}

	/// Fire a mouseup event with a specific button.
	pub fn mouse_up_button(element: &Element, button: i16) {
		let mut init = MouseEventInit::new();
		init.bubbles(true);
		init.cancelable(true);
		init.button(button);
		let event = MouseEvent::new_with_mouse_event_init_dict("mouseup", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire a mouseenter event.
	pub fn mouse_enter(element: &Element) {
		let event = create_mouse_event("mouseenter", false, false);
		dispatch(element, &event);
	}

	/// Fire a mouseleave event.
	pub fn mouse_leave(element: &Element) {
		let event = create_mouse_event("mouseleave", false, false);
		dispatch(element, &event);
	}

	/// Fire a mouseover event.
	pub fn mouse_over(element: &Element) {
		let event = create_mouse_event("mouseover", true, true);
		dispatch(element, &event);
	}

	/// Fire a mouseout event.
	pub fn mouse_out(element: &Element) {
		let event = create_mouse_event("mouseout", true, true);
		dispatch(element, &event);
	}

	/// Fire a focus event.
	pub fn focus(element: &Element) {
		let mut init = FocusEventInit::new();
		init.bubbles(false);
		init.cancelable(false);
		let event = FocusEvent::new_with_focus_event_init_dict("focus", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire a focus event on an HTML element.
	pub fn focus_element(element: &Element) {
		focus(element);
	}

	/// Fire a blur event.
	pub fn blur(element: &Element) {
		let mut init = FocusEventInit::new();
		init.bubbles(false);
		init.cancelable(false);
		let event = FocusEvent::new_with_focus_event_init_dict("blur", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire an input event.
	pub fn input(element: &Element, value: &str) {
		let mut init = InputEventInit::new();
		init.bubbles(true);
		init.cancelable(false);
		init.data(Some(value));
		init.input_type("insertText");
		let event = InputEvent::new_with_event_init_dict("input", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire an input event on an element.
	pub fn input_element(element: &Element, value: &str) {
		input(element, value);
	}

	/// Fire a change event.
	pub fn change(element: &Element) {
		let mut init = EventInit::new();
		init.bubbles(true);
		init.cancelable(false);
		let event = Event::new_with_event_init_dict("change", &init).unwrap();
		dispatch(element, &event);
	}

	/// Fire a change event on an element.
	pub fn change_element(element: &Element) {
		change(element);
	}

	/// Fire a keydown event.
	pub fn key_down(element: &Element, key: &str, modifiers: KeyModifiers) {
		let event = create_keyboard_event("keydown", key, modifiers);
		dispatch(element, &event);
	}

	/// Fire a keyup event.
	pub fn key_up(element: &Element, key: &str, modifiers: KeyModifiers) {
		let event = create_keyboard_event("keyup", key, modifiers);
		dispatch(element, &event);
	}

	/// Fire a keypress event.
	pub fn key_press(element: &Element, key: &str, modifiers: KeyModifiers) {
		let event = create_keyboard_event("keypress", key, modifiers);
		dispatch(element, &event);
	}

	/// Fire a submit event on a form.
	pub fn submit(form: &HtmlFormElement) {
		let mut init = EventInit::new();
		init.bubbles(true);
		init.cancelable(true);
		let event = Event::new_with_event_init_dict("submit", &init).unwrap();
		let _ = form.dispatch_event(&event);
	}

	/// Fire a custom event.
	pub fn custom(element: &Element, event_type: &str, detail: Option<&wasm_bindgen::JsValue>) {
		let mut init = CustomEventInit::new();
		init.bubbles(true);
		init.cancelable(true);
		if let Some(d) = detail {
			init.detail(d);
		}
		let event = CustomEvent::new_with_event_init_dict(event_type, &init).unwrap();
		dispatch(element, &event);
	}

	// Helper functions

	fn create_mouse_event(event_type: &str, bubbles: bool, cancelable: bool) -> MouseEvent {
		let mut init = MouseEventInit::new();
		init.bubbles(bubbles);
		init.cancelable(cancelable);
		init.button(0);
		MouseEvent::new_with_mouse_event_init_dict(event_type, &init).unwrap()
	}

	fn create_keyboard_event(
		event_type: &str,
		key: &str,
		modifiers: KeyModifiers,
	) -> KeyboardEvent {
		let mut init = KeyboardEventInit::new();
		init.bubbles(true);
		init.cancelable(true);
		init.key(key);
		init.code(&key_to_code(key));
		init.ctrl_key(modifiers.ctrl);
		init.shift_key(modifiers.shift);
		init.alt_key(modifiers.alt);
		init.meta_key(modifiers.meta);
		KeyboardEvent::new_with_keyboard_event_init_dict(event_type, &init).unwrap()
	}

	pub(super) fn key_to_code(key: &str) -> String {
		match key {
			"Enter" => "Enter".to_string(),
			"Escape" => "Escape".to_string(),
			"Tab" => "Tab".to_string(),
			"Backspace" => "Backspace".to_string(),
			"Delete" => "Delete".to_string(),
			"ArrowUp" => "ArrowUp".to_string(),
			"ArrowDown" => "ArrowDown".to_string(),
			"ArrowLeft" => "ArrowLeft".to_string(),
			"ArrowRight" => "ArrowRight".to_string(),
			"Home" => "Home".to_string(),
			"End" => "End".to_string(),
			"PageUp" => "PageUp".to_string(),
			"PageDown" => "PageDown".to_string(),
			" " => "Space".to_string(),
			k if k.len() == 1 => {
				let c = k.chars().next().unwrap();
				if c.is_ascii_alphabetic() {
					format!("Key{}", c.to_ascii_uppercase())
				} else if c.is_ascii_digit() {
					format!("Digit{}", c)
				} else {
					k.to_string()
				}
			}
			_ => key.to_string(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn key_modifiers_default_has_all_false() {
		// Arrange
		let mods = KeyModifiers::default();

		// Act & Assert
		assert!(!mods.ctrl);
		assert!(!mods.shift);
		assert!(!mods.alt);
		assert!(!mods.meta);
	}

	#[rstest]
	fn key_modifiers_ctrl_sets_only_ctrl() {
		// Arrange & Act
		let mods = KeyModifiers::ctrl();

		// Assert
		assert!(mods.ctrl);
		assert!(!mods.shift);
		assert!(!mods.alt);
		assert!(!mods.meta);
	}

	#[rstest]
	fn key_modifiers_shift_sets_only_shift() {
		// Arrange & Act
		let mods = KeyModifiers::shift();

		// Assert
		assert!(!mods.ctrl);
		assert!(mods.shift);
		assert!(!mods.alt);
		assert!(!mods.meta);
	}

	#[rstest]
	fn key_modifiers_alt_sets_only_alt() {
		// Arrange & Act
		let mods = KeyModifiers::alt();

		// Assert
		assert!(!mods.ctrl);
		assert!(!mods.shift);
		assert!(mods.alt);
		assert!(!mods.meta);
	}

	#[rstest]
	fn key_modifiers_meta_sets_only_meta() {
		// Arrange & Act
		let mods = KeyModifiers::meta();

		// Assert
		assert!(!mods.ctrl);
		assert!(!mods.shift);
		assert!(!mods.alt);
		assert!(mods.meta);
	}

	#[rstest]
	fn key_modifiers_ctrl_shift_sets_both() {
		// Arrange & Act
		let mods = KeyModifiers::ctrl_shift();

		// Assert
		assert!(mods.ctrl);
		assert!(mods.shift);
		assert!(!mods.alt);
		assert!(!mods.meta);
	}

	#[rstest]
	fn key_modifiers_copy_trait_preserves_values() {
		// Arrange
		let original = KeyModifiers::ctrl();

		// Act
		let copied = original;
		// original is still usable because KeyModifiers implements Copy
		let _also_original = original;

		// Assert
		assert!(copied.ctrl);
		assert!(original.ctrl);
	}

	#[rstest]
	fn key_modifiers_debug_format_contains_field_names() {
		// Arrange
		let mods = KeyModifiers {
			ctrl: true,
			shift: false,
			alt: true,
			meta: false,
		};

		// Act
		let debug_str = format!("{:?}", mods);

		// Assert
		assert!(debug_str.contains("ctrl: true"));
		assert!(debug_str.contains("shift: false"));
		assert!(debug_str.contains("alt: true"));
		assert!(debug_str.contains("meta: false"));
	}

	#[rstest]
	#[case("Enter", "Enter")]
	#[case("Escape", "Escape")]
	#[case("Tab", "Tab")]
	#[case("Backspace", "Backspace")]
	#[case(" ", "Space")]
	#[case("a", "KeyA")]
	#[case("A", "KeyA")]
	#[case("5", "Digit5")]
	#[case("ArrowUp", "ArrowUp")]
	fn key_to_code_maps_key_to_expected_code(#[case] key: &str, #[case] expected: &str) {
		// Arrange & Act
		let code = fire_event::key_to_code(key);

		// Assert
		assert_eq!(code, expected);
	}

	#[rstest]
	fn key_to_code_passes_through_unknown_multichar_string() {
		// Arrange
		let unknown = "F13";

		// Act
		let code = fire_event::key_to_code(unknown);

		// Assert
		assert_eq!(code, "F13");
	}

	#[rstest]
	fn select_option_variants_can_be_created() {
		// Arrange & Act
		let by_value = SelectOption::ByValue("test");
		let by_text = SelectOption::ByText("Test Option");
		let by_index = SelectOption::ByIndex(0);

		// Assert
		assert!(matches!(by_value, SelectOption::ByValue("test")));
		assert!(matches!(by_text, SelectOption::ByText("Test Option")));
		assert!(matches!(by_index, SelectOption::ByIndex(0)));
	}

	#[rstest]
	fn select_option_debug_format_shows_variant_and_value() {
		// Arrange
		let option = SelectOption::ByValue("option1");

		// Act
		let debug_str = format!("{:?}", option);

		// Assert
		assert!(debug_str.contains("ByValue"));
		assert!(debug_str.contains("option1"));
	}

	#[rstest]
	fn select_option_clone_produces_equal_variant() {
		// Arrange
		let original = SelectOption::ByIndex(42);

		// Act
		let cloned = original.clone();

		// Assert
		assert!(matches!(cloned, SelectOption::ByIndex(42)));
	}
}
