#![cfg(target_arch = "wasm32")]

//! DOM assertion helpers for WASM testing.
//!
//! This module provides assertion methods for verifying DOM element states,
//! similar to Jest-DOM and Testing Library assertions.
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_test::wasm::assertions::ElementAssertions;
//!
//! let element = document.get_element_by_id("submit-button").unwrap();
//! element.should_be_visible();
//! element.should_be_enabled();
//! element.should_have_text("Submit");
//! element.should_have_class("btn-primary");
//! ```

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

/// Assertion error for DOM assertions.
#[derive(Debug, Clone)]
pub struct AssertionError {
	/// The assertion that failed.
	pub assertion: String,
	/// Expected value or state.
	pub expected: String,
	/// Actual value or state.
	pub actual: String,
	/// Optional element information.
	pub element_info: Option<String>,
}

impl std::fmt::Display for AssertionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"Assertion failed: {}\n  Expected: {}\n  Actual: {}",
			self.assertion, self.expected, self.actual
		)?;
		if let Some(info) = &self.element_info {
			write!(f, "\n  Element: {}", info)?;
		}
		Ok(())
	}
}

impl std::error::Error for AssertionError {}

/// Result type for assertions.
pub type AssertionResult = Result<(), AssertionError>;

/// Trait providing assertion methods for DOM elements.
///
/// This trait is implemented for `web_sys::Element` and provides
/// fluent assertion methods similar to jest-dom.
pub trait ElementAssertions {
	/// Assert that the element is visible in the viewport.
	///
	/// An element is considered visible if:
	/// - It has `display` other than `none`
	/// - It has `visibility` other than `hidden`
	/// - It has `opacity` greater than 0
	/// - It has non-zero dimensions
	fn should_be_visible(&self);

	/// Assert that the element is not visible.
	fn should_be_hidden(&self);

	/// Assert that the element is present in the document.
	fn should_be_in_document(&self);

	/// Assert that the element is not present in the document.
	fn should_not_be_in_document(&self);

	/// Assert that the element is enabled (not disabled).
	fn should_be_enabled(&self);

	/// Assert that the element is disabled.
	fn should_be_disabled(&self);

	/// Assert that the element has focus.
	fn should_have_focus(&self);

	/// Assert that the element does not have focus.
	fn should_not_have_focus(&self);

	/// Assert that the element contains the exact text content.
	fn should_have_text(&self, expected: &str);

	/// Assert that the element contains the specified text (partial match).
	fn should_contain_text(&self, expected: &str);

	/// Assert that the element has the specified attribute with any value.
	fn should_have_attribute(&self, name: &str);

	/// Assert that the element has the specified attribute with the given value.
	fn should_have_attribute_value(&self, name: &str, value: &str);

	/// Assert that the element does not have the specified attribute.
	fn should_not_have_attribute(&self, name: &str);

	/// Assert that the element has the specified CSS class.
	fn should_have_class(&self, class_name: &str);

	/// Assert that the element does not have the specified CSS class.
	fn should_not_have_class(&self, class_name: &str);

	/// Assert that the element has the specified style property value.
	fn should_have_style(&self, property: &str, value: &str);

	/// Assert that the element has the specified value (for form elements).
	fn should_have_value(&self, expected: &str);

	/// Assert that the checkbox/radio element is checked.
	fn should_be_checked(&self);

	/// Assert that the checkbox/radio element is not checked.
	fn should_not_be_checked(&self);

	/// Assert that the form element is required.
	fn should_be_required(&self);

	/// Assert that the form element is not required.
	fn should_not_be_required(&self);

	/// Assert that the form element is valid.
	fn should_be_valid(&self);

	/// Assert that the form element is invalid.
	fn should_be_invalid(&self);

	/// Assert that the element has the specified ARIA role.
	fn should_have_role(&self, role: &str);

	/// Assert that the element has the specified accessible name.
	fn should_have_accessible_name(&self, name: &str);

	/// Assert that the element has the specified accessible description.
	fn should_have_accessible_description(&self, description: &str);
}

