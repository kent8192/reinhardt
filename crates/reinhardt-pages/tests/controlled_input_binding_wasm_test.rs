#![cfg(wasm)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Function, Reflect};
use reinhardt_pages::component::{
	Component, ControlBinding, ControlBindingError, ControlKind, ControlValue, IntoPage,
	MountError, NumberParseError, NumberParseErrorKind, Page, PageExt,
};
use reinhardt_pages::dom::Element;
use reinhardt_pages::event::EventFile;
use reinhardt_pages::prelude::defer_yield;
use reinhardt_pages::reactive::{ReactiveScope, Signal, with_runtime};
use reinhardt_pages::{PageElement, page};
use rstest::rstest;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct SsrStateElement(web_sys::Element);

struct AttachedRootCleanup(web_sys::Element);

struct HydratedControlPage(Page);

impl Component for HydratedControlPage {
	fn name() -> &'static str {
		"HydratedControlPage"
	}

	fn render(&self) -> Page {
		self.0.clone()
	}
}

impl Drop for AttachedRootCleanup {
	fn drop(&mut self) {
		reinhardt_pages::cleanup_reactive_nodes();
		self.0.remove();
	}
}

impl SsrStateElement {
	fn install(document: &web_sys::Document) -> Self {
		if let Some(existing) = document.get_element_by_id("ssr-state") {
			existing.remove();
		}
		let element = document.create_element("script").expect("state element");
		element.set_id("ssr-state");
		element.set_text_content(Some(""));
		document
			.body()
			.expect("body")
			.append_child(&element)
			.expect("state mount");
		Self(element)
	}
}

impl Drop for SsrStateElement {
	fn drop(&mut self) {
		self.0.remove();
	}
}

#[wasm_bindgen_test]
fn public_page_mount_installs_control_binding() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("signal".to_owned());
		let observed = Rc::new(RefCell::new(String::new()));
		let handler_value = value.clone();
		let handler_observed = Rc::clone(&observed);
		page!({
			input {
				a11y: off,
				bind: value,
				@input: move |_| *handler_observed.borrow_mut() = handler_value.get(),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		assert_eq!(input.value(), "signal");
		input.set_value("dom");
		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "dom");
		assert_eq!(&*observed.borrow(), "dom");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn unrelated_reactive_attributes_preserve_an_active_control_edit() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("bound".to_owned());
		let invalid = Signal::new(false);
		let invalid_for_page = invalid.clone();
		PageElement::new("input")
			.attr("type", "text")
			.reactive_attr("aria-invalid", move || {
				Some(invalid_for_page.get().to_string().into())
			})
			.control_binding(ControlBinding::text(value.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		input.set_value("draft");
		invalid.set(true);

		assert_eq!(value.get(), "bound");
		assert_eq!(input.value(), "draft");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[rstest]
#[case::mount(false, false)]
#[case::reactive_if(true, false)]
#[case::hydrate(false, true)]
#[case::hydrate_reactive_if(true, true)]
#[wasm_bindgen_test]
fn range_reconciliation_updates_the_bound_value_as_the_step_base(
	#[case] nested_in_reactive_if: bool,
	#[case] hydrate: bool,
) {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let container = document.create_element("div").expect("container");
		let _cleanup = AttachedRootCleanup(container.clone());
		let value = Signal::new(3_i32);
		let range_value = value.clone();
		let range = move || {
			PageElement::new("input")
				.attr("type", "range")
				.attr("step", "2")
				.reactive_attr("class", || Some("stepped".into()))
				.control_binding(ControlBinding::number(range_value.clone()))
				.into_page()
		};
		let input_page = if nested_in_reactive_if {
			Page::reactive_if(|| true, range, || Page::Empty)
		} else {
			range()
		};
		let page = PageElement::new("div").child(input_page).into_page();

		// Act
		let root = if hydrate {
			container.set_inner_html(&page.render_to_string());
			let root = Element::new(container.first_element_child().expect("SSR root"));
			let _state = SsrStateElement::install(&document);
			reinhardt_pages::hydration::hydrate(&HydratedControlPage(page), &root)
				.expect("hydrate");
			root
		} else {
			let root = Element::new(container);
			page.mount(&root).expect("mount");
			root
		};
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("input")
			.expect("query")
			.expect("range")
			.unchecked_into();

		// Assert
		assert_eq!(value.get(), 3);
		assert_eq!(input.value(), "3");
		assert_eq!(input.get_attribute("value").as_deref(), Some("3"));

		value.set(4);

		assert_eq!(value.get(), 4);
		assert_eq!(input.value(), "4");
		assert_eq!(input.default_value(), "4");
		assert!(
			input.is_same_node(
				root.as_web_sys()
					.query_selector("input")
					.unwrap()
					.as_deref()
			)
		);
	});
}

#[rstest]
#[case::mount(false, false)]
#[case::reactive_if(true, false)]
#[case::hydrate(false, true)]
#[case::hydrate_reactive_if(true, true)]
#[wasm_bindgen_test]
fn reactive_range_constraints_reconcile_shared_peers(
	#[case] nested_in_reactive_if: bool,
	#[case] hydrate: bool,
) {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let container = document.create_element("div").expect("container");
		let _cleanup = AttachedRootCleanup(container.clone());
		let value = Signal::new(100_i32);
		let max = Signal::new(100_i32);
		let first_range = move || {
			PageElement::new("input")
				.attr("id", "first")
				.attr("type", "range")
				.attr("min", "0")
				.reactive_attr("max", move || Some(max.get().to_string().into()))
				.control_binding(ControlBinding::number(value))
				.into_page()
		};
		let first_range = if nested_in_reactive_if {
			Page::reactive_if(|| true, first_range, || Page::Empty)
		} else {
			first_range()
		};
		let page = PageElement::new("div")
			.child(first_range)
			.child(
				PageElement::new("input")
					.attr("id", "second")
					.attr("type", "range")
					.attr("min", "200")
					.attr("max", "300")
					.control_binding(ControlBinding::number(value))
					.into_page(),
			)
			.into_page();
		let root = if hydrate {
			container.set_inner_html(&page.render_to_string());
			let root = Element::new(container.first_element_child().expect("SSR root"));
			let _state = SsrStateElement::install(&document);
			reinhardt_pages::hydration::hydrate(&HydratedControlPage(page), &root)
				.expect("hydrate");
			root
		} else {
			let root = Element::new(container);
			page.mount(&root).expect("mount");
			root
		};
		let first: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#first")
			.expect("query")
			.expect("first range")
			.unchecked_into();
		let second: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#second")
			.expect("query")
			.expect("second range")
			.unchecked_into();
		value.set(100);
		with_runtime(|runtime| runtime.flush_updates());
		assert_eq!(value.get(), 100);
		assert_eq!(
			(first.value(), second.value()),
			("100".to_owned(), "200".to_owned())
		);

		// Act: only the already-representable control's constraints change.
		max.set(300);
		with_runtime(|runtime| runtime.flush_updates());
		with_runtime(|runtime| runtime.flush_updates());

		// Assert
		assert_eq!(value.get(), 200);
		assert_eq!(
			(first.value(), second.value()),
			("200".to_owned(), "200".to_owned())
		);
		assert!(
			second.is_same_node(
				root.as_web_sys()
					.query_selector("#second")
					.unwrap()
					.as_deref()
			)
		);
	});
}

#[rstest]
#[wasm_bindgen_test]
fn range_binding_canonicalizes_negative_zero() {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let value = Signal::new(-0.0_f64);
		PageElement::new("input")
			.attr("type", "range")
			.attr("step", "any")
			.control_binding(ControlBinding::number(value))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("range")
			.unchecked_into();
		assert_eq!(input.value(), "0");
		assert_eq!(value.get().to_bits(), 0.0_f64.to_bits());

		// Act
		value.set(-0.0);

		// Assert
		assert_eq!(input.value(), "0");
		assert_eq!(value.get().to_bits(), 0.0_f64.to_bits());
	});
}

#[rstest]
#[wasm_bindgen_test]
fn shared_ranges_with_different_step_grids_keep_their_local_projection() {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let value = Signal::new(8_i32);
		Page::Fragment(
			["2", "3"]
				.into_iter()
				.map(|step| {
					PageElement::new("input")
						.attr("type", "range")
						.attr("min", "0")
						.attr("max", "15")
						.attr("step", step)
						.control_binding(ControlBinding::number(value))
						.into_page()
				})
				.collect(),
		)
		.mount(&root)
		.expect("mount");
		let first: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("first range")
			.unchecked_into();
		let second: web_sys::HtmlInputElement = root
			.as_web_sys()
			.last_element_child()
			.expect("second range")
			.unchecked_into();

		// Act: the grids intersect, but their local projections of 8 cycle 9 <-> 10.
		value.set(8);
		with_runtime(|runtime| runtime.flush_updates());

		// Assert
		assert_eq!(value.get(), 8);
		assert_eq!(
			(first.value(), second.value()),
			("8".to_owned(), "9".to_owned())
		);
	});
}

#[rstest]
#[case::remove_one(true, false)]
#[case::dispose_signal_before_queued_reconciliation(true, true)]
#[case::unmount_all(false, false)]
#[case::dispose_signal_before_unmount(false, true)]
#[wasm_bindgen_test]
fn shared_range_removal_reconciles_only_live_surviving_controls(
	#[case] remove_one: bool,
	#[case] dispose_signal: bool,
) {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let signal_scope = ReactiveScope::new();
		let value = signal_scope.enter(|| Signal::new(100_i32));
		let show_first = Signal::new(true);
		Page::Fragment(vec![
			Page::reactive_if(
				move || show_first.get(),
				move || {
					PageElement::new("input")
						.attr("type", "range")
						.attr("min", "0")
						.attr("max", "100")
						.control_binding(ControlBinding::number(value))
						.into_page()
				},
				|| Page::Empty,
			),
			PageElement::new("input")
				.attr("type", "range")
				.attr("min", "200")
				.attr("max", "300")
				.control_binding(ControlBinding::number(value))
				.into_page(),
		])
		.mount(&root)
		.expect("mount");
		let first: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("first range")
			.unchecked_into();
		let survivor: web_sys::HtmlInputElement = root
			.as_web_sys()
			.last_element_child()
			.expect("surviving range")
			.unchecked_into();
		let effects = with_runtime(|runtime| runtime.debug_subscribers(value.id()));
		assert_eq!(value.get(), 100);
		assert_eq!(
			(first.value(), survivor.value()),
			("100".to_owned(), "200".to_owned())
		);

		// Act
		if remove_one {
			show_first.set(false);
		}
		if dispose_signal {
			signal_scope.dispose();
		}
		if remove_one {
			with_runtime(|runtime| runtime.flush_updates());
			assert!(survivor.is_same_node(root.as_web_sys().first_element_child().as_deref()));
			assert_eq!(survivor.value(), "200");
		}
		let expected_value = if remove_one { 200 } else { 100 };
		if !dispose_signal {
			assert_eq!(value.get(), expected_value);
		}
		drop(cleanup);
		with_runtime(|runtime| runtime.flush_updates());

		// Assert: full teardown cancels reconciliation and all removed control effects.
		assert_eq!(
			with_runtime(|runtime| runtime.subscriber_count(value.id())),
			0
		);
		assert!(
			effects
				.iter()
				.all(|effect| !with_runtime(|runtime| runtime.has_node(*effect)))
		);
		if !dispose_signal {
			assert_eq!(value.get(), expected_value);
			value.set(250);
		}
		assert_eq!(
			(first.value(), survivor.value()),
			("100".to_owned(), "200".to_owned())
		);
	});
}

