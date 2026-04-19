#![cfg(not(target_arch = "wasm32"))]
//! Integration tests for DOM Abstraction
//!
//! These tests verify the DOM abstraction layer:
//! 1. DOM wrapper functional
//! 2. Reactive attributes update on Signal changes
//! 3. Event listeners clean up automatically
//! 4. Zero memory leaks

use reinhardt_pages::{dom::*, reactive::Signal};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Success Criterion 1: DOM wrapper functional
#[wasm_bindgen_test]
fn test_document_create_element() {
	let doc = document();
	let div = doc.create_element("div").unwrap();

	// Verify element was created
	assert_eq!(div.as_web_sys().tag_name().to_lowercase(), "div");
}

/// Success Criterion 1: Element attribute manipulation
#[wasm_bindgen_test]
fn test_element_attributes() {
	let doc = document();
	let div = doc.create_element("div").unwrap();

	// Set attribute
	div.set_attribute("id", "test-div").unwrap();
	assert_eq!(div.get_attribute("id"), Some("test-div".to_string()));

	// Update attribute
	div.set_attribute("id", "updated-div").unwrap();
	assert_eq!(div.get_attribute("id"), Some("updated-div".to_string()));

	// Remove attribute
	div.remove_attribute("id").unwrap();
	assert_eq!(div.get_attribute("id"), None);
}

/// Success Criterion 1: Text content manipulation
#[wasm_bindgen_test]
fn test_element_text_content() {
	let doc = document();
	let p = doc.create_element("p").unwrap();

	p.set_text_content("Hello, World!");
	assert_eq!(p.text_content(), Some("Hello, World!".to_string()));

	p.set_text_content("Updated text");
	assert_eq!(p.text_content(), Some("Updated text".to_string()));
}

/// Success Criterion 2: Reactive attributes update on Signal changes
#[wasm_bindgen_test]
fn test_reactive_attribute() {
	let doc = document();
	let div = doc.create_element("div").unwrap();

	// Create a Signal
	let count = Signal::new(0);

	// Bind attribute to Signal
	div.set_reactive_attribute("data-count", count.clone());

	// Initial value
	// Note: Effect runs immediately, but we need to flush updates
	use reinhardt_pages::reactive::with_runtime;
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(div.get_attribute("data-count"), Some("0".to_string()));

	// Update Signal
	count.set(42);
	with_runtime(|rt| rt.flush_updates());

	// Attribute should be updated automatically
	assert_eq!(div.get_attribute("data-count"), Some("42".to_string()));

	// Update again
	count.set(100);
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(div.get_attribute("data-count"), Some("100".to_string()));
}

/// Success Criterion 2: Multiple reactive attributes on same element
#[wasm_bindgen_test]
fn test_multiple_reactive_attributes() {
	let doc = document();
	let div = doc.create_element("div").unwrap();

	let x = Signal::new(10);
	let y = Signal::new(20);

	div.set_reactive_attribute("data-x", x.clone());
	div.set_reactive_attribute("data-y", y.clone());

	use reinhardt_pages::reactive::with_runtime;
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(div.get_attribute("data-x"), Some("10".to_string()));
	assert_eq!(div.get_attribute("data-y"), Some("20".to_string()));

	// Update both
	x.set(100);
	y.set(200);
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(div.get_attribute("data-x"), Some("100".to_string()));
	assert_eq!(div.get_attribute("data-y"), Some("200".to_string()));
}

/// Success Criterion 3: Event listeners work
#[wasm_bindgen_test]
fn test_event_listener() {
	use std::cell::RefCell;
	use std::rc::Rc;

	let doc = document();
	let button = doc.create_element("button").unwrap();

	let click_count = Rc::new(RefCell::new(0));
	let click_count_clone = click_count.clone();

	// Add event listener
	let _handle = button.add_event_listener("click", move || {
		*click_count_clone.borrow_mut() += 1;
	});

	// Initial state
	assert_eq!(*click_count.borrow(), 0);

	// Simulate click by dispatching event
	let event = web_sys::Event::new("click").unwrap();
	button.as_web_sys().dispatch_event(&event).unwrap();

	// Click count should be incremented
	assert_eq!(*click_count.borrow(), 1);

	// Click again
	button.as_web_sys().dispatch_event(&event).unwrap();
	assert_eq!(*click_count.borrow(), 2);
}

