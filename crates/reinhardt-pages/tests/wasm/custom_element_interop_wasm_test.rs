//! Custom element interop tests for explicit DOM property and event APIs.
//!
//! **Run with**: `wasm-pack test --headless --chrome`

#![cfg(wasm)]

use std::cell::RefCell;
use std::rc::Rc;

use reinhardt_pages::dom::{CustomEventOptions, Element};
use reinhardt_pages::event::CustomEventDetailError;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WidgetDetail {
	value: String,
	count: u32,
}

#[wasm_bindgen_test]
fn element_property_helpers_keep_js_properties_distinct_from_attributes() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let web_element = document.create_element("rh-widget").expect("element");
	let element = Element::new(web_element);

	element
		.set_attribute("value", "attribute-value")
		.expect("attribute");
	element
		.set_property("value", &JsValue::from_str("property-value"))
		.expect("property");

	assert_eq!(
		element.get_attribute("value").as_deref(),
		Some("attribute-value")
	);
	assert_eq!(
		element
			.get_property("value")
			.expect("get property")
			.as_string(),
		Some("property-value".to_string())
	);

	assert!(element.delete_property("value").expect("delete property"));
	assert!(
		element
			.get_property("value")
			.expect("get deleted property")
			.is_undefined()
	);
}

#[wasm_bindgen_test]
fn raw_custom_event_listener_receives_detail() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = Element::new(document.create_element("rh-widget").expect("element"));
	let received = Rc::new(RefCell::new(None));

	let handle = element.add_custom_event_listener("widget-ready", {
		let received = Rc::clone(&received);
		move |detail| {
			*received.borrow_mut() = detail.as_string();
		}
	});

	element
		.dispatch_custom_event("widget-ready", &JsValue::from_str("ready"))
		.expect("dispatch custom event");

	assert_eq!(received.borrow().as_deref(), Some("ready"));
	drop(handle);
}

#[wasm_bindgen_test]
fn typed_custom_event_listener_decodes_detail_and_drops_cleanly() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = Element::new(document.create_element("rh-widget").expect("element"));
	let received = Rc::new(RefCell::new(None::<WidgetDetail>));

	let handle = element.add_typed_custom_event_listener::<WidgetDetail, _>("widget-change", {
		let received = Rc::clone(&received);
		move |event| {
			assert_eq!(event.event_type(), "widget-change");
			assert!(event.raw().dyn_ref::<web_sys::CustomEvent>().is_some());
			*received.borrow_mut() = Some(event.into_detail().expect("typed detail"));
		}
	});

	let detail = WidgetDetail {
		value: "selected".to_string(),
		count: 3,
	};
	let detail_value = serde_wasm_bindgen::to_value(&detail).expect("serialize detail");
	element
		.dispatch_custom_event_with_options(
			"widget-change",
			&detail_value,
			CustomEventOptions::new().bubbles(true).composed(true),
		)
		.expect("dispatch typed custom event");

	assert_eq!(*received.borrow(), Some(detail));

	drop(handle);
	*received.borrow_mut() = None;

	let ignored_detail = serde_wasm_bindgen::to_value(&WidgetDetail {
		value: "ignored".to_string(),
		count: 4,
	})
	.expect("serialize ignored detail");
	element
		.dispatch_custom_event("widget-change", &ignored_detail)
		.expect("dispatch after drop");

	assert_eq!(*received.borrow(), None);
}

#[wasm_bindgen_test]
fn typed_custom_event_listener_defers_malformed_detail_error() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = Element::new(document.create_element("rh-widget").expect("element"));
	let received_error = Rc::new(RefCell::new(None));

	let handle = element.add_typed_custom_event_listener::<WidgetDetail, _>("widget-change", {
		let received_error = Rc::clone(&received_error);
		move |event| {
			*received_error.borrow_mut() = Some(event.into_detail().expect_err("malformed detail"));
		}
	});

	element
		.dispatch_custom_event("widget-change", &JsValue::from_str("not a widget detail"))
		.expect("dispatch malformed custom event");

	assert!(matches!(
		received_error.borrow().as_ref(),
		Some(CustomEventDetailError::Deserialize { event_type, .. }) if event_type == "widget-change"
	));
	drop(handle);
}

#[wasm_bindgen_test]
fn typed_custom_event_listener_receives_same_named_plain_event() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = Element::new(document.create_element("rh-widget").expect("element"));
	let received_error = Rc::new(RefCell::new(None));

	let handle = element.add_typed_custom_event_listener::<WidgetDetail, _>("widget-change", {
		let received_error = Rc::clone(&received_error);
		move |event| {
			*received_error.borrow_mut() = Some(event.into_detail().expect_err("plain event"));
		}
	});

	let plain = web_sys::Event::new("widget-change").expect("plain event");
	element
		.as_web_sys()
		.dispatch_event(&plain)
		.expect("dispatch plain event");
	assert!(matches!(
		received_error.borrow().as_ref(),
		Some(CustomEventDetailError::NotCustomEvent { event_type })
			if event_type == "widget-change"
	));
	drop(handle);
}