#[rstest]
#[case(false)]
#[case(true)]
#[wasm_bindgen_test]
fn reactive_multiple_reconciles_email_value_sanitization(#[case] nested_in_reactive_if: bool) {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("a@example.test,  b@example.test ".to_owned());
		let multiple = Signal::new(false);
		let reactive_multiple = multiple.clone();
		let email_value = value.clone();
		let email = move || {
			let reactive_multiple = reactive_multiple.clone();
			PageElement::new("input")
				.attr("type", "email")
				.reactive_attr("multiple", move || {
					Some(reactive_multiple.get().to_string().into())
				})
				.control_binding(ControlBinding::text(email_value.clone()))
				.into_page()
		};
		let page = if nested_in_reactive_if {
			Page::reactive_if(|| true, email, || Page::Empty)
		} else {
			email()
		};
		page.mount(&root).expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("email")
			.unchecked_into();

		// Act
		multiple.set(true);

		// Assert
		assert_eq!(value.get(), "a@example.test,b@example.test");
		assert_eq!(input.value(), "a@example.test,b@example.test");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[rstest]
#[case(false)]
#[case(true)]
#[wasm_bindgen_test]
fn reactive_password_type_removes_the_serialized_bound_value(#[case] nested_in_reactive_if: bool) {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_form = document.create_element("form").expect("form");
		let form: web_sys::HtmlFormElement = raw_form.clone().unchecked_into();
		let root = Element::new(raw_form);
		let password_type = Signal::new(false);
		let reactive_password_type = password_type;
		let value = Signal::new("secret".to_owned());
		let bound_value = value;
		let input = move || {
			let reactive_password_type = reactive_password_type;
			PageElement::new("input")
				.attr("type", "text")
				.reactive_attr("type", move || {
					Some(if reactive_password_type.get() {
						"password".into()
					} else {
						"text".into()
					})
				})
				.control_binding(ControlBinding::text(bound_value))
				.into_page()
		};
		let page = if nested_in_reactive_if {
			Page::reactive_if(|| true, input, || Page::Empty)
		} else {
			input()
		};
		page.mount(&root).expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		password_type.set(true);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(input.type_(), "password");
		assert_eq!(input.value(), "secret");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(value.get(), "secret");

		password_type.set(false);
		with_runtime(|runtime| runtime.flush_updates());
		input.set_value("draft");
		form.reset();

		assert_eq!(input.type_(), "text");
		assert_eq!(input.value(), "secret");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

fn password_reset_form(
	value: Signal<String>,
	hydrate: bool,
) -> (
	AttachedRootCleanup,
	web_sys::HtmlFormElement,
	web_sys::HtmlInputElement,
) {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let container = document.create_element("div").expect("container");
	let cleanup = AttachedRootCleanup(container.clone());
	document
		.body()
		.expect("body")
		.append_child(&container)
		.expect("attach form container");
	let page = PageElement::new("form")
		.child(
			PageElement::new("input")
				.attr("type", "password")
				.control_binding(ControlBinding::text(value)),
		)
		.child(PageElement::new("button").attr("type", "reset"))
		.into_page();
	if hydrate {
		container.set_inner_html(&page.render_to_string());
		let root = Element::new(container.first_element_child().expect("SSR form"));
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(&HydratedControlPage(page), &root)
			.expect("hydrate password form");
	} else {
		page.mount(&Element::new(container.clone()))
			.expect("mount password form");
	}
	let form: web_sys::HtmlFormElement = container
		.first_element_child()
		.expect("form")
		.unchecked_into();
	let input = form
		.query_selector("input")
		.expect("query password")
		.expect("password input")
		.unchecked_into();
	(cleanup, form, input)
}

#[rstest]
#[case::mount_form_reset(false, false)]
#[case::mount_reset_button(false, true)]
#[case::hydrate_form_reset(true, false)]
#[case::hydrate_reset_button(true, true)]
#[test_attr(wasm_bindgen_test)]
async fn bound_password_reset_respects_cancellation_and_clears_the_signal(
	#[case] hydrate: bool,
	#[case] use_reset_button: bool,
) {
	// Arrange
	let scope = ReactiveScope::new();
	let value = scope.enter(|| Signal::new("initial-secret".to_owned()));
	let (_cleanup, form, input) = scope.enter(|| password_reset_form(value, hydrate));
	assert_eq!(input.value(), "initial-secret");
	assert_eq!(input.get_attribute("value"), None);
	assert_eq!(input.default_value(), "");
	input.set_value("edited-secret");
	input
		.dispatch_event(&web_sys::InputEvent::new("input").expect("input event"))
		.expect("dispatch password edit");
	assert_eq!(value.get(), "edited-secret");
	let cancel_reset = Rc::new(Cell::new(true));
	let reset_count = Rc::new(Cell::new(0));
	let cancel_for_handler = Rc::clone(&cancel_reset);
	let count_for_handler = Rc::clone(&reset_count);
	let _reset_listener = Element::new(form.clone().unchecked_into())
		.add_event_listener_with_event("reset", move |event| {
			count_for_handler.set(count_for_handler.get() + 1);
			event.stop_propagation();
			if cancel_for_handler.get() {
				event.prevent_default();
			}
		});
	let reset = || {
		if use_reset_button {
			form.query_selector("button")
				.expect("query reset button")
				.expect("reset button")
				.unchecked_into::<web_sys::HtmlElement>()
				.click();
		} else {
			form.reset();
		}
	};

	// Act: a cancelled reset preserves both the live value and the signal.
	reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(reset_count.get(), 1);
	assert_eq!(input.value(), "edited-secret");
	assert_eq!(value.get(), "edited-secret");
	assert_eq!(input.get_attribute("value"), None);
	assert_eq!(input.default_value(), "");

	// Act: allow the browser reset despite the stopped event propagation.
	cancel_reset.set(false);
	reset();
	defer_yield().await;
	// Native clicks can run microtasks before the reset default action completes.
	assert_eq!(value.get(), "edited-secret");
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(reset_count.get(), 2);
	assert_eq!(input.value(), "");
	assert_eq!(value.get(), "");
	assert_eq!(input.get_attribute("value"), None);
	assert_eq!(input.default_value(), "");
}

#[rstest]
#[case::mount_unmount(false, false)]
#[case::mount_dispose_signal(false, true)]
#[case::hydrate_unmount(true, false)]
#[case::hydrate_dispose_signal(true, true)]
#[test_attr(wasm_bindgen_test)]
async fn bound_password_reset_queued_before_teardown_skips_removed_controls(
	#[case] hydrate: bool,
	#[case] dispose_signal: bool,
) {
	// Arrange
	let signal_scope = ReactiveScope::new();
	let value = signal_scope.enter(|| Signal::new("secret".to_owned()));
	let binding_scope = ReactiveScope::new();
	let (cleanup, form, input) = binding_scope.enter(|| password_reset_form(value, hydrate));
	let effects = with_runtime(|runtime| runtime.debug_subscribers(value.id()));
	assert_eq!(input.value(), "secret");
	assert_eq!(input.get_attribute("value"), None);
	assert_eq!(input.default_value(), "");

	// Act: remove the binding before its deferred reset reconciliation runs.
	form.reset();
	drop(cleanup);
	if dispose_signal {
		drop(signal_scope);
	}
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert: queued work neither writes a removed binding nor revives its effects.
	assert_eq!(input.value(), "");
	assert_eq!(input.get_attribute("value"), None);
	assert_eq!(input.default_value(), "");
	assert_eq!(
		with_runtime(|runtime| runtime.subscriber_count(value.id())),
		0
	);
	assert!(
		effects
			.iter()
			.all(|effect| !with_runtime(|runtime| runtime.has_node(*effect)))
	);
	if dispose_signal {
		assert!(value.try_get_untracked().is_err());
	} else {
		assert_eq!(value.get(), "secret");
		value.set("after-teardown".to_owned());
		assert_eq!(input.value(), "");
	}
}

#[rstest]
#[case(false)]
#[case(true)]
#[wasm_bindgen_test]
fn reactive_text_binding_rejects_file_type_transition(#[case] nested_in_reactive_if: bool) {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let file_type = Signal::new(false);
		let reactive_file_type = file_type;
		let value = Signal::new("bound".to_owned());
		let bound_value = value;
		let input = move || {
			let reactive_file_type = reactive_file_type;
			PageElement::new("input")
				.attr("type", "text")
				.reactive_attr("type", move || {
					Some(if reactive_file_type.get() {
						"file".into()
					} else {
						"text".into()
					})
				})
				.control_binding(ControlBinding::text(bound_value))
				.into_page()
		};
		let page = if nested_in_reactive_if {
			Page::reactive_if(|| true, input, || Page::Empty)
		} else {
			input()
		};
		page.mount(&root).expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		file_type.set(true);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(input.type_(), "text");
		assert_eq!(input.value(), "bound");
		assert_eq!(value.get(), "bound");
	});
}

#[rstest]
#[case(false, false)]
#[case(false, true)]
#[case(true, false)]
#[case(true, true)]
#[wasm_bindgen_test]
fn reactive_select_cardinality_changes_are_rejected(
	#[case] starts_multiple: bool,
	#[case] nested_in_reactive_if: bool,
) {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let multiple = Signal::new(starts_multiple);
		let reactive_multiple = multiple;
		let binding = if starts_multiple {
			ControlBinding::select_many(Signal::new(vec!["rust".to_owned()]))
		} else {
			ControlBinding::select_one(Signal::new("rust".to_owned()))
		};
		let select = move || {
			let reactive_multiple = reactive_multiple;
			let element = PageElement::new("select");
			let element = if starts_multiple {
				element.attr("multiple", "multiple")
			} else {
				element
			};
			element
				.reactive_attr("multiple", move || {
					Some(reactive_multiple.get().to_string().into())
				})
				.control_binding(binding.clone())
				.child(
					PageElement::new("option")
						.attr("value", "rust")
						.child("Rust"),
				)
				.child(
					PageElement::new("option")
						.attr("value", "wasm")
						.child("WebAssembly"),
				)
				.into_page()
		};
		let page = if nested_in_reactive_if {
			Page::reactive_if(|| true, select, || Page::Empty)
		} else {
			select()
		};
		page.mount(&root).expect("mount");
		let select: web_sys::HtmlSelectElement = root
			.as_web_sys()
			.first_element_child()
			.expect("select")
			.unchecked_into();

		multiple.set(!starts_multiple);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(select.multiple(), starts_multiple);
	});
}

#[rstest]
#[case(false)]
#[case(true)]
#[wasm_bindgen_test]
fn duplicate_static_password_type_uses_the_browser_first_value(
	#[case] nested_in_reactive_if: bool,
) {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("secret".to_owned());
		let bound_value = value;
		let input = move || {
			PageElement::new("input")
				.attr("type", "password")
				.attr("type", "text")
				.control_binding(ControlBinding::text(bound_value))
				.into_page()
		};
		let page = if nested_in_reactive_if {
			Page::reactive_if(|| true, input, || Page::Empty)
		} else {
			input()
		};
		page.mount(&root).expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		assert_eq!(input.type_(), "password");
		assert_eq!(input.value(), "secret");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(value.get(), "secret");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

struct HydratedReactiveAttributeInput {
	invalid: Signal<bool>,
	value: Signal<String>,
}

struct HydratedReactivePasswordInput {
	password_type: Signal<bool>,
	value: Signal<String>,
}

struct HydratedRejectedReactiveTypeInput {
	file_type: Signal<bool>,
	value: Signal<String>,
}

struct HydratedDuplicateStaticPasswordInput {
	value: Signal<String>,
}

struct HydratedReactiveNumberInput {
	max: Signal<u16>,
	value: Signal<u8>,
}

struct HydratedReactiveRangeInput {
	max: Signal<u16>,
	value: Signal<u8>,
}

impl Component for HydratedReactivePasswordInput {
	fn name() -> &'static str {
		"HydratedReactivePasswordInput"
	}

	fn render(&self) -> Page {
		let password_type = self.password_type;
		PageElement::new("input")
			.attr("type", "text")
			.reactive_attr("type", move || {
				Some(if password_type.get() {
					"password".into()
				} else {
					"text".into()
				})
			})
			.control_binding(ControlBinding::text(self.value))
			.into_page()
	}
}

impl Component for HydratedRejectedReactiveTypeInput {
	fn name() -> &'static str {
		"HydratedRejectedReactiveTypeInput"
	}

	fn render(&self) -> Page {
		let file_type = self.file_type;
		let value = self.value;
		PageElement::new("div")
			.child(Page::reactive(move || {
				let reactive_file_type = file_type;
				PageElement::new("input")
					.attr("type", "text")
					.reactive_attr("type", move || {
						Some(if reactive_file_type.get() {
							"file".into()
						} else {
							"text".into()
						})
					})
					.control_binding(ControlBinding::text(value))
					.into_page()
			}))
			.into_page()
	}
}

impl Component for HydratedDuplicateStaticPasswordInput {
	fn name() -> &'static str {
		"HydratedDuplicateStaticPasswordInput"
	}

	fn render(&self) -> Page {
		PageElement::new("input")
			.attr("type", "password")
			.attr("type", "text")
			.control_binding(ControlBinding::text(self.value))
			.into_page()
	}
}

impl Component for HydratedReactiveNumberInput {
	fn name() -> &'static str {
		"HydratedReactiveNumberInput"
	}

	fn render(&self) -> Page {
		let max = self.max;
		PageElement::new("input")
			.attr("type", "number")
			.reactive_attr("max", move || Some(max.get().to_string().into()))
			.control_binding(ControlBinding::number(self.value))
			.into_page()
	}
}

impl Component for HydratedReactiveRangeInput {
	fn name() -> &'static str {
		"HydratedReactiveRangeInput"
	}

	fn render(&self) -> Page {
		let max = self.max;
		PageElement::new("input")
			.attr("type", "range")
			.reactive_attr("max", move || Some(max.get().to_string().into()))
			.control_binding(ControlBinding::number(self.value))
			.into_page()
	}
}

impl Component for HydratedReactiveAttributeInput {
	fn name() -> &'static str {
		"HydratedReactiveAttributeInput"
	}

	fn render(&self) -> Page {
		let invalid = self.invalid.clone();
		PageElement::new("input")
			.attr("type", "text")
			.reactive_attr("aria-invalid", move || {
				Some(invalid.get().to_string().into())
			})
			.control_binding(ControlBinding::text(self.value.clone()))
			.into_page()
	}
}

#[rstest]
#[wasm_bindgen_test]
fn hydrated_unrelated_reactive_attributes_preserve_an_active_control_edit() {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_attribute("type", "text").expect("input type");
		raw_input
			.set_attribute("aria-invalid", "false")
			.expect("initial reactive attribute");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		input.set_value("bound");
		let root = Element::new(raw_input);
		let invalid = Signal::new(false);
		let value = Signal::new("bound".to_owned());
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(
			&HydratedReactiveAttributeInput {
				invalid: invalid.clone(),
				value: value.clone(),
			},
			&root,
		)
		.expect("hydrate");
		input.set_value("draft");

		// Act
		invalid.set(true);
		with_runtime(|runtime| runtime.flush_updates());

		// Assert
		assert_eq!(value.get(), "bound");
		assert_eq!(input.value(), "draft");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[rstest]
#[wasm_bindgen_test]
fn hydrated_password_type_removes_the_serialized_bound_value() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_attribute("type", "text").expect("input type");
		raw_input
			.set_attribute("value", "secret")
			.expect("serialized value");
		let raw_form = document.create_element("form").expect("form");
		raw_form.append_child(&raw_input).expect("form input");
		let form: web_sys::HtmlFormElement = raw_form.unchecked_into();
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let root = Element::new(raw_input);
		let password_type = Signal::new(false);
		let value = Signal::new("secret".to_owned());
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(
			&HydratedReactivePasswordInput {
				password_type,
				value,
			},
			&root,
		)
		.expect("hydrate");

		password_type.set(true);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(input.type_(), "password");
		assert_eq!(input.value(), "secret");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(value.get(), "secret");

		password_type.set(false);
		with_runtime(|runtime| runtime.flush_updates());
		input.set_value("draft");
		form.reset();

		assert_eq!(input.type_(), "text");
		assert_eq!(input.value(), "secret");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[rstest]
#[wasm_bindgen_test]
fn hydration_initial_password_type_removes_the_serialized_bound_value() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_attribute("type", "text").expect("input type");
		raw_input
			.set_attribute("value", "secret")
			.expect("serialized value");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let _cleanup = AttachedRootCleanup(raw_input.clone());
		let root = Element::new(raw_input);
		let password_type = Signal::new(true);
		let value = Signal::new("secret".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedReactivePasswordInput {
				password_type,
				value,
			},
			&root,
		)
		.expect("hydrate");

		assert_eq!(input.type_(), "password");
		assert_eq!(input.value(), "secret");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(value.get(), "secret");
	});
}