impl ElementAssertions for Element {
	fn should_be_visible(&self) {
		assert!(
			is_visible(self),
			"Expected element to be visible, but it was hidden.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_hidden(&self) {
		assert!(
			!is_visible(self),
			"Expected element to be hidden, but it was visible.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_in_document(&self) {
		let document = get_document();
		assert!(
			document.contains(Some(self)),
			"Expected element to be in document.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_not_be_in_document(&self) {
		let document = get_document();
		assert!(
			!document.contains(Some(self)),
			"Expected element to not be in document.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_enabled(&self) {
		assert!(
			!is_disabled(self),
			"Expected element to be enabled, but it was disabled.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_disabled(&self) {
		assert!(
			is_disabled(self),
			"Expected element to be disabled, but it was enabled.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_have_focus(&self) {
		let document = get_document();
		let active = document.active_element();
		assert!(
			active.as_ref() == Some(self),
			"Expected element to have focus.\nElement: {}\nActual focused: {}",
			element_debug_info(self),
			active
				.as_ref()
				.map(element_debug_info)
				.unwrap_or_else(|| "none".to_string())
		);
	}

	fn should_not_have_focus(&self) {
		let document = get_document();
		let active = document.active_element();
		assert!(
			active.as_ref() != Some(self),
			"Expected element to not have focus, but it does.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_have_text(&self, expected: &str) {
		let actual = self.text_content().unwrap_or_default();
		let actual_trimmed = actual.trim();
		assert!(
			actual_trimmed == expected,
			"Expected element to have text '{}'.\nActual: '{}'\nElement: {}",
			expected,
			actual_trimmed,
			element_debug_info(self)
		);
	}

	fn should_contain_text(&self, expected: &str) {
		let actual = self.text_content().unwrap_or_default();
		assert!(
			actual.contains(expected),
			"Expected element to contain text '{}'.\nActual: '{}'\nElement: {}",
			expected,
			actual.trim(),
			element_debug_info(self)
		);
	}

	fn should_have_attribute(&self, name: &str) {
		assert!(
			self.has_attribute(name),
			"Expected element to have attribute '{}'.\nElement: {}",
			name,
			element_debug_info(self)
		);
	}

	fn should_have_attribute_value(&self, name: &str, value: &str) {
		let actual = self.get_attribute(name);
		assert!(
			actual.as_deref() == Some(value),
			"Expected element attribute '{}' to be '{}'.\nActual: {:?}\nElement: {}",
			name,
			value,
			actual,
			element_debug_info(self)
		);
	}

	fn should_not_have_attribute(&self, name: &str) {
		assert!(
			!self.has_attribute(name),
			"Expected element to not have attribute '{}'.\nElement: {}",
			name,
			element_debug_info(self)
		);
	}

	fn should_have_class(&self, class_name: &str) {
		let class_list = self.class_list();
		assert!(
			class_list.contains(class_name),
			"Expected element to have class '{}'.\nActual classes: {}\nElement: {}",
			class_name,
			self.class_name(),
			element_debug_info(self)
		);
	}

	fn should_not_have_class(&self, class_name: &str) {
		let class_list = self.class_list();
		assert!(
			!class_list.contains(class_name),
			"Expected element to not have class '{}'.\nActual classes: {}\nElement: {}",
			class_name,
			self.class_name(),
			element_debug_info(self)
		);
	}

	fn should_have_style(&self, property: &str, value: &str) {
		let actual = get_computed_style(self, property);
		assert!(
			actual.as_deref() == Some(value),
			"Expected element style '{}' to be '{}'.\nActual: {:?}\nElement: {}",
			property,
			value,
			actual,
			element_debug_info(self)
		);
	}

	fn should_have_value(&self, expected: &str) {
		let actual = get_element_value(self);
		assert!(
			actual.as_deref() == Some(expected),
			"Expected element value to be '{}'.\nActual: {:?}\nElement: {}",
			expected,
			actual,
			element_debug_info(self)
		);
	}

	fn should_be_checked(&self) {
		assert!(
			is_checked(self),
			"Expected element to be checked.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_not_be_checked(&self) {
		assert!(
			!is_checked(self),
			"Expected element to not be checked.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_required(&self) {
		assert!(
			self.has_attribute("required")
				|| self.get_attribute("aria-required").as_deref() == Some("true"),
			"Expected element to be required.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_not_be_required(&self) {
		assert!(
			!self.has_attribute("required")
				&& self.get_attribute("aria-required").as_deref() != Some("true"),
			"Expected element to not be required.\nElement: {}",
			element_debug_info(self)
		);
	}

	fn should_be_valid(&self) {
		if let Some(input) = self.dyn_ref::<HtmlInputElement>() {
			assert!(
				input.check_validity(),
				"Expected element to be valid.\nElement: {}",
				element_debug_info(self)
			);
		} else {
			// For non-input elements, check aria-invalid
			let aria_invalid = self.get_attribute("aria-invalid");
			assert!(
				aria_invalid.as_deref() != Some("true"),
				"Expected element to be valid (aria-invalid should not be 'true').\nElement: {}",
				element_debug_info(self)
			);
		}
	}

	fn should_be_invalid(&self) {
		if let Some(input) = self.dyn_ref::<HtmlInputElement>() {
			assert!(
				!input.check_validity(),
				"Expected element to be invalid.\nElement: {}",
				element_debug_info(self)
			);
		} else {
			let aria_invalid = self.get_attribute("aria-invalid");
			assert!(
				aria_invalid.as_deref() == Some("true"),
				"Expected element to be invalid (aria-invalid should be 'true').\nElement: {}",
				element_debug_info(self)
			);
		}
	}

	fn should_have_role(&self, role: &str) {
		let actual_role = get_element_role(self);
		assert!(
			actual_role.as_deref() == Some(role),
			"Expected element to have role '{}'.\nActual role: {:?}\nElement: {}",
			role,
			actual_role,
			element_debug_info(self)
		);
	}

	fn should_have_accessible_name(&self, name: &str) {
		let actual_name = get_accessible_name(self);
		assert!(
			actual_name.as_deref() == Some(name),
			"Expected element to have accessible name '{}'.\nActual: {:?}\nElement: {}",
			name,
			actual_name,
			element_debug_info(self)
		);
	}

	fn should_have_accessible_description(&self, description: &str) {
		let actual = self.get_attribute("aria-describedby").and_then(|id| {
			get_document()
				.get_element_by_id(&id)
				.and_then(|el| el.text_content())
		});

		assert!(
			actual.as_deref() == Some(description),
			"Expected element to have accessible description '{}'.\nActual: {:?}\nElement: {}",
			description,
			actual,
			element_debug_info(self)
		);
	}
}

// Helper functions

fn get_window() -> Window {
	web_sys::window().expect("Window should be available")
}

fn get_document() -> Document {
	get_window()
		.document()
		.expect("Document should be available")
}

fn element_debug_info(element: &Element) -> String {
	let tag = element.tag_name().to_lowercase();
	let id = element
		.id()
		.is_empty()
		.then_some(String::new())
		.unwrap_or_else(|| format!("#{}", element.id()));
	let classes = element.class_name();
	let class_str = if classes.is_empty() {
		String::new()
	} else {
		format!(
			".{}",
			classes.split_whitespace().collect::<Vec<_>>().join(".")
		)
	};

	format!("<{}{}{}>", tag, id, class_str)
}

fn is_visible(element: &Element) -> bool {
	// Check if element is in document
	if !get_document().contains(Some(element)) {
		return false;
	}

	// Check for HtmlElement-specific visibility
	if let Some(html_element) = element.dyn_ref::<HtmlElement>() {
		// Check offsetParent (null means element is hidden via display:none or not in document)
		if html_element.offset_parent().is_none() {
			// Exception for fixed/sticky positioned elements
			if let Some(style) = get_window().get_computed_style(element).ok().flatten() {
				let position = style.get_property_value("position").unwrap_or_default();
				if position != "fixed" && position != "sticky" {
					return false;
				}
			} else {
				return false;
			}
		}
	}

	// Check computed styles
	if let Some(style) = get_window().get_computed_style(element).ok().flatten() {
		// Check display
		let display = style.get_property_value("display").unwrap_or_default();
		if display == "none" {
			return false;
		}

		// Check visibility
		let visibility = style.get_property_value("visibility").unwrap_or_default();
		if visibility == "hidden" || visibility == "collapse" {
			return false;
		}

		// Check opacity
		if let Ok(opacity) = style
			.get_property_value("opacity")
			.unwrap_or_else(|_| "1".to_string())
			.parse::<f64>()
		{
			if opacity == 0.0 {
				return false;
			}
		}
	}

	// Check dimensions
	let rect = element.get_bounding_client_rect();
	if rect.width() == 0.0 && rect.height() == 0.0 {
		return false;
	}

	true
}

fn is_disabled(element: &Element) -> bool {
	// Check disabled attribute
	if element.has_attribute("disabled") {
		return true;
	}

	// Check aria-disabled
	if element.get_attribute("aria-disabled").as_deref() == Some("true") {
		return true;
	}

	// For form elements, check the disabled property
	if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
		return input.disabled();
	}

	if let Some(button) = element.dyn_ref::<web_sys::HtmlButtonElement>() {
		return button.disabled();
	}

	if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
		return select.disabled();
	}

	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		return textarea.disabled();
	}

	false
}

fn is_checked(element: &Element) -> bool {
	if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
		return input.checked();
	}

	// Check aria-checked for custom checkboxes
	element.get_attribute("aria-checked").as_deref() == Some("true")
}

fn get_computed_style(element: &Element, property: &str) -> Option<String> {
	get_window()
		.get_computed_style(element)
		.ok()
		.flatten()
		.and_then(|style| style.get_property_value(property).ok())
		.filter(|v| !v.is_empty())
}

fn get_element_value(element: &Element) -> Option<String> {
	if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
		return Some(input.value());
	}

	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		return Some(textarea.value());
	}

	if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
		return Some(select.value());
	}

	None
}

fn get_element_role(element: &Element) -> Option<String> {
	// First check explicit role attribute
	if let Some(role) = element.get_attribute("role") {
		return Some(role);
	}

	// Otherwise, derive implicit role from element type
	let tag = element.tag_name().to_lowercase();
	let implicit_role = match tag.as_str() {
		"button" => Some("button"),
		"a" if element.has_attribute("href") => Some("link"),
		"input" => {
			let input_type = element
				.get_attribute("type")
				.unwrap_or_else(|| "text".to_string())
				.to_lowercase();
			match input_type.as_str() {
				"button" | "submit" | "reset" | "image" => Some("button"),
				"checkbox" => Some("checkbox"),
				"radio" => Some("radio"),
				"text" | "email" | "tel" | "url" | "search" | "password" => Some("textbox"),
				"number" => Some("spinbutton"),
				"range" => Some("slider"),
				_ => None,
			}
		}
		"select" => Some(if element.has_attribute("multiple") {
			"listbox"
		} else {
			"combobox"
		}),
		"textarea" => Some("textbox"),
		"img" => Some("img"),
		"nav" => Some("navigation"),
		"main" => Some("main"),
		"header" => Some("banner"),
		"footer" => Some("contentinfo"),
		"aside" => Some("complementary"),
		"article" => Some("article"),
		"section" => Some("region"),
		"form" => Some("form"),
		"table" => Some("table"),
		"ul" | "ol" => Some("list"),
		"li" => Some("listitem"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some("heading"),
		"dialog" => Some("dialog"),
		"progress" => Some("progressbar"),
		"menu" => Some("menu"),
		"menuitem" => Some("menuitem"),
		_ => None,
	};

	implicit_role.map(String::from)
}

fn get_accessible_name(element: &Element) -> Option<String> {
	// Check aria-label first
	if let Some(label) = element.get_attribute("aria-label") {
		if !label.is_empty() {
			return Some(label);
		}
	}

	// Check aria-labelledby
	if let Some(labelledby) = element.get_attribute("aria-labelledby") {
		let document = get_document();
		let names: Vec<String> = labelledby
			.split_whitespace()
			.filter_map(|id| {
				document
					.get_element_by_id(id)
					.and_then(|el| el.text_content())
			})
			.collect();

		if !names.is_empty() {
			return Some(names.join(" "));
		}
	}

	// For inputs, check associated label
	if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
		// Check for label with matching 'for' attribute
		let id = input.id();
		// Fixes #878: Escape CSS selector value to prevent injection
		if !id.is_empty() {
			let document = get_document();
			if let Ok(Some(label)) = document.query_selector(&format!(
				"label[for='{}']",
				super::query::escape_css_selector(&id)
			)) {
				if let Some(text) = label.text_content() {
					return Some(text.trim().to_string());
				}
			}
		}

		// Check for wrapping label
		let mut parent = element.parent_element();
		while let Some(p) = parent {
			if p.tag_name().to_lowercase() == "label" {
				if let Some(text) = p.text_content() {
					// Remove the input's own text if any
					let input_text = input.value();
					let label_text = text.replace(&input_text, "").trim().to_string();
					if !label_text.is_empty() {
						return Some(label_text);
					}
				}
			}
			parent = p.parent_element();
		}
	}

	// Check title attribute
	if let Some(title) = element.get_attribute("title") {
		if !title.is_empty() {
			return Some(title);
		}
	}

	// For images, check alt text
	if element.tag_name().to_lowercase() == "img" {
		if let Some(alt) = element.get_attribute("alt") {
			return Some(alt);
		}
	}

	// For buttons/links, use text content
	let tag = element.tag_name().to_lowercase();
	if tag == "button" || tag == "a" {
		if let Some(text) = element.text_content() {
			let trimmed = text.trim();
			if !trimmed.is_empty() {
				return Some(trimmed.to_string());
			}
		}
	}

	None
}

/// Additional assertion functions that work with multiple elements or special cases.
pub mod assert {
	use super::*;

	/// Assert that all elements in the list are visible.
	pub fn all_visible(elements: &[Element]) {
		for (i, element) in elements.iter().enumerate() {
			assert!(
				is_visible(element),
				"Expected element {} to be visible.\nElement: {}",
				i,
				element_debug_info(element)
			);
		}
	}

	/// Assert that all elements in the list are hidden.
	pub fn all_hidden(elements: &[Element]) {
		for (i, element) in elements.iter().enumerate() {
			assert!(
				!is_visible(element),
				"Expected element {} to be hidden.\nElement: {}",
				i,
				element_debug_info(element)
			);
		}
	}

	/// Assert the number of elements matching a selector.
	pub fn element_count(selector: &str, expected: usize) {
		let document = get_document();
		let elements = document.query_selector_all(selector).unwrap();
		assert!(
			elements.length() as usize == expected,
			"Expected {} elements matching '{}', found {}",
			expected,
			selector,
			elements.length()
		);
	}

	/// Assert that no elements match the selector.
	pub fn no_elements(selector: &str) {
		element_count(selector, 0);
	}

	/// Assert that the document title matches.
	pub fn document_title(expected: &str) {
		let actual = get_document().title();
		assert!(
			actual == expected,
			"Expected document title to be '{}'.\nActual: '{}'",
			expected,
			actual
		);
	}

	/// Assert that the document contains the specified text anywhere.
	pub fn document_contains_text(expected: &str) {
		let body_text = get_document()
			.body()
			.and_then(|b| b.text_content())
			.unwrap_or_default();
		assert!(
			body_text.contains(expected),
			"Expected document to contain text '{}'",
			expected
		);
	}

	/// Assert that the current URL matches.
	pub fn current_url(expected: &str) {
		let location = get_window().location();
		let actual = location.href().unwrap_or_default();
		assert!(
			actual == expected,
			"Expected URL to be '{}'.\nActual: '{}'",
			expected,
			actual
		);
	}

	/// Assert that the current URL contains the specified substring.
	pub fn url_contains(expected: &str) {
		let location = get_window().location();
		let actual = location.href().unwrap_or_default();
		assert!(
			actual.contains(expected),
			"Expected URL to contain '{}'.\nActual: '{}'",
			expected,
			actual
		);
	}

	/// Assert that the current pathname matches.
	pub fn current_pathname(expected: &str) {
		let location = get_window().location();
		let actual = location.pathname().unwrap_or_default();
		assert!(
			actual == expected,
			"Expected pathname to be '{}'.\nActual: '{}'",
			expected,
			actual
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: WASM tests should use wasm_bindgen_test
	// These tests are for compile-time verification only

	#[test]
	fn test_assertion_error_display() {
		let error = AssertionError {
			assertion: "should_be_visible".to_string(),
			expected: "visible".to_string(),
			actual: "hidden".to_string(),
			element_info: Some("<div#test>".to_string()),
		};

		let display = error.to_string();
		assert!(display.contains("should_be_visible"));
		assert!(display.contains("visible"));
		assert!(display.contains("hidden"));
		assert!(display.contains("<div#test>"));
	}
}
