//! Browser regressions for parser-sensitive form defaults and selective hydration.
//!
//! Run with `wasm-pack test crates/reinhardt-pages --headless --chrome --test form_control_hydration_regressions`.

#![cfg(target_arch = "wasm32")]

use reinhardt_pages::component::{
	ControlBinding, ControlKind, ControlValue, IntoPage, PageElement, PageExt,
	cleanup_reactive_nodes,
};
use reinhardt_pages::dom::Element;
use reinhardt_pages::hydration::{
	ReconcileError, ReconcileOptions, attach_events_to_mounted_view, reconcile,
	reconcile_with_options,
};
use reinhardt_pages::reactive::{Signal, with_runtime};
use reinhardt_pages::{Page, form, use_form};
use serial_test::serial;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct MountedPage(web_sys::Element);

impl MountedPage {
	fn new() -> Self {
		let document = web_sys::window().unwrap().document().unwrap();
		let root = Self(document.create_element("div").unwrap());
		document.body().unwrap().append_child(&root.0).unwrap();
		root
	}

	fn root(&self) -> Element {
		Element::new(self.0.first_element_child().unwrap())
	}
}

impl Drop for MountedPage {
	fn drop(&mut self) {
		cleanup_reactive_nodes();
		self.0.remove();
	}
}

fn flush() {
	with_runtime(|runtime| runtime.flush_updates());
}

#[rstest::rstest]
#[test_attr(wasm_bindgen_test)]
#[serial(form_control_hydration_dom)]
async fn textarea_leading_newlines_survive_parsing_hydration_and_reset() {
	for value in ["", "notes", "\n", "\nnotes", "\n\nnotes", "\n<&"] {
		// Arrange: the browser parses the actual generated SSR markup.
		let form = form! {
			name: TextareaParserDefaults,
			action: "/notes",
			fields: {
				notes: TextField {
					initial: "\nnotes"
				}
			}
		};
		form.notes().set(String::from(value));
		let page = form.clone().into_page();
		let mounted = MountedPage::new();
		mounted.0.set_inner_html(&page.render_to_string());
		let textarea = mounted
			.0
			.query_selector("textarea")
			.unwrap()
			.unwrap()
			.unchecked_into::<web_sys::HtmlTextAreaElement>();
		assert_eq!(textarea.value(), value);
		assert_eq!(textarea.default_value().unwrap(), value);

		// Act: hydrate, then change the live property before a native reset.
		assert_eq!(reconcile(&mounted.root(), &page), Ok(()));
		attach_events_to_mounted_view(&mounted.root(), &page).unwrap();
		flush();
		assert_eq!(form.notes().get(), value);
		textarea.set_value("changed");
		textarea
			.dispatch_event(&web_sys::Event::new("input").unwrap())
			.unwrap();
		assert_eq!(form.notes().get(), "changed");
		mounted
			.root()
			.as_web_sys()
			.unchecked_ref::<web_sys::HtmlFormElement>()
			.reset();
		gloo_timers::future::TimeoutFuture::new(0).await;
		flush();

		// Assert: both the live value and the reset default retain every line feed.
		assert_eq!(textarea.value(), value);
		assert_eq!(textarea.default_value().unwrap(), value);
		assert_eq!(form.notes().get(), value);
		drop(mounted);

		// Direct mounting consumes the original view text without SSR padding.
		let mounted = MountedPage::new();
		page.mount(&Element::new(mounted.0.clone())).unwrap();
		flush();
		let textarea = mounted
			.0
			.query_selector("textarea")
			.unwrap()
			.unwrap()
			.unchecked_into::<web_sys::HtmlTextAreaElement>();
		assert_eq!(textarea.value(), value);
		assert_eq!(textarea.default_value().unwrap(), value);
	}
}