#[wasm_bindgen_test]
fn hydration_preserves_an_adopted_control_after_rejecting_its_initial_type() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_attribute("type", "text").expect("input type");
		raw_input
			.set_attribute("value", "server")
			.expect("serialized value");
		raw_root.append_child(&raw_input).expect("SSR child");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root);
		let file_type = Signal::new(true);
		let value = Signal::new("server".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedRejectedReactiveTypeInput { file_type, value },
			&root,
		)
		.expect("hydrate");

		assert!(raw_input.is_same_node(root.as_web_sys().first_element_child().as_deref()));
		assert_eq!(input.type_(), "text");
		assert_eq!(input.value(), "server");
		assert_eq!(value.get(), "server");
	});
}

#[rstest]
#[wasm_bindgen_test]
fn hydration_initial_reactive_attributes_preserve_rejected_numeric_edits() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_input = document.create_element("input").expect("input");
		raw_input
			.set_attribute("type", "number")
			.expect("input type");
		raw_input.set_attribute("max", "500").expect("input max");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let _cleanup = AttachedRootCleanup(raw_input.clone());
		input.set_value("300");
		let root = Element::new(raw_input);
		let max = Signal::new(500_u16);
		let value = Signal::new(7_u8);
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(&HydratedReactiveNumberInput { max, value }, &root)
			.expect("hydrate rejected numeric edit");

		assert_eq!(input.value(), "300");
		assert_eq!(value.get(), 7);
	});
}

struct ReactiveCleanupGuard {
	root: Option<web_sys::Element>,
}

impl ReactiveCleanupGuard {
	fn new() -> Self {
		Self { root: None }
	}

	fn attach_root(&mut self, root: &web_sys::Element) {
		self.root = Some(root.clone());
	}
}

impl Drop for ReactiveCleanupGuard {
	fn drop(&mut self) {
		reinhardt_pages::cleanup_reactive_nodes();
		if let Some(root) = self.root.take() {
			root.remove();
		}
	}
}

#[rstest]
#[wasm_bindgen_test]
fn hydration_initial_reactive_range_constraint_reconciles_browser_sanitization() {
	ReactiveScope::run(|| {
		// Arrange
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_input = document.create_element("input").expect("input");
		raw_input
			.set_attribute("type", "range")
			.expect("input type");
		raw_input.set_attribute("max", "100").expect("SSR max");
		raw_input.set_attribute("value", "100").expect("SSR value");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let _cleanup = AttachedRootCleanup(raw_input.clone());
		let root = Element::new(raw_input);
		let max = Signal::new(50_u16);
		let value = Signal::new(100_u8);
		let _state = SsrStateElement::install(&document);

		// Act
		reinhardt_pages::hydration::hydrate(&HydratedReactiveRangeInput { max, value }, &root)
			.expect("hydrate range with a narrower reactive maximum");

		// Assert
		assert_eq!(
			(input.max(), input.value(), value.get()),
			("50".to_owned(), "50".to_owned(), 50)
		);
	});
}

#[rstest]
#[wasm_bindgen_test]
fn hydration_accepts_the_browser_first_static_password_type() {
	ReactiveScope::run(|| {
		// Arrange: hydrate the actual core renderer output.
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let value = Signal::new("secret".to_owned());
		let component = HydratedDuplicateStaticPasswordInput { value };
		let container = document.create_element("div").expect("container");
		let _cleanup = AttachedRootCleanup(container.clone());
		container.set_inner_html(&component.render().render_to_string());
		let raw_input = container.first_element_child().expect("server input");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		let root = Element::new(raw_input);
		let _state = SsrStateElement::install(&document);
		assert_eq!(input.value(), "");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(
			input.get_attribute("data-rh-password-omitted").as_deref(),
			Some("true")
		);

		// Act
		reinhardt_pages::hydration::hydrate(&component, &root).expect("hydrate");

		// Assert
		assert_eq!(input.type_(), "password");
		assert_eq!(input.value(), "secret");
		assert_eq!(input.get_attribute("value"), None);
		assert_eq!(input.get_attribute("data-rh-password-omitted"), None);
		assert_eq!(value.get(), "secret");
	});
}

#[wasm_bindgen_test]
fn public_page_mount_supports_all_additional_bound_input_types() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		document
			.body()
			.expect("body")
			.append_child(&raw_root)
			.expect("attach root");
		let _cleanup = AttachedRootCleanup(raw_root.clone());
		let root = Element::new(raw_root.clone());
		let search = Signal::new("initial search".to_owned());
		let tel = Signal::new("+81-3-1234-5678".to_owned());
		let url_raw = "https://example.test\n";
		let url = Signal::new(url_raw.to_owned());
		let email_raw = "old@example.test\n";
		let email = Signal::new(email_raw.to_owned());
		let password = Signal::new("old-secret".to_owned());
		let color = Signal::new("#112233".to_owned());
		let date = Signal::new("2026-08-30".to_owned());
		let datetime_local = Signal::new("2026-08-30T09:15".to_owned());
		let month = Signal::new("2026-08".to_owned());
		let week = Signal::new("2026-W35".to_owned());
		let time = Signal::new("09:15".to_owned());
		let range = Signal::new(200_i32);

		page!({
			input {
				a11y: off,
				id: "search",
				type: "search",
				bind: search
			}
			input {
				a11y: off,
				id: "tel",
				type: "tel",
				bind: tel
			}
			input {
				a11y: off,
				id: "url",
				type: "url",
				bind: url
			}
			input {
				a11y: off,
				id: "email",
				type: "email",
				bind: email
			}
			input {
				a11y: off,
				id: "password",
				type: "password",
				bind: password
			}
			input {
				a11y: off,
				id: "color",
				type: "color",
				bind: color
			}
			input {
				a11y: off,
				id: "date",
				type: "date",
				bind: date
			}
			input {
				a11y: off,
				id: "datetime-local",
				type: "datetime-local",
				bind: datetime_local
			}
			input {
				a11y: off,
				id: "month",
				type: "month",
				bind: month
			}
			input {
				a11y: off,
				id: "week",
				type: "week",
				bind: week
			}
			input {
				a11y: off,
				id: "time",
				type: "time",
				bind: time
			}
			input {
				a11y: off,
				id: "range",
				type: "range",
				bind: range
			}
		})
		.mount(&root)
		.expect("mount");

		let initial_url: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#url")
			.expect("query")
			.expect("url input")
			.unchecked_into();
		assert_eq!(url.get(), initial_url.value());
		assert_ne!(url.get(), url_raw);
		let initial_email: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#email")
			.expect("query")
			.expect("email input")
			.unchecked_into();
		assert_eq!(email.get(), initial_email.value());
		assert_ne!(email.get(), email_raw);
		let initial_range: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#range")
			.expect("query")
			.expect("range input")
			.unchecked_into();
		assert_eq!(range.get(), 100);
		assert_eq!(initial_range.value(), "100");

		let check_text = |id: &str, signal: &Signal<String>, next: &str, empty: &str| {
			let input: web_sys::HtmlInputElement = root
				.as_web_sys()
				.query_selector(&format!("#{id}"))
				.expect("query")
				.expect("input")
				.unchecked_into();
			let element: web_sys::Element = input.clone().unchecked_into();
			input.focus().expect("focus");
			input.set_value(next);
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
				.expect("dispatch");

			assert_eq!(signal.get(), next);
			let current = root
				.as_web_sys()
				.query_selector(&format!("#{id}"))
				.expect("query")
				.expect("current input");
			assert!(element.is_same_node(Some(&current)));
			assert!(
				document
					.active_element()
					.is_some_and(|active| active.is_same_node(Some(&element)))
			);

			input.set_value("");
			input
				.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
				.expect("dispatch");
			assert_eq!(signal.get(), empty);
			let current = root
				.as_web_sys()
				.query_selector(&format!("#{id}"))
				.expect("query")
				.expect("current input");
			assert!(element.is_same_node(Some(&current)));
			assert!(
				document
					.active_element()
					.is_some_and(|active| active.is_same_node(Some(&element)))
			);
		};

		check_text("search", &search, "next search", "");
		check_text("tel", &tel, "+81-3-9876-5432", "");
		check_text("url", &url, "https://next.example.test", "");
		check_text("email", &email, "next@example.test", "");
		check_text("password", &password, "next-secret", "");
		check_text("color", &color, "#abcdef", "#000000");
		check_text("date", &date, "2026-08-31", "");
		check_text("datetime-local", &datetime_local, "2026-08-31T10:30", "");
		check_text("month", &month, "2026-09", "");
		check_text("week", &week, "2026-W36", "");
		check_text("time", &time, "10:30", "");

		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#date")
			.expect("query")
			.expect("date input")
			.unchecked_into();
		let element: web_sys::Element = input.clone().unchecked_into();
		input.focus().expect("focus");
		date.set("not-a-date".to_owned());
		assert_eq!(date.get(), "not-a-date");
		assert_eq!(input.value(), "");
		let current = root
			.as_web_sys()
			.query_selector("#date")
			.expect("query")
			.expect("current date input");
		assert!(element.is_same_node(Some(&current)));
		assert!(
			document
				.active_element()
				.is_some_and(|active| active.is_same_node(Some(&element)))
		);

		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("#range")
			.expect("query")
			.expect("range input")
			.unchecked_into();
		let element: web_sys::Element = input.clone().unchecked_into();
		input.focus().expect("focus");
		input.set_value("42");
		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(range.get(), 42);
		let current = root
			.as_web_sys()
			.query_selector("#range")
			.expect("query")
			.expect("current range input");
		assert!(element.is_same_node(Some(&current)));
		assert!(
			document
				.active_element()
				.is_some_and(|active| active.is_same_node(Some(&element)))
		);
	});
}

#[wasm_bindgen_test]
fn controlled_form_reset_uses_the_bound_initial_value() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("signal".to_owned());
		PageElement::new("form")
			.child(PageElement::new("input").control_binding(ControlBinding::text(value)))
			.into_page()
			.mount(&root)
			.expect("mount");
		let form: web_sys::HtmlFormElement = root
			.as_web_sys()
			.first_element_child()
			.expect("form")
			.unchecked_into();
		let input: web_sys::HtmlInputElement =
			form.first_element_child().expect("input").unchecked_into();

		input.set_value("edited");
		form.reset();

		assert_eq!(input.value(), "signal");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn controlled_textarea_ignores_stale_child_content_on_mount() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("signal".to_owned());
		PageElement::new("textarea")
			.control_binding(ControlBinding::text(value))
			.child("stale child")
			.into_page()
			.mount(&root)
			.expect("mount");
		let textarea: web_sys::HtmlTextAreaElement = root
			.as_web_sys()
			.first_element_child()
			.expect("textarea")
			.unchecked_into();

		assert_eq!(textarea.value(), "signal");
		assert_eq!(textarea.default_value().expect("default value"), "signal");
		assert_eq!(textarea.text_content(), Some("signal".to_owned()));
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_evaluates_dynamic_radio_value_once() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let selected = Signal::new("first".to_owned());
		let evaluations = Rc::new(Cell::new(0));
		let value_evaluations = Rc::clone(&evaluations);
		page!({
			input {
				a11y: off,
				type: "radio",
				value: {
					let count = value_evaluations.get() + 1;
					value_evaluations.set(count);
					if count == 1 { "first" } else { "second" }
				},
				bind: selected,
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		assert_eq!(evaluations.get(), 1);
		assert_eq!(input.value(), "first");
		assert!(input.checked());
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_projects_a_radio_binding_value_over_a_stale_attribute() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let selected = Signal::new("draft".to_owned());
		PageElement::new("input")
			.attr("type", "radio")
			.attr("value", "stale")
			.control_binding(ControlBinding::radio(selected, "draft".to_owned()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		assert_eq!(input.value(), "draft");
		assert!(input.checked());
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_applies_initial_select_one_after_mounting_options() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let selected = Signal::new("wasm".to_owned());
		page!({
			select {
				a11y: off,
				bind: selected,
				option {
					value: "rust",
					"Rust"
				}
				option {
					value: "wasm",
					"WebAssembly"
				}
			}
		})
		.mount(&root)
		.expect("mount");
		let select: web_sys::HtmlSelectElement = root
			.as_web_sys()
			.first_element_child()
			.expect("select")
			.unchecked_into();

		assert_eq!(select.value(), "wasm");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_marks_only_the_first_duplicate_select_one_default() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let selected = Signal::new("rust".to_owned());
		PageElement::new("select")
			.control_binding(ControlBinding::select_one(selected))
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.child("First"),
			)
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.child("Second"),
			)
			.into_page()
			.mount(&root)
			.expect("mount");
		let select: web_sys::HtmlSelectElement = root
			.as_web_sys()
			.first_element_child()
			.expect("select")
			.unchecked_into();
		let first: web_sys::HtmlOptionElement = select
			.options()
			.item(0)
			.expect("first option")
			.unchecked_into();
		let second: web_sys::HtmlOptionElement = select
			.options()
			.item(1)
			.expect("second option")
			.unchecked_into();

		assert!(first.default_selected());
		assert!(!second.default_selected());
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_applies_initial_select_many_after_mounting_options() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let selected = Signal::new(vec!["rust".to_owned(), "wasm".to_owned()]);
		page!({
			select {
				a11y: off,
				multiple: true,
				bind: selected,
				option {
					value: "rust",
					"Rust"
				}
				option {
					value: "wasm",
					"WebAssembly"
				}
			}
		})
		.mount(&root)
		.expect("mount");
		let select: web_sys::HtmlSelectElement = root
			.as_web_sys()
			.first_element_child()
			.expect("select")
			.unchecked_into();

		let rust: web_sys::HtmlOptionElement =
			select.item(0).expect("rust option").unchecked_into();
		let wasm: web_sys::HtmlOptionElement =
			select.item(1).expect("wasm option").unchecked_into();
		assert_eq!(select.selected_options().length(), 2);
		assert!(rust.selected());
		assert!(wasm.selected());
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test(async)]
async fn reactive_select_options_refresh_the_bound_value_without_a_signal_write() {
	let scope = ReactiveScope::new();
	let (root, show_options) = scope.enter(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let show_options = Signal::new(false);
		let selected = Signal::new(vec!["wasm".to_owned()]);
		let render_show_options = show_options.clone();
		PageElement::new("select")
			.attr("multiple", "multiple")
			.control_binding(ControlBinding::select_many(selected))
			.child(Page::reactive(move || {
				if render_show_options.get() {
					PageElement::new("option")
						.attr("value", "wasm")
						.child("WebAssembly")
						.into_page()
				} else {
					Page::Empty
				}
			}))
			.into_page()
			.mount(&root)
			.expect("mount");

		(root, show_options)
	});

	show_options.set(true);
	JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
		.await
		.expect("microtask");

	let select: web_sys::HtmlSelectElement = root
		.as_web_sys()
		.first_element_child()
		.expect("select")
		.unchecked_into();
	let option: web_sys::HtmlOptionElement = select.item(0).expect("option").unchecked_into();
	assert!(option.selected());
	reinhardt_pages::cleanup_reactive_nodes();
}

#[wasm_bindgen_test]
fn reactive_select_remount_applies_binding_after_mounting_replacement_options() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let alternate = Signal::new(false);
		let selected = Signal::new("wasm".to_owned());
		let render_alternate = alternate.clone();
		let render_selected = selected.clone();
		Page::reactive(move || {
			let bound = render_selected.clone();
			let id = if render_alternate.get() {
				"replacement"
			} else {
				"original"
			};
			page!({
				select {
					a11y: off,
					id: id,
					bind: bound,
					option {
						value: "rust",
						"Rust"
					}
					option {
						value: "wasm",
						"WebAssembly"
					}
				}
			})
		})
		.mount(&root)
		.expect("mount");

		alternate.set(true);
		let replacement: web_sys::HtmlSelectElement = root
			.as_web_sys()
			.first_element_child()
			.expect("replacement")
			.unchecked_into();
		assert_eq!(replacement.id(), "replacement");
		assert_eq!(replacement.value(), "wasm");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_page_mount_preserves_a_structured_binding_error() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let checked = Signal::new(false);
		let page = Page::Element(
			PageElement::new("select").control_binding(ControlBinding::checkbox(checked)),
		);

		let error = page.mount(&root).expect_err("mismatch");

		assert_eq!(
			error,
			MountError::ControlBinding(ControlBindingError::UnsupportedElement {
				control: ControlKind::Checkbox,
				actual_tag: "select".to_owned(),
			})
		);
	});
}