/// Success Criterion 3 & 4: Event listener cleanup (RAII pattern)
#[wasm_bindgen_test]
fn test_event_listener_cleanup() {
	use std::cell::RefCell;
	use std::rc::Rc;

	let doc = document();
	let button = doc.create_element("button").unwrap();

	let click_count = Rc::new(RefCell::new(0));
	let click_count_clone = click_count.clone();

	{
		// EventHandle is created in this scope
		let _handle = button.add_event_listener("click", move || {
			*click_count_clone.borrow_mut() += 1;
		});

		// Click while handle is alive
		let event = web_sys::Event::new("click").unwrap();
		button.as_web_sys().dispatch_event(&event).unwrap();
		assert_eq!(*click_count.borrow(), 1);

		// handle.drop() is called here
	}

	// After handle is dropped, listener should be removed
	let event = web_sys::Event::new("click").unwrap();
	button.as_web_sys().dispatch_event(&event).unwrap();

	// Click count should NOT increase (listener was removed)
	assert_eq!(*click_count.borrow(), 1);
}

/// Success Criterion 4: No memory leaks from multiple event listeners
#[wasm_bindgen_test]
fn test_multiple_event_listeners_cleanup() {
	use std::cell::RefCell;
	use std::rc::Rc;

	let doc = document();
	let button = doc.create_element("button").unwrap();

	let count1 = Rc::new(RefCell::new(0));
	let count2 = Rc::new(RefCell::new(0));

	let count1_clone = count1.clone();
	let count2_clone = count2.clone();

	{
		let _handle1 = button.add_event_listener("click", move || {
			*count1_clone.borrow_mut() += 1;
		});

		let _handle2 = button.add_event_listener("click", move || {
			*count2_clone.borrow_mut() += 1;
		});

		// Both listeners active
		let event = web_sys::Event::new("click").unwrap();
		button.as_web_sys().dispatch_event(&event).unwrap();

		assert_eq!(*count1.borrow(), 1);
		assert_eq!(*count2.borrow(), 1);

		// Both handles dropped here
	}

	// After drop, listeners should be removed
	let event = web_sys::Event::new("click").unwrap();
	button.as_web_sys().dispatch_event(&event).unwrap();

	// Counts should NOT increase
	assert_eq!(*count1.borrow(), 1);
	assert_eq!(*count2.borrow(), 1);
}

/// Integration test: Reactive attribute + Event listener
#[wasm_bindgen_test]
fn test_reactive_attribute_with_event_listener() {
	let doc = document();
	let button = doc.create_element("button").unwrap();

	let count = Signal::new(0);
	let count_clone = count.clone();

	// Reactive attribute
	button.set_reactive_attribute("data-count", count.clone());

	use reinhardt_pages::reactive::with_runtime;
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(button.get_attribute("data-count"), Some("0".to_string()));

	// Event listener that updates Signal
	let _handle = button.add_event_listener("click", move || {
		count_clone.update(|n| *n += 1);
	});

	// Click button
	let event = web_sys::Event::new("click").unwrap();
	button.as_web_sys().dispatch_event(&event).unwrap();

	// Flush updates to apply reactive changes
	with_runtime(|rt| rt.flush_updates());

	// Attribute should be updated automatically
	assert_eq!(button.get_attribute("data-count"), Some("1".to_string()));

	// Click again
	button.as_web_sys().dispatch_event(&event).unwrap();
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(button.get_attribute("data-count"), Some("2".to_string()));
}