#[rstest::rstest]
#[test_attr(wasm_bindgen_test)]
#[serial(form_control_hydration_dom)]
fn selective_reconciliation_preserves_reset_select_defaults() {
	// Arrange: both select kinds have SSR defaults superseded by a client reset.
	let form = form! {
		name: SelectReconciliationDefaults,
		action: "/status",
		fields: {
			status: ChoiceField<String> {
				initial: "review",
				choices: [("open", "Open"), ("review", "Review")]
			}
			tags: MultipleChoiceField<String> {
				initial: vec![String::from("rust"), String::from("wasm")],
				choices: [("rust", "Rust"), ("web", "Web"), ("wasm", "WASM")]
			}
		}
	};
	let runtime = use_form(&form).build();
	form.status().set(String::from("open"));
	form.tags().set(vec![String::from("web")]);
	let mounted = MountedPage::new();
	mounted
		.0
		.set_inner_html(&form.clone().into_page().render_to_string());
	let select = mounted.0.query_selector("#status").unwrap().unwrap();
	assert_eq!(
		select.unchecked_ref::<web_sys::HtmlSelectElement>().value(),
		"open"
	);
	runtime.reset();
	let page = form.clone().into_page();

	// Act: options-aware traversal revisits the select options independently.
	let options = ReconcileOptions::full_reconciliation().warn_on_mismatch(false);
	assert_eq!(
		reconcile_with_options(&mounted.root(), &page, &options),
		Ok(())
	);
	attach_events_to_mounted_view(&mounted.root(), &page).unwrap();
	flush();

	// Assert: binding hydration applies reset values to the original nodes.
	assert_eq!(
		select.unchecked_ref::<web_sys::HtmlSelectElement>().value(),
		"review"
	);
	assert_eq!(form.status().get(), "review");
	assert_eq!(form.tags().get(), ["rust", "wasm"]);
	let selected = mounted
		.0
		.query_selector_all("#tags option:checked")
		.unwrap();
	let values = (0..selected.length())
		.map(|index| {
			selected
				.item(index)
				.unwrap()
				.unchecked_into::<web_sys::HtmlOptionElement>()
				.value()
		})
		.collect::<Vec<_>>();
	assert_eq!(values, ["rust", "wasm"]);
	assert_eq!(
		mounted.0.query_selector("#status").unwrap().unwrap(),
		select
	);
}

#[rstest::rstest]
#[test_attr(wasm_bindgen_test)]
#[serial(form_control_hydration_dom)]
fn island_reconciliation_inherits_only_ancestor_controlled_selection() {
	// Arrange: the island sits below a controlled select, inside an option group.
	let value = Signal::new(String::from("open"));
	let render = || {
		let read = value.clone();
		let write = value.clone();
		PageElement::new("select")
			.control_binding(
				ControlBinding::from_parts(
					ControlKind::SelectOne,
					move || ControlValue::Text(read.get()),
					move |raw| {
						if let ControlValue::Text(value) = raw {
							write.set(value);
						}
					},
				)
				.prefer_source_on_hydration(|| true),
			)
			.child(
				PageElement::new("optgroup")
					.attr("label", "Status")
					.attr("data-rh-island", "true")
					.child(Page::keyed_fragment(["open", "review"].map(|option| {
						(
							option,
							PageElement::new("option")
								.attr("value", option)
								.bool_attr("selected", value.get() == option)
								.child(option),
						)
					}))),
			)
			.into_page()
	};
	let mounted = MountedPage::new();
	mounted.0.set_inner_html(&render().render_to_string());
	value.set(String::from("review"));
	let page = render();
	let options = ReconcileOptions::island_only().warn_on_mismatch(false);

	// Act and assert: the island inherits the select binding before hydration.
	assert_eq!(
		reconcile_with_options(&mounted.root(), &page, &options),
		Ok(())
	);
	attach_events_to_mounted_view(&mounted.root(), &page).unwrap();
	flush();
	assert_eq!(
		mounted
			.root()
			.as_web_sys()
			.unchecked_ref::<web_sys::HtmlSelectElement>()
			.value(),
		"review"
	);

	// An unrelated unbound island still validates its selected attribute.
	let unbound = MountedPage::new();
	unbound
		.0
		.set_inner_html("<select data-rh-island=\"true\"><option>Review</option></select>");
	let page = PageElement::new("select")
		.attr("data-rh-island", "true")
		.child(
			PageElement::new("option")
				.bool_attr("selected", true)
				.child("Review"),
		)
		.into_page();
	let error = reconcile_with_options(&unbound.root(), &page, &options).unwrap_err();
	let ReconcileError::AttributeMismatch {
		name,
		expected,
		actual,
		..
	} = error
	else {
		panic!("expected selected attribute mismatch, got {error:?}");
	};
	assert_eq!(name, "selected");
	assert_eq!(expected.as_deref(), Some("selected"));
	assert_eq!(actual, None);
}