#[wasm_bindgen_test]
fn failed_select_mount_rolls_back_child_reactive_resources() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let parent_checked = Signal::new(false);
		let child_value = Signal::new("initial".to_owned());
		let render_count = Rc::new(std::cell::Cell::new(0));
		let listener_owner = Rc::new(());
		let weak_listener_owner = Rc::downgrade(&listener_owner);
		let render_value = child_value.clone();
		let render_count_for_child = Rc::clone(&render_count);
		let listener_owner_for_child = Rc::clone(&listener_owner);
		let page = Page::Element(
			PageElement::new("select")
				.control_binding(ControlBinding::checkbox(parent_checked))
				.child(Page::reactive(move || {
					render_count_for_child.set(render_count_for_child.get() + 1);
					let _ = render_value.get();
					let bound = render_value.clone();
					let listener_owner = Rc::clone(&listener_owner_for_child);
					page!({
						input {
							a11y: off,
							bind: bound,
							@input: move |_| drop(Rc::clone(&listener_owner)),
						}
					})
				})),
		);
		drop(listener_owner);

		let error = page.mount(&root).expect_err("parent binding mismatch");

		assert_eq!(
			error,
			MountError::ControlBinding(ControlBindingError::UnsupportedElement {
				control: ControlKind::Checkbox,
				actual_tag: "select".to_owned(),
			})
		);
		assert_eq!(render_count.get(), 1);
		assert!(weak_listener_owner.upgrade().is_none());
		assert_eq!(root.as_web_sys().first_element_child(), None);
		child_value.set("after failure".to_owned());
		assert_eq!(render_count.get(), 1);
	});
}

#[wasm_bindgen_test]
fn reactive_failed_select_mount_rolls_back_child_reactive_resources() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let trigger = Signal::new(0_u32);
		let render_count = Rc::new(std::cell::Cell::new(0));
		let last_listener_owner = Rc::new(RefCell::new(None));
		let render_trigger = trigger.clone();
		let render_count_for_page = Rc::clone(&render_count);
		let last_listener_owner_for_page = Rc::clone(&last_listener_owner);
		let page = Page::reactive(move || {
			let _ = render_trigger.get();
			render_count_for_page.set(render_count_for_page.get() + 1);
			let listener_owner = Rc::new(());
			*last_listener_owner_for_page.borrow_mut() = Some(Rc::downgrade(&listener_owner));
			Page::Element(
				PageElement::new("select")
					.control_binding(ControlBinding::checkbox(Signal::new(false)))
					.child(page!({
						input {
							a11y: off,
							bind: Signal::new(String::new()),
							@input: move |_| drop(Rc::clone(&listener_owner)),
						}
					})),
			)
		});

		page.mount(&root).expect("reactive owner mount");

		assert_eq!(
			root.as_web_sys().query_selector("select").expect("query"),
			None
		);
		assert_eq!(render_count.get(), 1);
		assert!(
			last_listener_owner
				.borrow()
				.as_ref()
				.expect("owner observation")
				.upgrade()
				.is_none()
		);
		trigger.set(1);
		assert_eq!(render_count.get(), 2);
		assert!(
			last_listener_owner
				.borrow()
				.as_ref()
				.expect("rerendered owner observation")
				.upgrade()
				.is_none()
		);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn reactive_invalid_nonselect_binding_is_omitted() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let page = Page::reactive(|| {
			PageElement::new("input")
				.attr("id", "invalid-nonselect")
				.control_binding(ControlBinding::select_one(Signal::new(String::new())))
				.into_page()
		});

		page.mount(&root).expect("reactive owner mount");

		assert_eq!(
			root.as_web_sys()
				.query_selector("#invalid-nonselect")
				.expect("query"),
			None
		);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn invalid_file_binding_is_omitted_before_form_ownership_is_established() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let page = PageElement::new("div")
			.control_binding(ControlBinding::file(Signal::new(Vec::new())))
			.into_page();

		let error = page.mount(&root).expect_err("file binding mismatch");

		assert_eq!(
			error,
			MountError::ControlBinding(ControlBindingError::UnsupportedElement {
				control: ControlKind::File,
				actual_tag: "div".to_owned(),
			})
		);
		assert_eq!(root.as_web_sys().first_element_child(), None);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn reactive_nonselect_mount_keeps_parent_when_a_child_mount_fails() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let retained_trigger = Signal::new(0_u32);
		let retained_render_count = Rc::new(Cell::new(0));
		let failed_trigger = Signal::new(0_u32);
		let failed_render_count = Rc::new(Cell::new(0));
		let listener_owner = Rc::new(RefCell::new(None));
		let retained_trigger_for_child = retained_trigger.clone();
		let retained_render_count_for_child = Rc::clone(&retained_render_count);
		let failed_trigger_for_child = failed_trigger.clone();
		let failed_render_count_for_child = Rc::clone(&failed_render_count);
		let listener_owner_for_child = Rc::clone(&listener_owner);
		let page = Page::reactive(move || {
			let retained_trigger = retained_trigger_for_child.clone();
			let retained_render_count = Rc::clone(&retained_render_count_for_child);
			let failed_trigger = failed_trigger_for_child.clone();
			let failed_render_count = Rc::clone(&failed_render_count_for_child);
			let listener_owner_for_child = Rc::clone(&listener_owner_for_child);
			Page::Element(
				PageElement::new("div")
					.attr("id", "retained-parent")
					.child(Page::Element(
						PageElement::new("section")
							.attr("id", "retained-child")
							.child(Page::reactive(move || {
								let value = retained_trigger.get();
								retained_render_count.set(retained_render_count.get() + 1);
								PageElement::new("span")
									.attr("id", "retained-reactive")
									.child(value.to_string())
									.into_page()
							}))
							.child(Page::Element(
								PageElement::new("select")
									.control_binding(ControlBinding::checkbox(Signal::new(false)))
									.child(Page::reactive(move || {
										let _ = failed_trigger.get();
										failed_render_count.set(failed_render_count.get() + 1);
										let owner = Rc::new(());
										*listener_owner_for_child.borrow_mut() =
											Some(Rc::downgrade(&owner));
										page!({
											input {
												a11y: off,
												bind: Signal::new(String::new()),
												@input: move |_| drop(Rc::clone(&owner)),
											}
										})
									})),
							))
							.child(
								PageElement::new("span")
									.attr("id", "nested-valid-sibling")
									.child("nested ready"),
							),
					))
					.child(Page::Element(
						PageElement::new("span")
							.attr("id", "valid-sibling")
							.child(Page::Text("ready".into())),
					)),
			)
		});

		page.mount(&root).expect("reactive owner mount");

		let parent = root
			.as_web_sys()
			.query_selector("#retained-parent")
			.expect("query")
			.expect("non-select parent should remain mounted");
		assert!(
			parent
				.query_selector("#retained-child")
				.expect("retained child query")
				.is_some()
		);
		assert_eq!(
			parent
				.query_selector("#nested-valid-sibling")
				.expect("nested sibling query")
				.expect("nested sibling")
				.text_content(),
			Some("nested ready".to_owned())
		);
		assert_eq!(
			parent
				.query_selector("#valid-sibling")
				.expect("sibling query")
				.expect("sibling")
				.text_content(),
			Some("ready".to_owned())
		);
		assert_eq!(
			(retained_render_count.get(), failed_render_count.get()),
			(1, 1)
		);
		assert!(
			listener_owner
				.borrow()
				.as_ref()
				.expect("owner observation")
				.upgrade()
				.is_none()
		);
		retained_trigger.set(1);
		failed_trigger.set(1);
		assert_eq!(retained_render_count.get(), 2);
		assert_eq!(failed_render_count.get(), 1);
		assert_eq!(
			parent
				.query_selector("#retained-reactive")
				.expect("reactive query")
				.expect("retained reactive")
				.text_content(),
			Some("1".to_owned())
		);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn failed_plain_parent_mount_drops_detached_reactive_children() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let trigger = Signal::new(0_u32);
		let render_count = Rc::new(Cell::new(0));
		let listener_owner = Rc::new(());
		let weak_listener_owner = Rc::downgrade(&listener_owner);
		let render_trigger = trigger.clone();
		let render_count_for_child = Rc::clone(&render_count);
		let listener_owner_for_child = Rc::clone(&listener_owner);
		let page = PageElement::new("div")
			.child(Page::reactive(move || {
				let value = render_trigger.get();
				render_count_for_child.set(render_count_for_child.get() + 1);
				let listener_owner = Rc::clone(&listener_owner_for_child);
				page!({
					button {
						id: "detached-reactive-child",
						@input: move |_| drop(Rc::clone(&listener_owner)),
						{ value.to_string() }
					}
				})
			}))
			.child(Page::Element(PageElement::new("input").control_binding(
				ControlBinding::select_one(Signal::new(String::new())),
			)))
			.into_page();
		drop(listener_owner);

		let error = page.mount(&root).expect_err("later invalid child binding");
		let renders_after_failure = render_count.get();
		trigger.set(1);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(
			error,
			MountError::ControlBinding(ControlBindingError::UnsupportedElement {
				control: ControlKind::SelectOne,
				actual_tag: "input".to_owned(),
			})
		);
		assert_eq!(root.as_web_sys().first_element_child(), None);
		assert_eq!(render_count.get(), renders_after_failure);
		assert!(weak_listener_owner.upgrade().is_none());
	});
}

fn browser_file(name: &str, contents: &str) -> web_sys::File {
	let bits = Array::new();
	bits.push(&JsValue::from_str(contents));
	let options = js_sys::Object::new();
	Reflect::set(
		&options,
		&JsValue::from_str("type"),
		&JsValue::from_str("text/plain"),
	)
	.expect("file media type");
	Reflect::set(
		&options,
		&JsValue::from_str("lastModified"),
		&JsValue::from_f64(1_000.0),
	)
	.expect("file modification time");
	let args = Array::new();
	args.push(&bits);
	args.push(&JsValue::from_str(name));
	args.push(&options);
	let constructor = Reflect::get(&js_sys::global(), &JsValue::from_str("File"))
		.expect("File constructor")
		.dyn_into::<Function>()
		.expect("File constructor type");
	Reflect::construct(&constructor, &args)
		.expect("browser file")
		.unchecked_into()
}

fn assign_files(input: &web_sys::HtmlInputElement, files: &[web_sys::File]) {
	let transfer = web_sys::DataTransfer::new().expect("data transfer");
	let items =
		Reflect::get(transfer.as_ref(), &JsValue::from_str("items")).expect("data transfer items");
	let add = Reflect::get(&items, &JsValue::from_str("add"))
		.expect("data transfer add")
		.dyn_into::<Function>()
		.expect("data transfer add function");
	for file in files {
		add.call1(&items, file.as_ref()).expect("add file");
	}
	input.set_files(transfer.files().as_ref());
}

fn file_reset_form(
	binding: ControlBinding,
	hydrate: bool,
) -> (
	AttachedRootCleanup,
	web_sys::HtmlFormElement,
	web_sys::HtmlInputElement,
) {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let container = document.create_element("div").expect("container");
	let cleanup = AttachedRootCleanup(container.clone());
	document
		.body()
		.expect("body")
		.append_child(&container)
		.expect("attach form");
	let page = PageElement::new("form")
		.child(
			PageElement::new("input")
				.attr("type", "file")
				.control_binding(binding),
		)
		.child(PageElement::new("button").attr("type", "reset"))
		.into_page();
	if hydrate {
		container.set_inner_html(&page.render_to_string());
		let root = Element::new(container.first_element_child().expect("SSR form"));
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(&HydratedControlPage(page), &root)
			.expect("hydrate file form");
	} else {
		page.mount(&Element::new(container.clone()))
			.expect("mount file form");
	}
	let form: web_sys::HtmlFormElement = container
		.first_element_child()
		.expect("form")
		.unchecked_into();
	let input = form
		.query_selector("input")
		.expect("query file input")
		.expect("file input")
		.unchecked_into();
	(cleanup, form, input)
}

