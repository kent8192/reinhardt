//! Browser-level coverage for the headless UI primitives.

#![cfg(wasm)]

use std::cell::Cell;
use std::rc::Rc;

use reinhardt_pages::component::{
	Component, IntoPage, Page, PageElement, PageExt, cleanup_reactive_nodes,
};
use reinhardt_pages::dom::Element;
use reinhardt_pages::hydration::reconcile;
use reinhardt_pages::prelude::{defer_yield, use_action};
use reinhardt_pages::reactive::ReactiveScope;
use reinhardt_pages::ui::{ActionButton, ActionResultPanel};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct BodyRoot {
	element: web_sys::Element,
}

impl BodyRoot {
	fn new(id: &str) -> Self {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let element = document.create_element("div").expect("create root");
		element.set_id(id);
		document
			.body()
			.expect("body")
			.append_child(&element)
			.expect("append root");
		Self { element }
	}
}

impl Drop for BodyRoot {
	fn drop(&mut self) {
		cleanup_reactive_nodes();
		self.element.remove();
	}
}

#[wasm_bindgen_test]
fn reactive_boolean_attributes_remove_falsy_values_after_mount() {
	// Arrange
	let root = BodyRoot::new("reactive-boolean-attribute");
	let page = PageElement::new("button")
		.reactive_attr("DISABLED", || Some("false".into()))
		.into_page();

	// Act
	page.mount(&Element::new(root.element.clone()))
		.expect("button mounts");

	// Assert
	let button = root
		.element
		.query_selector("button")
		.expect("query button")
		.expect("button exists");
	assert!(!button.has_attribute("disabled"));
}

#[wasm_bindgen_test]
fn hydration_reconciliation_ignores_static_reactive_attribute_overrides() {
	// Arrange
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = document.create_element("div").expect("create element");
	element
		.set_attribute("class", "current")
		.expect("set server attribute");
	let page = PageElement::new("div")
		.attr("class", "stale")
		.reactive_attr("CLASS", || Some("current".into()))
		.into_page();

	// Act
	let result = reconcile(&Element::new(element), &page);

	// Assert
	assert!(result.is_ok());
}

#[wasm_bindgen_test]
fn hydration_reconciles_the_effective_duplicate_boolean_attribute() {
	// Arrange
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let element = document.create_element("button").expect("create button");
	element
		.set_attribute("disabled", "true")
		.expect("set server attribute");
	let page = PageElement::new("button")
		.attr("disabled", "true")
		.attr("DISABLED", "false")
		.into_page();

	// Act
	let result = reconcile(&Element::new(element.clone()), &page);

	// Assert
	assert!(result.is_ok(), "{result:?}");
	assert!(element.has_attribute("disabled"));
}

#[wasm_bindgen_test]
async fn action_button_mounts_dispatches_once_and_exposes_pending_attributes() {
	let root = BodyRoot::new("ui-action-button-pending");
	let scope = ReactiveScope::new();
	let invocations = Rc::new(Cell::new(0));
	let invocations_for_action = Rc::clone(&invocations);

	scope.enter(|| {
		let action = use_action(move |_: ()| {
			invocations_for_action.set(invocations_for_action.get() + 1);
			async { std::future::pending::<Result<String, String>>().await }
		});
		ActionButton::new(action, (), Page::text("Run"))
			.render()
			.mount(&Element::new(root.element.clone()))
			.expect("action button mounts");
	});

	let button = root
		.element
		.query_selector("button")
		.expect("query button")
		.expect("button exists")
		.dyn_into::<web_sys::HtmlButtonElement>()
		.expect("button element");
	assert_eq!(button.get_attribute("type").as_deref(), Some("button"));
	assert_eq!(button.get_attribute("disabled"), None);
	assert_eq!(button.get_attribute("aria-busy"), None);

	let click = web_sys::MouseEvent::new("click").expect("click event");
	button
		.dispatch_event(&click)
		.expect("first click dispatches");
	defer_yield().await;
	assert_eq!(invocations.get(), 1);
	assert!(button.has_attribute("disabled"));
	assert_eq!(button.get_attribute("aria-busy").as_deref(), Some("true"));

	button
		.dispatch_event(&click)
		.expect("second click dispatches");
	defer_yield().await;
	assert_eq!(invocations.get(), 1);

	scope.dispose();
}

#[wasm_bindgen_test]
async fn action_result_panel_rerenders_success_and_error_slots_after_mount() {
	let root = BodyRoot::new("ui-action-result");
	let scope = ReactiveScope::new();

	scope.enter(|| {
		let action = use_action(|_: ()| async { Ok::<String, String>("saved".to_string()) });
		PageElement::new("div")
			.child(ActionButton::new(action, (), Page::text("Save")).into_page())
			.child(
				ActionResultPanel::new(action)
					.idle(|| Page::text("idle"))
					.pending(|| Page::text("pending"))
					.success(|value| Page::text(format!("success:{value}")))
					.error(|error| Page::text(format!("error:{error}")))
					.into_page(),
			)
			.into_page()
			.mount(&Element::new(root.element.clone()))
			.expect("success view mounts");
	});

	let button = root
		.element
		.query_selector("button")
		.expect("query button")
		.expect("button exists")
		.dyn_into::<web_sys::HtmlButtonElement>()
		.expect("button element");
	assert_eq!(root.element.text_content().as_deref(), Some("Saveidle"));
	button
		.dispatch_event(&web_sys::MouseEvent::new("click").expect("click event"))
		.expect("success click dispatches");
	defer_yield().await;
	defer_yield().await;
	assert_eq!(
		root.element.text_content().as_deref(),
		Some("Savesuccess:saved")
	);
	assert_eq!(button.get_attribute("disabled"), None);
	assert_eq!(button.get_attribute("aria-busy"), None);

	scope.dispose();

	let error_root = BodyRoot::new("ui-action-error");
	let error_scope = ReactiveScope::new();
	error_scope.enter(|| {
		let action = use_action(|_: ()| async { Err::<String, String>("failed".to_string()) });
		ActionButton::new(action, (), Page::text("Retry"))
			.into_page()
			.mount(&Element::new(error_root.element.clone()))
			.expect("error button mounts");
		ActionResultPanel::new(action)
			.idle(|| Page::text("idle"))
			.error(|error| Page::text(format!("error:{error}")))
			.into_page()
			.mount(&Element::new(error_root.element.clone()))
			.expect("error panel mounts");
	});

	let error_button = error_root
		.element
		.query_selector("button")
		.expect("query error button")
		.expect("error button exists")
		.dyn_into::<web_sys::HtmlButtonElement>()
		.expect("error button element");
	error_button
		.dispatch_event(&web_sys::MouseEvent::new("click").expect("error click event"))
		.expect("error click dispatches");
	defer_yield().await;
	defer_yield().await;
	assert_eq!(
		error_root.element.text_content().as_deref(),
		Some("Retryerror:failed")
	);
	assert_eq!(error_button.get_attribute("disabled"), None);
	assert_eq!(error_button.get_attribute("aria-busy"), None);

	error_scope.dispose();
}