fn shadow_file_reset_form(
	binding: ControlBinding,
	hydrate: bool,
) -> (
	AttachedRootCleanup,
	web_sys::HtmlFormElement,
	web_sys::HtmlInputElement,
) {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let host = document.create_element("div").expect("shadow host");
	let cleanup = AttachedRootCleanup(host.clone());
	document
		.body()
		.expect("body")
		.append_child(&host)
		.expect("attach shadow host");
	let shadow = host
		.attach_shadow(&web_sys::ShadowRootInit::new(web_sys::ShadowRootMode::Open))
		.expect("attach shadow root");
	let container = document.create_element("div").expect("container");
	shadow
		.append_child(&container)
		.expect("attach form container");
	let page = PageElement::new("form")
		.child(
			PageElement::new("input")
				.attr("type", "file")
				.control_binding(binding),
		)
		.into_page();
	if hydrate {
		container.set_inner_html(&page.render_to_string());
		let root = Element::new(container.first_element_child().expect("SSR form"));
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(&HydratedControlPage(page), &root)
			.expect("hydrate shadow file form");
	} else {
		page.mount(&Element::new(container.clone()))
			.expect("mount shadow file form");
	}
	let form: web_sys::HtmlFormElement = container
		.first_element_child()
		.expect("form")
		.unchecked_into();
	let input = form
		.query_selector("input")
		.expect("query file input")
		.expect("file input")
		.unchecked_into();
	(cleanup, form, input)
}

fn nested_file_reset_form(
	binding: ControlBinding,
	attached_shadow_root: bool,
	reactive_branch: bool,
) -> (
	AttachedRootCleanup,
	web_sys::HtmlFormElement,
	web_sys::HtmlInputElement,
) {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let host = document.create_element("div").expect("host");
	let cleanup = AttachedRootCleanup(host.clone());
	let container = if attached_shadow_root {
		document
			.body()
			.expect("body")
			.append_child(&host)
			.expect("attach shadow host");
		let shadow = host
			.attach_shadow(&web_sys::ShadowRootInit::new(web_sys::ShadowRootMode::Open))
			.expect("attach shadow root");
		let container = document.create_element("div").expect("container");
		shadow
			.append_child(&container)
			.expect("attach form container");
		container
	} else {
		host.clone()
	};
	let input = move || {
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(binding.clone())
			.into_page()
	};
	let input = if reactive_branch {
		Page::reactive_if(|| true, input, || Page::Empty)
	} else {
		input()
	};
	PageElement::new("form")
		.child(PageElement::new("div").child(input))
		.into_page()
		.mount(&Element::new(container.clone()))
		.expect("mount nested file form");
	let form: web_sys::HtmlFormElement = container
		.first_element_child()
		.expect("form")
		.unchecked_into();
	let input = form
		.query_selector("input")
		.expect("query file input")
		.expect("file input")
		.unchecked_into();
	(cleanup, form, input)
}

#[rstest]
#[case::mount_form_reset(false, false)]
#[case::mount_reset_button(false, true)]
#[case::hydrate_form_reset(true, false)]
#[case::hydrate_reset_button(true, true)]
#[test_attr(wasm_bindgen_test)]
async fn bound_file_reset_waits_for_task_and_respects_cancellation(
	#[case] hydrate: bool,
	#[case] use_reset_button: bool,
) {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (_cleanup, form, input) =
		scope.enter(|| file_reset_form(ControlBinding::file(files), hydrate));
	let selected = browser_file("selected.txt", "selected");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	let cancel = Rc::new(Cell::new(true));
	let cancel_handler = Rc::clone(&cancel);
	let _handler = Element::new(form.clone().unchecked_into()).add_event_listener_with_event(
		"reset",
		move |event| {
			event.stop_propagation();
			if cancel_handler.get() {
				event.prevent_default();
			}
		},
	);
	let reset = || {
		if use_reset_button {
			form.query_selector("button")
				.expect("query reset button")
				.expect("reset button")
				.unchecked_into::<web_sys::HtmlElement>()
				.click();
		} else {
			form.reset();
		}
	};

	// Act: cancellation must not publish a reset value.
	reset();
	TimeoutFuture::new(0).await;

	// Assert
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
	assert_eq!(input.files().expect("files").length(), 1);

	// Act: complete reset while preserving event/microtask ordering.
	cancel.set(false);
	reset();
	defer_yield().await;
	yield_microtask().await;

	// Assert: the Signal changes only at the task boundary.
	assert_eq!(files.get().len(), 1);
	assert_eq!(input.files().expect("files").length(), 0);
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());
	assert_eq!(files.get().len(), 0);
}

#[rstest]
#[case::mount(false)]
#[case::hydrate(true)]
#[test_attr(wasm_bindgen_test)]
async fn bound_file_reset_inside_attached_shadow_root_waits_for_task(#[case] hydrate: bool) {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (_cleanup, form, input) =
		scope.enter(|| shadow_file_reset_form(ControlBinding::file(files), hydrate));
	let selected = browser_file("shadow.txt", "shadow");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	let cancel = Rc::new(Cell::new(true));
	let cancel_handler = Rc::clone(&cancel);
	let _handler = Element::new(form.clone().unchecked_into()).add_event_listener_with_event(
		"reset",
		move |event| {
			if cancel_handler.get() {
				event.prevent_default();
			}
		},
	);

	// Act: cancellation preserves the browser selection and Signal.
	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(input.files().expect("files").length(), 1);
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));

	// Act: a successful reset remains deferred until the browser task.
	cancel.set(false);
	form.reset();
	defer_yield().await;
	yield_microtask().await;

	// Assert
	assert_eq!(input.files().expect("files").length(), 0);
	assert_eq!(files.get().len(), 1);
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());
	assert_eq!(input.files().expect("files").length(), 0);
	assert_eq!(files.get().len(), 0);
}

#[rstest]
#[case::detached_nested(false, false)]
#[case::attached_shadow_nested(true, false)]
#[case::detached_reactive_branch(false, true)]
#[test_attr(wasm_bindgen_test)]
async fn bound_nested_file_reset_uses_its_final_form_owner(
	#[case] attached_shadow_root: bool,
	#[case] reactive_branch: bool,
) {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (_cleanup, form, input) = scope.enter(|| {
		nested_file_reset_form(
			ControlBinding::file(files),
			attached_shadow_root,
			reactive_branch,
		)
	});
	let selected = browser_file("nested.txt", "nested");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	let cancel = Rc::new(Cell::new(true));
	let cancel_handler = Rc::clone(&cancel);
	let _handler = Element::new(form.clone().unchecked_into()).add_event_listener_with_event(
		"reset",
		move |event| {
			if cancel_handler.get() {
				event.prevent_default();
			}
		},
	);

	// Act: cancellation preserves the raw browser selection and Signal.
	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	let dom_file = input
		.files()
		.expect("files")
		.get(0)
		.expect("selected DOM file");
	assert!(js_sys::Object::is(dom_file.as_ref(), selected.as_ref()));
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));

	// Act: successful reset clears the DOM before the deferred binding update.
	cancel.set(false);
	form.reset();
	defer_yield().await;
	yield_microtask().await;

	// Assert
	assert_eq!(input.files().expect("files").length(), 0);
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());
	assert_eq!(input.files().expect("files").length(), 0);
	assert!(files.get().is_empty());
}

#[wasm_bindgen_test(async)]
async fn bound_file_reset_follows_form_reassociation() {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (_cleanup, original_form, input) =
		scope.enter(|| file_reset_form(ControlBinding::file(files), false));
	let selected = browser_file("selected.txt", "selected");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let current_form: web_sys::HtmlFormElement = document
		.create_element("form")
		.expect("current form")
		.unchecked_into();
	current_form.set_id("reinhardt-file-reset-current-owner");
	original_form
		.parent_element()
		.expect("form container")
		.append_child(&current_form)
		.expect("attach current form");
	input
		.set_attribute("form", "reinhardt-file-reset-current-owner")
		.expect("reassociate file input");

	// Act: resetting the former owner must not synchronize this control.
	original_form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
	assert_eq!(input.files().expect("files").length(), 1);

	// Act: the current owner resets the DOM and binding.
	current_form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(input.files().expect("files").length(), 0);
	assert_eq!(files.get().len(), 0);
}

#[wasm_bindgen_test(async)]
async fn bound_file_reset_writes_once_for_an_attached_form() {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let writes = Rc::new(Cell::new(0_usize));
	let reads = Rc::new(Cell::new(0_usize));
	let inner = scope.enter(|| ControlBinding::file(files));
	let read_binding = inner.clone();
	let write_binding = inner.clone();
	let write_count = Rc::clone(&writes);
	let read_count = Rc::clone(&reads);
	let binding = scope.enter(|| {
		ControlBinding::from_parts(
			ControlKind::File,
			None,
			inner.target(),
			move || {
				read_count.set(read_count.get() + 1);
				read_binding.read()
			},
			move |value| {
				write_count.set(write_count.get() + 1);
				write_binding.write(value)
			},
			move || {
				let previous = files.get();
				Box::new(move || files.set(previous))
			},
		)
	});
	let (_cleanup, form, input) = scope.enter(|| file_reset_form(binding, false));
	assign_files(&input, &[browser_file("selected.txt", "selected")]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	writes.set(0);

	// Act
	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(writes.get(), 1);
	assert_eq!(input.files().expect("files").length(), 0);
	assert_eq!(files.get().len(), 0);

	// Act: an already-empty reset performs one reconciliation read and no write.
	reads.set(0);
	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(reads.get(), 1);
	assert_eq!(writes.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn bound_file_reset_adopts_a_different_file_with_the_same_metadata() {
	// Arrange
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (_cleanup, form, input) =
		scope.enter(|| file_reset_form(ControlBinding::file(files), false));
	let first = browser_file("same.txt", "same");
	let second = browser_file("same.txt", "same");
	assert!(!js_sys::Object::is(first.as_ref(), second.as_ref()));
	assign_files(&input, &[first.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	let input_for_reset = input.clone();
	let second_for_reset = second.clone();
	let _handler = Element::new(form.clone().unchecked_into()).add_event_listener_with_event(
		"reset",
		move |_| {
			let input = input_for_reset.clone();
			let second = second_for_reset.clone();
			let assign = wasm_bindgen::closure::Closure::once_into_js(move || {
				assign_files(&input, &[second]);
			});
			let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
			let then = Reflect::get(promise.as_ref(), &JsValue::from_str("then"))
				.expect("Promise.then")
				.unchecked_into::<Function>();
			then.call1(promise.as_ref(), &assign)
				.expect("queue reset microtask");
		},
	);

	// Act
	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	// Assert
	assert_eq!(input.files().expect("files").length(), 1);
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		second.as_ref()
	));
}

async fn yield_microtask() {
	JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
		.await
		.expect("microtask");
}

#[wasm_bindgen_test(async)]
async fn bound_file_input_syncs_the_signal_after_a_normal_form_reset() {
	let _cleanup = ReactiveCleanupGuard::new();
	let scope = ReactiveScope::new();
	let (root, files) = scope.enter(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		let files_for_reset = files.clone();
		let files_seen_by_reset = Rc::new(RefCell::new(Vec::new()));
		let files_seen_by_reset_handler = Rc::clone(&files_seen_by_reset);
		page!({
			form {
				@reset: move |_| {
					*files_seen_by_reset_handler.borrow_mut() = files_for_reset
						.get()
						.iter()
						.map(|file| file.name().to_owned())
						.collect()
				},
				input {
					a11y: off,
					type: "file",
					bind: files,
				}
			}
		})
		.mount(&root)
		.expect("mount");
		let form: web_sys::HtmlFormElement = root
			.as_web_sys()
			.first_element_child()
			.expect("form")
			.unchecked_into();
		let input: web_sys::HtmlInputElement =
			form.first_element_child().expect("input").unchecked_into();
		let selected = browser_file("selected.txt", "selected");
		assign_files(&input, &[selected]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch");

		assert_eq!(files.get().len(), 1);
		form.reset();
		assert_eq!(&*files_seen_by_reset.borrow(), &["selected.txt"]);

		(root, files)
	});

	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	let input: web_sys::HtmlInputElement = root
		.as_web_sys()
		.first_element_child()
		.expect("form")
		.first_element_child()
		.expect("input")
		.unchecked_into();
	assert_eq!(input.files().expect("files").length(), 0);
	assert!(files.get().is_empty());
}

#[wasm_bindgen_test(async)]
async fn bound_file_input_preserves_the_signal_when_form_reset_is_prevented() {
	let _cleanup = ReactiveCleanupGuard::new();
	let scope = ReactiveScope::new();
	let (root, files, selected) = scope.enter(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		let reset_seen = Rc::new(Cell::new(false));
		let reset_seen_handler = Rc::clone(&reset_seen);
		page!({
			form {
				@reset: move |event| {
					reset_seen_handler.set(true);
					event.prevent_default();
				},
				input {
					a11y: off,
					type: "file",
					bind: files,
				}
			}
		})
		.mount(&root)
		.expect("mount");
		let form: web_sys::HtmlFormElement = root
			.as_web_sys()
			.first_element_child()
			.expect("form")
			.unchecked_into();
		let input: web_sys::HtmlInputElement =
			form.first_element_child().expect("input").unchecked_into();
		let selected = browser_file("selected.txt", "selected");
		assign_files(&input, &[selected.clone()]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch");

		form.reset();
		assert!(reset_seen.get());

		(root, files, selected)
	});

	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	let input: web_sys::HtmlInputElement = root
		.as_web_sys()
		.first_element_child()
		.expect("form")
		.first_element_child()
		.expect("input")
		.unchecked_into();
	let dom_file = input
		.files()
		.expect("files")
		.get(0)
		.expect("selected DOM file");
	assert!(js_sys::Object::is(dom_file.as_ref(), selected.as_ref()));
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
}

#[wasm_bindgen_test]
fn bound_file_input_fresh_mount_normalizes_nonempty_signal_to_empty() {
	ReactiveScope::run(|| {
		// Arrange
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(vec![reinhardt_pages::event::EventFile::from(browser_file(
			"stale.txt",
			"stale",
		))]);

		// Act
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(files.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		// Assert
		assert!(files.get().is_empty());
		assert_eq!(input.files().expect("files").length(), 0);
	});
}

#[wasm_bindgen_test]
fn hydration_clears_stale_file_signal_when_live_file_list_is_empty() {
	ReactiveScope::run(|| {
		// Arrange
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw.clone().unchecked_into();
		input.set_type("file");
		let root = Element::new(raw);
		let files = Signal::new(vec![reinhardt_pages::event::EventFile::from(browser_file(
			"server.txt",
			"server",
		))]);
		let _state = SsrStateElement::install(&document);

		// Act
		reinhardt_pages::hydration::hydrate(
			&HydratedFileInput {
				files: files.clone(),
			},
			&root,
		)
		.expect("hydrate empty file input");

		// Assert
		assert_eq!(input.files().expect("files").length(), 0);
		assert!(files.get().is_empty());
	});
}

#[wasm_bindgen_test]
fn bound_file_input_clears_signal_after_picker_returns_empty_selection() {
	ReactiveScope::run(|| {
		// Arrange
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(files.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		assign_files(&input, &[browser_file("selected.txt", "selected")]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch selection");
		assert_eq!(files.get().len(), 1);

		// Act
		assign_files(&input, &[]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch empty selection");

		// Assert
		assert!(files.get().is_empty());
		assert_eq!(input.files().expect("files").length(), 0);
	});
}

#[wasm_bindgen_test]
fn bound_file_input_keeps_focus_when_cleared_reactively() {
	ReactiveScope::run(|| {
		// Arrange
		let mut cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		cleanup.attach_root(&raw_root);
		document
			.body()
			.expect("body")
			.append_child(&raw_root)
			.expect("attach root");
		let root = Element::new(raw_root.clone());
		let files = Signal::new(Vec::new());
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(files.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		let node: web_sys::Element = input.clone().unchecked_into();
		assign_files(&input, &[browser_file("selected.txt", "selected")]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch selection");
		input.focus().expect("focus");

		// Act
		files.set(Vec::new());

		// Assert
		assert_eq!(input.files().expect("files").length(), 0);
		assert!(
			node.is_same_node(
				root.as_web_sys()
					.first_element_child()
					.as_ref()
					.map(|element| &**element)
			)
		);
		assert!(
			document
				.active_element()
				.is_some_and(|active| active.is_same_node(Some(&node)))
		);
	});
}

#[wasm_bindgen_test(async)]
async fn bound_file_input_removes_its_form_reset_listener_on_cleanup() {
	let scope = ReactiveScope::new();
	let files = scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let (cleanup, form, input) =
		scope.enter(|| file_reset_form(ControlBinding::file(files), false));
	let selected = browser_file("selected.txt", "selected");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	assert_eq!(files.get().len(), 1);
	drop(cleanup);

	form.reset();
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
}

#[wasm_bindgen_test(async)]
async fn bound_file_input_does_not_write_after_cleanup_when_form_reset_is_pending() {
	let signal_scope = ReactiveScope::new();
	let files = signal_scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let binding_scope = ReactiveScope::new();
	let (cleanup, form, input) =
		binding_scope.enter(|| file_reset_form(ControlBinding::file(files), false));
	let effects = with_runtime(|runtime| runtime.debug_subscribers(files.id()));
	let selected = browser_file("selected.txt", "selected");
	assign_files(&input, &[selected.clone()]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	assert_eq!(files.get().len(), 1);

	form.reset();
	drop(cleanup);
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	assert_eq!(
		with_runtime(|runtime| runtime.subscriber_count(files.id())),
		0
	);
	assert!(
		effects
			.iter()
			.all(|effect| !with_runtime(|runtime| runtime.has_node(*effect)))
	);
	assert_eq!(files.get().len(), 1);
	assert!(js_sys::Object::is(
		files.get()[0].raw().as_ref(),
		selected.as_ref()
	));
	assert_eq!(input.files().expect("files").length(), 0);
}

#[wasm_bindgen_test(async)]
async fn bound_file_input_does_not_write_after_signal_disposal_when_reset_is_pending() {
	let signal_scope = ReactiveScope::new();
	let files = signal_scope.enter(|| Signal::new(Vec::<EventFile>::new()));
	let binding_scope = ReactiveScope::new();
	let (_cleanup, form, input) =
		binding_scope.enter(|| file_reset_form(ControlBinding::file(files), false));
	assign_files(&input, &[browser_file("selected.txt", "selected")]);
	input
		.dispatch_event(&web_sys::Event::new("change").expect("change"))
		.expect("dispatch selection");
	assert_eq!(files.get().len(), 1);

	form.reset();
	drop(signal_scope);
	TimeoutFuture::new(0).await;
	with_runtime(|runtime| runtime.flush_updates());

	assert_eq!(
		with_runtime(|runtime| runtime.subscriber_count(files.id())),
		0
	);
	assert!(files.try_get_untracked().is_err());
	assert_eq!(input.files().expect("files").length(), 0);
}

#[wasm_bindgen_test]
fn bound_file_input_reads_data_transfer_order_before_change_handler() {
	ReactiveScope::run(|| {
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		let observed = Rc::new(RefCell::new(Vec::new()));
		let observed_handler = Rc::clone(&observed);
		let files_handler = files.clone();
		page!({
			input {
				a11y: off,
				type: "file",
				multiple: true,
				bind: files,
				@change: move |_| *observed_handler.borrow_mut() = files_handler.get(),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		let first = browser_file("first.txt", "one");
		let second = browser_file("second.txt", "two");
		assign_files(&input, &[first, second]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch");

		assert_eq!(
			files
				.get()
				.iter()
				.map(|file| file.name())
				.collect::<Vec<_>>(),
			vec!["first.txt", "second.txt"]
		);
		assert_eq!(
			observed
				.borrow()
				.iter()
				.map(|file| file.name())
				.collect::<Vec<_>>(),
			vec!["first.txt", "second.txt"]
		);
	});
}

#[wasm_bindgen_test]
fn bound_file_input_clears_without_replacing_the_dom_node() {
	ReactiveScope::run(|| {
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(files.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		let node: web_sys::Element = input.clone().unchecked_into();
		assign_files(&input, &[browser_file("selected.txt", "selected")]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch");
		files.set(Vec::new());

		assert_eq!(input.files().expect("files").length(), 0);
		assert!(
			node.is_same_node(
				root.as_web_sys()
					.first_element_child()
					.as_ref()
					.map(|element| &**element)
			)
		);
	});
}

#[wasm_bindgen_test]
fn bound_file_input_normalizes_nonempty_signal_to_live_selection() {
	ReactiveScope::run(|| {
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let files = Signal::new(Vec::new());
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(files.clone()))
			.into_page()
			.mount(&root)
			.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		let selected = browser_file("selected.txt", "selected");
		assign_files(&input, &[selected.clone()]);
		input
			.dispatch_event(&web_sys::Event::new("change").expect("change"))
			.expect("dispatch");
		files.set(vec![reinhardt_pages::event::EventFile::from(browser_file(
			"other.txt",
			"other",
		))]);

		assert!(js_sys::Object::is(
			files.get()[0].raw().as_ref(),
			selected.as_ref()
		));
	});
}

struct HydratedFileInput {
	files: Signal<Vec<reinhardt_pages::event::EventFile>>,
}

struct FailingHydrationAfterFile {
	files: Signal<Vec<reinhardt_pages::event::EventFile>>,
}

impl Component for HydratedFileInput {
	fn name() -> &'static str {
		"HydratedFileInput"
	}

	fn render(&self) -> Page {
		PageElement::new("input")
			.attr("type", "file")
			.control_binding(ControlBinding::file(self.files.clone()))
			.into_page()
	}
}

impl Component for FailingHydrationAfterFile {
	fn name() -> &'static str {
		"FailingHydrationAfterFile"
	}

	fn render(&self) -> Page {
		PageElement::new("div")
			.child(
				PageElement::new("input")
					.attr("type", "file")
					.control_binding(ControlBinding::file(self.files.clone())),
			)
			.child(Page::Element(
				PageElement::new("select")
					.control_binding(ControlBinding::checkbox(Signal::new(false))),
			))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn hydration_adopts_preselected_live_file_list() {
	ReactiveScope::run(|| {
		let _cleanup = ReactiveCleanupGuard::new();
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw.clone().unchecked_into();
		input.set_type("file");
		let selected = browser_file("hydrated.txt", "hydrated");
		assign_files(&input, &[selected.clone()]);
		let root = Element::new(raw);
		let files = Signal::new(vec![reinhardt_pages::event::EventFile::from(browser_file(
			"server.txt",
			"server",
		))]);
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedFileInput {
				files: files.clone(),
			},
			&root,
		)
		.expect("hydrate file input");

		assert_eq!(files.get().len(), 1);
		assert!(js_sys::Object::is(
			files.get()[0].raw().as_ref(),
			selected.as_ref()
		));
	});
}

#[wasm_bindgen_test]
fn failed_hydration_restores_the_file_signal() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_input = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		input.set_type("file");
		let live_file = browser_file("live.txt", "live");
		assign_files(&input, &[live_file]);
		raw_root.append_child(&raw_input).expect("file input");
		raw_root
			.append_child(&document.create_element("select").expect("select"))
			.expect("invalid sibling");
		let server_file =
			reinhardt_pages::event::EventFile::from(browser_file("server.txt", "server"));
		let files = Signal::new(vec![server_file.clone()]);
		let root = Element::new(raw_root);
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&FailingHydrationAfterFile {
				files: files.clone(),
			},
			&root,
		)
		.expect_err("later invalid binding");

		assert_eq!(files.get().len(), 1);
		assert!(js_sys::Object::is(
			files.get()[0].raw().as_ref(),
			server_file.raw().as_ref()
		));
	});
}

fn dispatch_before_input(input: &web_sys::HtmlInputElement, data: Option<&str>, input_type: &str) {
	let init = web_sys::InputEventInit::new();
	init.set_data(data);
	init.set_input_type(input_type);
	input
		.dispatch_event(
			&web_sys::InputEvent::new_with_event_init_dict("beforeinput", &init)
				.expect("beforeinput"),
		)
		.expect("dispatch beforeinput");
}

fn dispatch_input(input: &web_sys::HtmlInputElement, data: Option<&str>, input_type: &str) {
	let init = web_sys::InputEventInit::new();
	init.set_data(data);
	init.set_input_type(input_type);
	input
		.dispatch_event(
			&web_sys::InputEvent::new_with_event_init_dict("input", &init).expect("input"),
		)
		.expect("dispatch input");
}

fn dispatch_before_input_with_transfer(
	input: &web_sys::HtmlInputElement,
	data: &str,
	input_type: &str,
) {
	let transfer = web_sys::DataTransfer::new().expect("data transfer");
	transfer
		.set_data("text/plain", data)
		.expect("set transfer data");
	let init = web_sys::InputEventInit::new();
	init.set_data_transfer(Some(&transfer));
	init.set_input_type(input_type);
	input
		.dispatch_event(
			&web_sys::InputEvent::new_with_event_init_dict("beforeinput", &init)
				.expect("beforeinput"),
		)
		.expect("dispatch beforeinput");
}

fn mounted_number_control(
	initial: i32,
) -> (
	Element,
	web_sys::HtmlInputElement,
	Signal<i32>,
	Signal<Option<NumberParseError>>,
) {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let root = Element::new(document.create_element("div").expect("root"));
	let value = Signal::new(initial);
	let error = Signal::new(None::<NumberParseError>);
	page!({
		input {
			a11y: off,
			type: "number",
			bind: number(value, error),
		}
	})
	.mount(&root)
	.expect("mount");
	let input = root
		.as_web_sys()
		.first_element_child()
		.expect("input")
		.unchecked_into();
	(root, input, value, error)
}

fn dispatch_keydown(input: &web_sys::HtmlInputElement, key: &str, shift: bool) {
	dispatch_keydown_with_modifiers(input, key, shift, false, false, false);
}

fn dispatch_keydown_with_modifiers(
	input: &web_sys::HtmlInputElement,
	key: &str,
	shift: bool,
	ctrl: bool,
	alt: bool,
	meta: bool,
) {
	let init = web_sys::KeyboardEventInit::new();
	init.set_key(key);
	init.set_shift_key(shift);
	init.set_ctrl_key(ctrl);
	init.set_alt_key(alt);
	init.set_meta_key(meta);
	input
		.dispatch_event(
			&web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init)
				.expect("keydown"),
		)
		.expect("dispatch keydown");
}

#[wasm_bindgen_test]
fn number_binding_tracks_keyboard_caret_for_sanitized_input() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(12);
		dispatch_keydown(&input, "ArrowLeft", false);
		dispatch_before_input(&input, Some("-"), "insertText");
		input.set_value("");
		dispatch_input(&input, Some("-"), "insertText");

		assert_eq!(value.get(), 12);
		let parse_error = error.get().expect("invalid middle insertion");
		assert_eq!(parse_error.raw(), "1-2");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Invalid);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_tracks_shift_selection_for_sanitized_replacement() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(123);
		dispatch_keydown(&input, "ArrowLeft", false);
		dispatch_keydown(&input, "ArrowLeft", true);
		dispatch_before_input(&input, Some("-"), "insertReplacementText");
		input.set_value("");
		dispatch_input(&input, Some("-"), "insertReplacementText");

		assert_eq!(value.get(), 123);
		let parse_error = error.get().expect("invalid selection replacement");
		assert_eq!(parse_error.raw(), "1-3");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Invalid);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_resynchronizes_after_an_unknown_valid_pointer_edit() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(12);
		input
			.dispatch_event(&web_sys::MouseEvent::new("mousedown").expect("mousedown"))
			.expect("dispatch mousedown");
		dispatch_before_input(&input, Some("3"), "insertText");
		input.set_value("132");
		dispatch_input(&input, Some("3"), "insertText");
		assert_eq!(value.get(), 132);

		dispatch_before_input(&input, Some("-"), "insertText");
		input.set_value("");
		dispatch_input(&input, Some("-"), "insertText");

		assert_eq!(value.get(), 132);
		let parse_error = error.get().expect("invalid edit after resync");
		assert_eq!(parse_error.raw(), "13-2");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_tracks_word_and_line_deletes() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(1234);
		dispatch_keydown(&input, "ArrowLeft", false);
		dispatch_keydown(&input, "ArrowLeft", false);
		dispatch_before_input(&input, None, "deleteWordBackward");
		input.set_value("34");
		dispatch_input(&input, None, "deleteWordBackward");
		assert_eq!(value.get(), 34);

		dispatch_before_input(&input, None, "deleteSoftLineForward");
		input.set_value("");
		dispatch_input(&input, None, "deleteSoftLineForward");
		assert_eq!(value.get(), 34);
		assert_eq!(error.get().expect("empty line delete").raw(), "");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_uses_transfer_data_for_sanitized_paste() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(12);
		dispatch_before_input_with_transfer(&input, "-", "insertFromPaste");
		input.set_value("");
		dispatch_input(&input, None, "insertFromPaste");

		assert_eq!(value.get(), 12);
		let parse_error = error.get().expect("invalid pasted raw");
		assert_eq!(parse_error.raw(), "12-");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Invalid);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_does_not_invent_raw_after_an_unknown_pointer_edit() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(12);
		input
			.dispatch_event(&web_sys::MouseEvent::new("mousedown").expect("mousedown"))
			.expect("dispatch mousedown");
		dispatch_before_input(&input, Some("-"), "insertText");
		input.set_value("");
		dispatch_input(&input, Some("-"), "insertText");

		assert_eq!(value.get(), 12);
		let parse_error = error.get().expect("sanitized fallback");
		assert_eq!(parse_error.raw(), "");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Empty);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_does_not_predict_modifier_key_selection() {
	ReactiveScope::run(|| {
		let (_root, input, value, error) = mounted_number_control(12);
		dispatch_keydown_with_modifiers(&input, "a", false, true, false, false);
		dispatch_before_input(&input, Some("-"), "insertText");
		input.set_value("");
		dispatch_input(&input, Some("-"), "insertText");

		assert_eq!(value.get(), 12);
		let parse_error = error.get().expect("unknown modifier selection");
		assert_eq!(parse_error.raw(), "");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Empty);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_recovers_incomplete_raw_from_sanitized_browser_input() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new(7_i32);
		let error = Signal::new(None::<NumberParseError>);
		page!({
			input {
				a11y: off,
				type: "number",
				bind: number(value, error),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		dispatch_before_input(&input, None, "deleteContentBackward");
		input.set_value("");
		dispatch_input(&input, None, "deleteContentBackward");
		dispatch_before_input(&input, Some("-"), "insertText");
		input.set_value("-");
		dispatch_input(&input, Some("-"), "insertText");

		assert_eq!(input.value(), "", "Chrome sanitizes an incomplete number");
		assert_eq!(value.get(), 7);
		let parse_error = error.get().expect("incomplete raw should be reported");
		assert_eq!(parse_error.raw(), "-");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Incomplete);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn rejected_number_raw_survives_an_error_driven_reactive_remount() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new(7_i32);
		let error = Signal::new(None::<NumberParseError>);
		let render_value = value.clone();
		let render_error = error.clone();
		Page::reactive(move || {
			let input = PageElement::new("input")
				.attr("type", "number")
				.control_binding(ControlBinding::number_with_error(
					render_value.clone(),
					render_error.clone(),
				))
				.into_page();
			let validation = if render_error.get().is_some() {
				PageElement::new("p")
					.attr("id", "number-validation")
					.child("Invalid")
					.into_page()
			} else {
				Page::Empty
			};
			Page::Fragment(vec![input, validation])
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("input")
			.expect("query")
			.expect("input")
			.unchecked_into();

		dispatch_keydown(&input, "Home", false);
		dispatch_keydown(&input, "End", true);
		dispatch_before_input(&input, Some("1e-"), "insertText");
		input.set_value("");
		dispatch_input(&input, Some("1e-"), "insertText");

		assert_eq!(value.get(), 7);
		assert_eq!(error.get().expect("rejected number").raw(), "1e-");
		assert!(
			root.as_web_sys()
				.query_selector("#number-validation")
				.expect("query")
				.is_some()
		);
		let replacement: web_sys::HtmlInputElement = root
			.as_web_sys()
			.query_selector("input")
			.expect("query")
			.expect("replacement")
			.unchecked_into();
		dispatch_before_input(&replacement, Some("0"), "insertText");
		replacement.set_value("");
		dispatch_input(&replacement, Some("0"), "insertText");

		assert_eq!(value.get(), 1);
		assert_eq!(error.get(), None);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn number_binding_deduplicates_sanitized_final_input_after_composition() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new(7_i32);
		let error = Signal::new(None::<NumberParseError>);
		page!({
			input {
				a11y: off,
				type: "number",
				bind: number(value, error),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		dispatch_keydown(&input, "Home", false);
		dispatch_keydown(&input, "End", true);
		input
			.dispatch_event(
				&web_sys::CompositionEvent::new("compositionstart").expect("compositionstart"),
			)
			.expect("dispatch compositionstart");
		dispatch_before_input(&input, Some("-"), "insertCompositionText");
		input.set_value("-");
		dispatch_input(&input, Some("-"), "insertCompositionText");
		input
			.dispatch_event(
				&web_sys::CompositionEvent::new("compositionend").expect("compositionend"),
			)
			.expect("dispatch compositionend");
		dispatch_input(&input, Some("-"), "insertCompositionText");

		assert_eq!(value.get(), 7);
		assert_eq!(input.value(), "");
		let parse_error = error
			.get()
			.expect("duplicate final input should retain the incomplete raw");
		assert_eq!(parse_error.raw(), "-");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Incomplete);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn rejected_number_composition_reprojects_a_reentrant_signal_change() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new(7_i32);
		let end_value = value.clone();
		let error = Signal::new(None::<NumberParseError>);
		page!({
			input {
				a11y: off,
				type: "number",
				bind: number(value, error),
				@compositionend: move |_| end_value.set(42),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();

		input
			.dispatch_event(&web_sys::CompositionEvent::new("compositionstart").expect("start"))
			.expect("dispatch");
		input.set_value("-");
		dispatch_input(&input, Some("-"), "insertCompositionText");
		input
			.dispatch_event(&web_sys::CompositionEvent::new("compositionend").expect("end"))
			.expect("dispatch");
		assert_eq!(value.get(), 42);
		input.set_value("-");

		dispatch_input(&input, Some("-"), "insertCompositionText");

		assert_eq!(value.get(), 42);
		assert_eq!(input.value(), "42");
		let parse_error = error.get().expect("sanitized value should be rejected");
		assert_eq!(parse_error.raw(), "");
		assert_eq!(parse_error.kind(), NumberParseErrorKind::Empty);
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn duplicate_final_input_reprojects_a_reentrant_compositionend_signal_change() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let value = Signal::new("old".to_owned());
		let end_value = value.clone();
		page!({
			input {
				a11y: off,
				bind: value,
				@compositionend: move |_| end_value.set("after-end".to_owned()),
			}
		})
		.mount(&root)
		.expect("mount");
		let input: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("input")
			.unchecked_into();
		input
			.dispatch_event(&web_sys::CompositionEvent::new("compositionstart").expect("start"))
			.expect("dispatch");
		input.set_value("かな");
		input
			.dispatch_event(&web_sys::CompositionEvent::new("compositionend").expect("end"))
			.expect("dispatch");
		assert_eq!(value.get(), "after-end");
		input.set_value("かな");

		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("input"))
			.expect("dispatch");

		assert_eq!(value.get(), "after-end");
		assert_eq!(input.value(), "after-end");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn reactive_remount_drops_the_replaced_control_owner() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let root = Element::new(document.create_element("div").expect("root"));
		let alternate = Signal::new(false);
		let value = Signal::new("initial".to_owned());
		let render_alternate = alternate.clone();
		let render_value = value.clone();
		Page::reactive(move || {
			let bound = render_value.clone();
			if render_alternate.get() {
				page!({
					input {
						a11y: off,
						id: "replacement",
						bind: bound,
					}
				})
			} else {
				page!({
					input {
						a11y: off,
						id: "original",
						bind: bound,
					}
				})
			}
		})
		.mount(&root)
		.expect("mount");
		let original: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("original")
			.unchecked_into();

		alternate.set(true);
		let replacement: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("replacement")
			.unchecked_into();
		assert_eq!(replacement.id(), "replacement");
		original.set_value("stale");
		original
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "initial");
		value.set("current".to_owned());
		assert_eq!(replacement.value(), "current");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

struct HydratedInput {
	value: Signal<String>,
	observed: Rc<RefCell<String>>,
}

struct HydratedDateTimeInput {
	value: Signal<String>,
}

impl Component for HydratedDateTimeInput {
	fn name() -> &'static str {
		"HydratedDateTimeInput"
	}

	fn render(&self) -> Page {
		PageElement::new("div")
			.child(
				PageElement::new("input")
					.attr("type", "datetime-local")
					.control_binding(ControlBinding::text(self.value.clone())),
			)
			.into_page()
	}
}

struct SourcePreferredHydratedInput {
	value: Signal<String>,
}

struct HydratedInputAfterText {
	value: Signal<String>,
}

impl Component for HydratedInputAfterText {
	fn name() -> &'static str {
		"HydratedInputAfterText"
	}

	fn render(&self) -> Page {
		PageElement::new("div")
			.child("Label")
			.child(
				PageElement::new("input").control_binding(ControlBinding::text(self.value.clone())),
			)
			.into_page()
	}
}

struct FailingHydrationRoot {
	trigger: Signal<u32>,
	render_count: Rc<Cell<u32>>,
	listener_count: Rc<Cell<u32>>,
}

struct FailingHydrationAfterControls {
	text: Signal<String>,
	number: Signal<i32>,
	number_error: Signal<Option<NumberParseError>>,
}

impl Component for FailingHydrationAfterControls {
	fn name() -> &'static str {
		"FailingHydrationAfterControls"
	}

	fn render(&self) -> Page {
		PageElement::new("div")
			.child(
				PageElement::new("input").control_binding(ControlBinding::text(self.text.clone())),
			)
			.child(
				PageElement::new("input")
					.attr("type", "number")
					.control_binding(ControlBinding::number_with_error(
						self.number.clone(),
						self.number_error.clone(),
					)),
			)
			.child(Page::Element(
				PageElement::new("select")
					.control_binding(ControlBinding::checkbox(Signal::new(false))),
			))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn failed_root_hydration_does_not_adopt_earlier_control_values() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_text: web_sys::HtmlInputElement = document
			.create_element("input")
			.expect("text")
			.unchecked_into();
		raw_text.set_value("restored");
		raw_root.append_child(&raw_text).expect("text input");
		let raw_number: web_sys::HtmlInputElement = document
			.create_element("input")
			.expect("number")
			.unchecked_into();
		raw_number.set_type("number");
		raw_number.set_value("2147483648");
		raw_root.append_child(&raw_number).expect("number input");
		raw_root
			.append_child(&document.create_element("select").expect("select"))
			.expect("invalid sibling");
		let root = Element::new(raw_root);
		let text = Signal::new("server".to_owned());
		let number = Signal::new(7_i32);
		let number_error = Signal::new(None::<NumberParseError>);
		ControlBinding::number_with_error(number.clone(), number_error.clone())
			.write(ControlValue::Text("pending".to_owned()))
			.expect("seed number error");
		let original_number_error = number_error.get();
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&FailingHydrationAfterControls {
				text: text.clone(),
				number: number.clone(),
				number_error: number_error.clone(),
			},
			&root,
		)
		.expect_err("later invalid binding");

		assert_eq!(text.get(), "server");
		assert_eq!(number.get(), 7);
		assert_eq!(number_error.get(), original_number_error);
	});
}

impl Component for FailingHydrationRoot {
	fn name() -> &'static str {
		"FailingHydrationRoot"
	}

	fn render(&self) -> Page {
		let trigger = self.trigger.clone();
		let render_count = Rc::clone(&self.render_count);
		let listener_count = Rc::clone(&self.listener_count);
		PageElement::new("div")
			.child(Page::reactive(move || {
				let _ = trigger.get();
				render_count.set(render_count.get() + 1);
				let listener_count = Rc::clone(&listener_count);
				page!({
					button {
						id: "reactive-sibling",
						@input: move |_| listener_count.set(listener_count.get() + 1),
						"ready"
					}
				})
			}))
			.child(Page::Element(
				PageElement::new("select")
					.control_binding(ControlBinding::checkbox(Signal::new(false))),
			))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn failed_root_hydration_rolls_back_earlier_reactive_siblings() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_button = document.create_element("button").expect("button");
		raw_button.set_id("reactive-sibling");
		raw_button.set_text_content(Some("ready"));
		raw_root
			.append_child(&raw_button)
			.expect("reactive sibling");
		let raw_select = document.create_element("select").expect("select");
		raw_root.append_child(&raw_select).expect("invalid sibling");
		let root = Element::new(raw_root.clone());
		let trigger = Signal::new(0_u32);
		let render_count = Rc::new(Cell::new(0));
		let listener_count = Rc::new(Cell::new(0));
		let _state = SsrStateElement::install(&document);

		let error = reinhardt_pages::hydration::hydrate(
			&FailingHydrationRoot {
				trigger: trigger.clone(),
				render_count: Rc::clone(&render_count),
				listener_count: Rc::clone(&listener_count),
			},
			&root,
		)
		.expect_err("later invalid binding");
		let renders_after_failure = render_count.get();
		raw_button
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		trigger.set(1);
		with_runtime(|runtime| runtime.flush_updates());
		let marker_count = (0..raw_root.child_nodes().length())
			.filter_map(|index| raw_root.child_nodes().item(index))
			.filter(|node| node.node_type() == web_sys::Node::COMMENT_NODE)
			.count();

		assert_eq!(
			error.to_string(),
			"Event attachment failed: checkbox control does not support a <select> element"
		);
		assert_eq!(
			(render_count.get(), listener_count.get(), marker_count),
			(renders_after_failure, 0, 0)
		);
	});
}

impl Component for HydratedInput {
	fn name() -> &'static str {
		"HydratedInput"
	}

	fn render(&self) -> Page {
		let value = self.value.clone();
		let handler_value = self.value.clone();
		let handler_observed = Rc::clone(&self.observed);
		page!({
			input {
				a11y: off,
				bind: value,
				@input: move |_| *handler_observed.borrow_mut() = handler_value.get(),
			}
		})
	}
}

impl Component for SourcePreferredHydratedInput {
	fn name() -> &'static str {
		"SourcePreferredHydratedInput"
	}

	fn render(&self) -> Page {
		PageElement::new("input")
			.control_binding(
				ControlBinding::text(self.value.clone()).prefer_source_on_hydration(|| true),
			)
			.into_page()
	}
}

struct ScopeAllocatingHydratedInput;

impl Component for ScopeAllocatingHydratedInput {
	fn name() -> &'static str {
		"ScopeAllocatingHydratedInput"
	}

	fn render(&self) -> Page {
		PageElement::new("input")
			.control_binding(ControlBinding::text(Signal::new("server".to_owned())))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn hydration_retains_a_scope_for_component_created_signals() {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let raw = document.create_element("input").expect("input");
	let input: web_sys::HtmlInputElement = raw.clone().unchecked_into();
	input.set_value("server");
	let root = Element::new(raw);
	let _state = SsrStateElement::install(&document);

	reinhardt_pages::hydration::hydrate(&ScopeAllocatingHydratedInput, &root)
		.expect("hydrate component-created signal");

	reinhardt_pages::cleanup_reactive_nodes();
}

struct HydratedControlledSelect {
	selected: Signal<String>,
}

impl Component for HydratedControlledSelect {
	fn name() -> &'static str {
		"HydratedControlledSelect"
	}

	fn render(&self) -> Page {
		PageElement::new("select")
			.control_binding(ControlBinding::select_one(self.selected.clone()))
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.attr("selected", "selected")
					.child("Rust"),
			)
			.child(
				PageElement::new("option")
					.attr("value", "wasm")
					.child("WebAssembly"),
			)
			.into_page()
	}
}

#[wasm_bindgen_test]
fn hydration_ignores_raw_selected_attributes_inside_a_controlled_select() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_select = document.create_element("select").expect("select");
		let raw_rust = document.create_element("option").expect("rust option");
		raw_rust.set_attribute("value", "rust").expect("rust value");
		raw_rust.set_text_content(Some("Rust"));
		let raw_wasm = document.create_element("option").expect("wasm option");
		raw_wasm.set_attribute("value", "wasm").expect("wasm value");
		raw_wasm
			.set_attribute("selected", "selected")
			.expect("wasm selection");
		raw_wasm.set_text_content(Some("WebAssembly"));
		raw_select.append_child(&raw_rust).expect("rust append");
		raw_select.append_child(&raw_wasm).expect("wasm append");
		let root = Element::new(raw_select);
		let selected = Signal::new("wasm".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedControlledSelect {
				selected: selected.clone(),
			},
			&root,
		)
		.expect("hydrate controlled select");

		assert_eq!(selected.get(), "wasm");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_hydration_adopts_the_live_dom_property() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw.clone().unchecked_into();
		input.set_value("restored");
		let root = Element::new(raw);
		let value = Signal::new("server".to_owned());
		let observed = Rc::new(RefCell::new(String::new()));
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedInput {
				value: value.clone(),
				observed: Rc::clone(&observed),
			},
			&root,
		)
		.expect("hydrate");

		assert_eq!(value.get(), "restored");
		input.set_value("edited");
		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "edited");
		assert_eq!(&*observed.borrow(), "edited");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn public_hydration_adopts_datetime_local_input_value() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_input = document.create_element("input").expect("input");
		raw_input
			.set_attribute("type", "datetime-local")
			.expect("input type");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		input.set_value("2026-08-31T10:30");
		raw_root.append_child(&raw_input).expect("input append");
		let root = Element::new(raw_root);
		let value = Signal::new("server datetime".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedDateTimeInput {
				value: value.clone(),
			},
			&root,
		)
		.expect("hydrate datetime-local input");

		assert_eq!(value.get(), "2026-08-31T10:30");
		assert!(raw_input.is_same_node(root.as_web_sys().first_element_child().as_deref()));
		input.set_value("2026-08-31T11:45");
		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "2026-08-31T11:45");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

#[wasm_bindgen_test]
fn source_preferred_hydration_restores_runtime_value_in_place() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw.clone().unchecked_into();
		input.set_value("stale live input");
		let node_before = input.clone();
		let root = Element::new(raw);
		let value = Signal::new("reset value".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&SourcePreferredHydratedInput {
				value: value.clone(),
			},
			&root,
		)
		.expect("hydrate source-preferred input");

		assert_eq!(value.get(), "reset value");
		assert_eq!(input.value(), "reset value");
		assert!(node_before.is_same_node(Some(&input)));
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

struct HydratedRadioForm(Page);

impl Component for HydratedRadioForm {
	fn name() -> &'static str {
		"HydratedRadioForm"
	}

	fn render(&self) -> Page {
		self.0.clone()
	}
}

impl Drop for HydratedRadioForm {
	fn drop(&mut self) {
		reinhardt_pages::cleanup_reactive_nodes();
	}
}

#[wasm_bindgen_test]
fn generated_radio_hydration_preserves_live_selection_and_runtime_reset() {
	for (edit_selection, reset_before_hydration, expected) in [
		(true, false, ["b", "a"]),
		(true, true, ["a", "b"]),
		(false, false, ["a", "b"]),
	] {
		ReactiveScope::run(|| {
			// Arrange: both groups use the same choice values in opposite SSR states.
			let form = reinhardt_pages::form! {
				name: RadioHydrationForm,
				action: "/radio-hydration",
				method: Get,
				fields: {
					first: ChoiceField<String> {
						bind: true,
						initial: "a",
						widget: RadioSelect,
						choices_from: "choices",
						choice_value: "value",
						choice_label: "label",
					},
					second: ChoiceField<String> {
						bind: true,
						initial: "b",
						widget: RadioSelect,
						choices_from: "choices",
						choice_value: "value",
						choice_label: "label",
					},
				},
			};
			let choices = vec![
				("a".to_owned(), "A".to_owned()),
				("b".to_owned(), "B".to_owned()),
			];
			form.first_choices.set(choices.clone());
			form.second_choices.set(choices);
			let runtime = reinhardt_pages::use_form(&form).build();
			let component = HydratedRadioForm(
				PageElement::new("div")
					.child(form.clone().into_page())
					.into_page(),
			);
			let document = web_sys::window()
				.expect("window")
				.document()
				.expect("document");
			let wrapper = document.create_element("div").expect("SSR wrapper");
			wrapper.set_inner_html(&component.render().render_to_string());
			let root = Element::new(wrapper.first_element_child().expect("SSR root"));
			let inputs = || {
				let nodes = root
					.as_web_sys()
					.query_selector_all("input[type=radio]")
					.expect("radio inputs");
				(0..nodes.length())
					.map(|index| {
						nodes
							.item(index)
							.expect("radio input")
							.unchecked_into::<web_sys::HtmlInputElement>()
					})
					.collect::<Vec<_>>()
			};
			let original_inputs = inputs();
			assert_eq!(original_inputs.len(), 4);
			for input in &original_inputs {
				input.set_checked(false);
			}
			if edit_selection {
				original_inputs[1].set_checked(true);
				original_inputs[2].set_checked(true);
			}
			if reset_before_hydration {
				runtime.reset();
			}
			let _state = SsrStateElement::install(&document);

			// Act
			reinhardt_pages::hydration::hydrate(&component, &root).expect("hydrate radio form");

			// Assert: later and earlier choices are adopted independently, unless reset wins.
			assert_eq!(
				[form.first.get(), form.second.get()],
				expected.map(str::to_owned)
			);
			assert_eq!(
				inputs()
					.iter()
					.map(|input| input.checked())
					.collect::<Vec<_>>(),
				vec![
					expected[0] == "a",
					expected[0] == "b",
					expected[1] == "a",
					expected[1] == "b"
				],
			);
			let hydrated_inputs = inputs();
			runtime.reset();
			assert_eq!(
				[form.first.get(), form.second.get()],
				["a".to_owned(), "b".to_owned()]
			);
			assert_eq!(
				inputs()
					.iter()
					.map(|input| input.checked())
					.collect::<Vec<_>>(),
				vec![true, false, false, true],
			);
			assert_eq!(
				hydrated_inputs
					.iter()
					.zip(inputs())
					.map(|(before, after)| before.is_same_node(Some(&after)))
					.collect::<Vec<_>>(),
				vec![true; 4],
			);
		});
	}
}

#[wasm_bindgen_test]
fn hydration_attaches_controls_after_text_siblings() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		raw_root
			.append_child(&document.create_text_node("Label"))
			.expect("label");
		let raw_input = document.create_element("input").expect("input");
		let input: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		input.set_value("restored");
		raw_root.append_child(&raw_input).expect("input child");
		let root = Element::new(raw_root);
		let value = Signal::new("server".to_owned());
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedInputAfterText {
				value: value.clone(),
			},
			&root,
		)
		.expect("hydrate control after text sibling");

		assert_eq!(value.get(), "restored");
		input.set_value("edited");
		input
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "edited");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

struct HydratedReactiveInput {
	alternate: Signal<bool>,
	value: Signal<String>,
	observed: Rc<RefCell<String>>,
}

impl Component for HydratedReactiveInput {
	fn name() -> &'static str {
		"HydratedReactiveInput"
	}

	fn render(&self) -> Page {
		let alternate = self.alternate.clone();
		let value = self.value.clone();
		let observed = Rc::clone(&self.observed);
		PageElement::new("div")
			.child(Page::reactive(move || {
				let _rendered_value = value.get();
				let bound = value.clone();
				let handler_value = value.clone();
				let handler_observed = Rc::clone(&observed);
				let id = if alternate.get() {
					"replacement"
				} else {
					"original"
				};
				page!({
					input {
						a11y: off,
						id: id,
						bind: bound,
						@change: move |_| {
							*handler_observed.borrow_mut() = handler_value.get();
						},
					}
				})
			}))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn hydrated_reactive_switch_drops_the_initial_branch_guards() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_id("original");
		let original: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		original.set_value("restored");
		raw_root.append_child(&raw_input).expect("SSR child");
		let root = Element::new(raw_root);
		let alternate = Signal::new(false);
		let value = Signal::new("server".to_owned());
		let observed = Rc::new(RefCell::new(String::new()));
		let _state = SsrStateElement::install(&document);
		reinhardt_pages::hydration::hydrate(
			&HydratedReactiveInput {
				alternate: alternate.clone(),
				value: value.clone(),
				observed: Rc::clone(&observed),
			},
			&root,
		)
		.expect("hydrate");
		assert_eq!(value.get(), "restored");
		assert!(raw_input.is_same_node(root.as_web_sys().first_element_child().as_deref(),));

		alternate.set(true);
		let replacement: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("replacement")
			.unchecked_into();
		assert_eq!(replacement.id(), "replacement");
		original.set_value("stale");
		original
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		original
			.dispatch_event(&web_sys::Event::new("change").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "restored");
		assert_eq!(&*observed.borrow(), "");

		value.set("fresh".to_owned());
		assert_eq!(original.value(), "stale");
		let fresh_control: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("fresh control")
			.unchecked_into();
		assert_eq!(fresh_control.value(), "fresh");
		fresh_control
			.dispatch_event(&web_sys::Event::new("change").expect("event"))
			.expect("dispatch");
		assert_eq!(&*observed.borrow(), "fresh");
		fresh_control.set_value("new branch");
		fresh_control
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "new branch");
		assert_eq!(&*observed.borrow(), "fresh");
		let live_control: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("live control")
			.unchecked_into();
		assert_eq!(live_control.value(), "new branch");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}

struct HydratedReactiveIfInput {
	alternate: Signal<bool>,
	value: Signal<String>,
	observed: Rc<RefCell<String>>,
}

impl Component for HydratedReactiveIfInput {
	fn name() -> &'static str {
		"HydratedReactiveIfInput"
	}

	fn render(&self) -> Page {
		let condition_alternate = self.alternate.clone();
		let condition_value = self.value.clone();
		let primary_value = self.value.clone();
		let primary_observed = Rc::clone(&self.observed);
		let replacement_value = self.value.clone();
		let replacement_observed = Rc::clone(&self.observed);
		PageElement::new("div")
			.child(Page::reactive_if(
				move || condition_alternate.get() || condition_value.get() == "server",
				move || {
					let bound = primary_value.clone();
					let handler_value = primary_value.clone();
					let handler_observed = Rc::clone(&primary_observed);
					page!({
						input {
							a11y: off,
							id: "primary",
							bind: bound,
							@input: move |_| {
								*handler_observed.borrow_mut() = handler_value.get();
							},
						}
					})
				},
				move || {
					let bound = replacement_value.clone();
					let handler_value = replacement_value.clone();
					let handler_observed = Rc::clone(&replacement_observed);
					page!({
						input {
							a11y: off,
							id: "replacement",
							bind: bound,
							@input: move |_| {
								*handler_observed.borrow_mut() = handler_value.get();
							},
						}
					})
				},
			))
			.into_page()
	}
}

#[wasm_bindgen_test]
fn hydrated_reactive_if_adopts_before_subscribing_and_transfers_guards() {
	ReactiveScope::run(|| {
		let document = web_sys::window()
			.expect("window")
			.document()
			.expect("document");
		let raw_root = document.create_element("div").expect("root");
		let raw_input = document.create_element("input").expect("input");
		raw_input.set_id("primary");
		let primary: web_sys::HtmlInputElement = raw_input.clone().unchecked_into();
		primary.set_value("restored");
		raw_root.append_child(&raw_input).expect("SSR child");
		let root = Element::new(raw_root);
		let alternate = Signal::new(false);
		let value = Signal::new("server".to_owned());
		let observed = Rc::new(RefCell::new(String::new()));
		let _state = SsrStateElement::install(&document);

		reinhardt_pages::hydration::hydrate(
			&HydratedReactiveIfInput {
				alternate: alternate.clone(),
				value: value.clone(),
				observed: Rc::clone(&observed),
			},
			&root,
		)
		.expect("hydrate");
		assert_eq!(value.get(), "restored");
		assert!(raw_input.is_same_node(root.as_web_sys().first_element_child().as_deref(),));
		with_runtime(|runtime| runtime.flush_updates());
		let converged: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("converged false branch")
			.unchecked_into();
		assert_eq!(converged.id(), "replacement");
		assert!(!raw_input.is_same_node(Some(&converged)));
		primary.set_value("stale after convergence");
		primary
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "restored");
		assert_eq!(&*observed.borrow(), "");

		alternate.set(true);
		let switched: web_sys::HtmlInputElement = root
			.as_web_sys()
			.first_element_child()
			.expect("switched branch")
			.unchecked_into();
		assert_eq!(switched.id(), "primary");
		converged.set_value("stale");
		converged
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "restored");
		assert_eq!(&*observed.borrow(), "");

		value.set("fresh".to_owned());
		assert_eq!(primary.value(), "stale after convergence");
		assert_eq!(switched.value(), "fresh");
		switched.set_value("new branch");
		switched
			.dispatch_event(&web_sys::InputEvent::new("input").expect("event"))
			.expect("dispatch");
		assert_eq!(value.get(), "new branch");
		assert_eq!(&*observed.borrow(), "new branch");
		reinhardt_pages::cleanup_reactive_nodes();
	});
}
