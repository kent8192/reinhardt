//! Native markup and browser behavior for a single `form!` radio control.

use reinhardt_pages::{form, use_form};

#[cfg(wasm)]
use wasm_bindgen_test::*;

#[cfg(wasm)]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(wasm, wasm_bindgen_test)]
#[cfg_attr(not(wasm), test)]
fn radio_input_renders_fixed_value_and_reactive_checked_state() {
	// Arrange
	let radio = form! {
		name: RadioForm,
		action: "/answer",
		method: Get,
		fields: {
			answer: ChoiceField<String> {
				widget: RadioInput,
				label: "Answer",
			}
		}
	};
	let page = radio.clone().into_page();
	let unchecked = concat!(
		"<form id=\"radio-form\" action=\"/answer\" method=\"get\" class=\"reinhardt-form\">",
		"<div class=\"reinhardt-field\"><label for=\"answer\" class=\"reinhardt-label\">Answer</label>",
		"<input type=\"radio\" name=\"answer\" id=\"answer\" value=\"on\" class=\"reinhardt-input\" />",
		"</div></form>",
	);

	// Act and assert: an unselected radio retains its fixed submission value.
	assert_eq!(radio.answer().get(), "");
	assert_eq!(page.render_to_string(), unchecked);
	radio.answer().set("on".into());
	assert_eq!(
		page.render_to_string(),
		unchecked.replace(
			"class=\"reinhardt-input\"",
			"class=\"reinhardt-input\" checked=\"checked\""
		),
	);
	radio.answer().set("other".into());
	assert_eq!(page.render_to_string(), unchecked);
}

#[cfg_attr(wasm, wasm_bindgen_test)]
#[cfg_attr(not(wasm), test)]
fn radio_input_renders_option_labels_disabled_and_unbound_defaults() {
	// Arrange
	let radio = form! {
		name: RadioOptions,
		action: "/answer",
		method: Get,
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choices: [("yes", "Yes") { disabled }],
				autocomplete: "off",
				initial: "yes",
				required,
			}
			snapshot: ChoiceField<::std::string::String> {
				widget: RadioInput,
				choices: [("accepted", "Option label")],
				label: "Explicit label",
				initial: "accepted",
				bind: false,
			}
		}
	};
	let page = radio.clone().into_page();
	let expected = concat!(
		"<form id=\"radio-options\" action=\"/answer\" method=\"get\" class=\"reinhardt-form\">",
		"<div class=\"reinhardt-field\"><label for=\"answer\" class=\"reinhardt-label\">Yes</label>",
		"<input type=\"radio\" name=\"answer\" id=\"answer\" value=\"yes\" class=\"reinhardt-input\" autocomplete=\"off\" required=\"required\" disabled=\"disabled\" checked=\"checked\" />",
		"</div><div class=\"reinhardt-field\"><label for=\"snapshot\" class=\"reinhardt-label\">Explicit label</label>",
		"<input type=\"radio\" name=\"snapshot\" id=\"snapshot\" value=\"accepted\" class=\"reinhardt-input\" checked=\"checked\" />",
		"</div></form>",
	);

	// Act and assert: bind:false snapshots checked state when the page is built.
	assert_eq!(page.render_to_string(), expected);
	radio.snapshot().set(String::new());
	assert_eq!(page.render_to_string(), expected);
}

fn input_tags(html: &str) -> Vec<String> {
	html.split("<input ")
		.skip(1)
		.map(|rest| {
			let (attributes, _) = rest.split_once(" />").expect("input tag closes");
			format!("<input {attributes} />")
		})
		.collect()
}

#[cfg_attr(wasm, wasm_bindgen_test)]
#[cfg_attr(not(wasm), test)]
fn collection_radio_input_renders_indexed_names_and_programmatic_values() {
	// Arrange
	let radio = form! {
		name: CollectionRadios,
		action: "/answer",
		method: Get,
		fields: {
			answers: FieldArray {
				fields: {
					answer: ChoiceField<String> {
						widget: RadioInput,
						choices: [("yes", "Yes")],
						autocomplete: "off",
					}
				}
			}
		}
	};
	let runtime = use_form(&radio).build();
	let key = runtime.push_item(radio.answers_collection(), radio.new_answers_item());
	let path = radio.answers_answer_path(key);
	let page = radio.into_page();
	let unchecked = "<input type=\"radio\" name=\"answers[0][answer]\" id=\"answers_0_answer\" value=\"yes\" class=\"reinhardt-input\" autocomplete=\"off\" />";

	// Act and assert
	assert_eq!(input_tags(&page.render_to_string()), [unchecked]);
	runtime.set_path_value(path, String::from("yes"));
	assert_eq!(
		input_tags(&page.render_to_string()),
		[unchecked.replace(" />", " checked=\"checked\" />")],
	);
}

#[cfg_attr(wasm, wasm_bindgen_test)]
#[cfg_attr(not(wasm), test)]
fn native_radio_reset_epoch_clears_runtime_state_after_nested_batches() {
	use reinhardt_pages::{FieldError, RevalidateOn, reactive::batch};
	use std::{cell::Cell, rc::Rc};

	// Arrange: metadata synchronization must compute dirtiness from the final source snapshot.
	let radio = form! {
		name: ResetEpochRadios,
		fields: {
			answer: ChoiceField<String> { widget: RadioInput }
			answers: FieldArray {
				fields: {
					answer: CharField {}
				}
			}
		}
	};
	let runtime = use_form(&radio).revalidate_on(RevalidateOn::Change).build();
	let key = runtime.push_item(radio.answers_collection(), radio.new_answers_item());
	let path = radio.answers_answer_path(key);
	runtime.set_path_value(path.clone(), String::from("kept"));
	runtime.set_path_error(path.clone(), FieldError::new("nested error"));
	runtime.set_value(radio.answer_field(), String::from("on"));
	runtime.set_error(radio.answer_field(), FieldError::new("radio error"));
	let event_count = Rc::new(Cell::new(0));
	let observed_events = Rc::clone(&event_count);
	let _subscription = runtime.subscribe(move |_| observed_events.set(observed_events.get() + 1));

	// Act: the epoch survives the outer batch even when inner reset writes are deferred.
	batch(|| {
		batch(|| {
			radio.answer().set(String::new());
			radio
				.__native_reset_epoch
				.update(|epoch| *epoch = epoch.wrapping_add(1));
		});
	});

	// Assert: native reset preserves unrelated values and clears every touched/error map.
	assert_eq!(runtime.get_values().answer, "");
	assert_eq!(runtime.watch().get().answer, "");
	assert_eq!(runtime.get_values().answers[0].answer, "kept");
	assert!(runtime.form_state().is_dirty.get());
	assert!(!runtime.form_state().is_touched.get());
	assert!(!runtime.get_field_state(radio.answer_field()).is_touched);
	assert!(
		!runtime
			.get_collection_state(radio.answers_collection())
			.is_touched
	);
	assert!(!runtime.get_path_state(path.clone()).is_touched);
	assert_eq!(runtime.get_field_state(radio.answer_field()).error, None);
	assert_eq!(runtime.get_path_state(path).error, None);
	assert_eq!(runtime.form_state().error.get(), None);
	assert_eq!(event_count.get(), 0);

	// An unchanged value still needs to discard errors left after validation.
	runtime.set_value(radio.answer_field(), String::new());
	runtime.set_error(radio.answer_field(), FieldError::new("unchanged error"));
	event_count.set(0);
	radio
		.__native_reset_epoch
		.update(|epoch| *epoch = epoch.wrapping_add(1));
	assert!(!runtime.form_state().is_touched.get());
	assert_eq!(runtime.form_state().error.get(), None);
	assert_eq!(event_count.get(), 0);

	// Ordinary changes after a reset still update the watched values and notify subscribers.
	radio.answer().set(String::from("on"));
	assert_eq!(runtime.watch().get().answer, "on");
	assert!(runtime.form_state().is_touched.get());
	assert_eq!(event_count.get(), 2);
}

#[cfg(wasm)]
mod browser {
	use super::*;
	use reinhardt_pages::component::{
		IntoPage, Page, PageElement, PageExt, cleanup_reactive_nodes,
	};
	use reinhardt_pages::dom::Element;
	use reinhardt_pages::{FieldError, RevalidateOn};
	use std::{cell::Cell, rc::Rc};
	use wasm_bindgen::JsCast;

	struct TestContainer(web_sys::Element);

	impl TestContainer {
		fn mount(page: Page) -> Self {
			let document = web_sys::window().unwrap().document().unwrap();
			let container = Self(document.create_element("div").unwrap());
			document.body().unwrap().append_child(&container.0).unwrap();
			page.mount(&Element::new(container.0.clone())).unwrap();
			container
		}

		fn input(&self, id: &str) -> web_sys::HtmlInputElement {
			self.0
				.query_selector(&format!("#{id}"))
				.unwrap()
				.expect("radio is mounted")
				.dyn_into()
				.unwrap()
		}

		fn native_form(&self) -> web_sys::HtmlFormElement {
			self.0
				.query_selector("form")
				.unwrap()
				.unwrap()
				.dyn_into()
				.unwrap()
		}
	}

	impl Drop for TestContainer {
		fn drop(&mut self) {
			cleanup_reactive_nodes();
			self.0.remove();
		}
	}

	fn assert_radio(
		input: &web_sys::HtmlInputElement,
		name: &str,
		id: &str,
		value: &str,
		checked: bool,
	) {
		assert_eq!(input.type_(), "radio");
		assert_eq!(input.name(), name);
		assert_eq!(input.id(), id);
		assert_eq!(input.value(), value);
		assert_eq!(input.checked(), checked);
	}

	fn assert_visible_radio(input: &web_sys::HtmlInputElement) {
		input.scroll_into_view();
		let width = web_sys::window()
			.unwrap()
			.inner_width()
			.unwrap()
			.as_f64()
			.unwrap();
		console_log!("RadioInput CSS viewport width: {width}");
		let visible = js_sys::Function::new_with_args(
			"input",
			"const r = input.getBoundingClientRect();
			return r.width > 0 && r.height > 0 && r.left >= 0 && r.right <= window.innerWidth
			    && document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) === input;",
		)
		.call1(&wasm_bindgen::JsValue::NULL, input.as_ref())
		.unwrap()
		.as_bool()
		.unwrap();
		assert!(
			visible,
			"radio must be visible and clickable within the {width}px viewport"
		);
	}

	#[wasm_bindgen_test]
	async fn radio_input_supports_label_selection_programmatic_changes_and_resets() {
		// Arrange
		let radio = form! {
			name: BrowserRadios,
			action: "/answer",
			method: Get,
			fields: {
				answer: ChoiceField<String> {
					widget: RadioInput,
					choices: [("yes", "Yes")],
				}
				selected: ChoiceField<String> {
					widget: RadioInput,
					label: "Initially selected",
					initial: "on",
				}
				reset: ResetButton {
					label: "Reset"
				}
			}
		};
		let runtime = use_form(&radio).revalidate_on(RevalidateOn::Change).build();
		let container = TestContainer::mount(radio.clone().into_page());
		let event_count = Rc::new(Cell::new(0));
		let observed_events = Rc::clone(&event_count);
		let _subscription =
			runtime.subscribe(move |_| observed_events.set(observed_events.get() + 1));
		assert_radio(&container.input("answer"), "answer", "answer", "yes", false);
		assert_visible_radio(&container.input("answer"));
		assert_radio(
			&container.input("selected"),
			"selected",
			"selected",
			"on",
			true,
		);
		let label = container
			.0
			.query_selector("label[for=\"answer\"]")
			.unwrap()
			.unwrap();
		assert_eq!(label.text_content().as_deref(), Some("Yes"));
		assert_eq!(
			label.get_attribute("for").as_deref(),
			Some(container.input("answer").id().as_str())
		);

		// Act and assert: the native label supplies both naming and selection.
		let document = web_sys::window().unwrap().document().unwrap();
		container.input("answer").focus().unwrap();
		assert_eq!(document.active_element().unwrap().id(), "answer");
		label.dyn_into::<web_sys::HtmlElement>().unwrap().click();
		assert_eq!(document.active_element().unwrap().id(), "answer");
		assert_eq!(runtime.get_values().answer, "yes");
		assert_radio(&container.input("answer"), "answer", "answer", "yes", true);
		assert_visible_radio(&container.input("answer"));
		assert!(container.input("selected").checked());

		// A change event from an unchecked radio must not select its option.
		runtime.set_value(radio.answer_field(), String::new());
		container
			.input("answer")
			.dispatch_event(&web_sys::Event::new("change").unwrap())
			.unwrap();
		assert_eq!(radio.answer().get(), "");
		assert!(!container.input("answer").checked());
		assert_eq!(document.active_element().unwrap().id(), "answer");

		radio.answer().set("yes".into());
		assert!(container.input("answer").checked());
		runtime.reset_field(radio.answer_field());
		assert_radio(&container.input("answer"), "answer", "answer", "yes", false);
		runtime.set_value(radio.selected_field(), String::new());
		assert!(!container.input("selected").checked());
		runtime.reset();
		assert!(container.input("selected").checked());

		container.input("answer").click();
		radio.selected().set(String::new());
		{
			let form_element = Element::new(container.native_form().into());
			let _cancel_reset = form_element.add_event_listener_with_event("reset", |event| {
				event.prevent_default();
			});
			container.native_form().reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			assert_eq!(runtime.get_values().answer, "yes");
			assert_eq!(runtime.get_values().selected, "");
			assert!(container.input("answer").checked());
			assert!(!container.input("selected").checked());
		}
		runtime.set_error(radio.answer_field(), FieldError::new("radio error"));
		event_count.set(0);
		container.native_form().reset();
		gloo_timers::future::TimeoutFuture::new(0).await;
		assert_eq!(runtime.get_values().answer, "");
		assert_eq!(runtime.get_values().selected, "on");
		assert_eq!(runtime.watch().get().answer, "");
		assert!(!runtime.form_state().is_touched.get());
		assert!(!runtime.form_state().is_dirty.get());
		assert!(!runtime.get_field_state(radio.answer_field()).is_touched);
		assert_eq!(runtime.get_field_state(radio.answer_field()).error, None);
		assert_eq!(event_count.get(), 0);
		assert_radio(&container.input("answer"), "answer", "answer", "yes", false);
		assert_radio(
			&container.input("selected"),
			"selected",
			"selected",
			"on",
			true,
		);

		container.input("answer").click();
		container
			.0
			.query_selector("button[type=reset]")
			.unwrap()
			.unwrap()
			.dyn_into::<web_sys::HtmlElement>()
			.unwrap()
			.click();
		gloo_timers::future::TimeoutFuture::new(0).await;
		assert_eq!(runtime.get_values().answer, "");
		assert!(!container.input("answer").checked());
	}

	#[wasm_bindgen_test]
	async fn native_radio_reset_restores_mixed_fields_and_collection_paths() {
		struct LoadedFields {
			answer: String,
			name: String,
			enabled: bool,
		}
		async fn load_fields() -> Result<LoadedFields, reinhardt_pages::ServerFnError> {
			Ok(LoadedFields {
				answer: String::from("yes"),
				name: String::from("loaded name"),
				enabled: false,
			})
		}

		// Arrange: scalar and collection controls share one native reset boundary.
		let radio = form! {
			name: MixedNativeReset,
			initial_loader: load_fields,
			fields: {
				answer: ChoiceField<String> {
					widget: RadioInput,
					choices: [("yes", "Yes")],
					initial_from: "answer",
				}
				name: CharField {
					initial: "initial name",
					initial_from: "name"
				}
				enabled: BooleanField {
					initial: true,
					initial_from: "enabled"
				}
				unbound: CharField { bind: false }
				answers: FieldArray {
					fields: {
						name: CharField {}
						enabled: BooleanField {}
						answer: ChoiceField<String> {
							widget: RadioInput,
							choices: [("yes", "Yes")],
						}
						unbound: CharField { bind: false }
					}
				}
			}
		};
		let runtime = use_form(&radio).revalidate_on(RevalidateOn::Change).build();
		let mut item = radio.new_answers_item();
		item.name = String::from("mounted row");
		item.enabled = true;
		item.answer = String::from("yes");
		let key = runtime.push_item(radio.answers_collection(), item);
		let mut second_item = radio.new_answers_item();
		second_item.name = String::from("second row");
		let second_key = runtime.push_item(radio.answers_collection(), second_item);
		let name_path = radio.answers_name_path(key);
		let name_watch = runtime.watch_path::<String>(name_path.clone());
		let container = TestContainer::mount(radio.clone().into_page());
		radio.unbound().set(String::from("unbound scalar kept"));
		runtime.set_path_value(
			radio.answers_unbound_path(key),
			String::from("unbound row kept"),
		);
		runtime.reset_default_values();
		let event_count = Rc::new(Cell::new(0));
		let observed_events = Rc::clone(&event_count);
		let _subscription =
			runtime.subscribe(move |_| observed_events.set(observed_events.get() + 1));

		// Act: reset edits to a row that has no persisted loader defaults yet.
		container.input("answer").click();
		container.input("name").set_value("edited name");
		container
			.input("name")
			.dispatch_event(&web_sys::Event::new("input").unwrap())
			.unwrap();
		runtime.set_value(radio.enabled_field(), false);
		runtime.set_path_value(name_path.clone(), String::from("edited row"));
		runtime.set_path_value(radio.answers_enabled_path(key), false);
		runtime.set_path_value(radio.answers_answer_path(key), String::new());
		runtime.set_error(radio.name_field(), FieldError::new("name error"));
		runtime.set_path_error(name_path.clone(), FieldError::new("row error"));
		event_count.set(0);
		container.native_form().reset();
		gloo_timers::future::TimeoutFuture::new(0).await;

		// Assert: values, DOM, and existing path handles all reflect their reset values.
		assert_eq!(runtime.get_values().name, "initial name");
		assert_eq!(container.input("name").value(), "initial name");
		assert!(runtime.get_values().enabled);
		assert!(container.input("enabled").checked());
		assert_eq!(runtime.get_values().answer, "");
		assert!(!container.input("answer").checked());
		assert_eq!(name_watch.get(), "mounted row");
		assert_eq!(container.input("answers_0_name").value(), "mounted row");
		assert!(runtime.get_values().answers[0].enabled);
		assert!(container.input("answers_0_enabled").checked());
		assert_eq!(runtime.get_values().answers[0].answer, "yes");
		assert!(container.input("answers_0_answer").checked());
		assert_eq!(radio.answers().get_untracked()[0].key(), key);
		assert_eq!(runtime.get_values().unbound, "unbound scalar kept");
		assert_eq!(runtime.get_values().answers[0].unbound, "unbound row kept");
		assert!(!runtime.form_state().is_dirty.get());
		assert!(!runtime.form_state().is_touched.get());
		assert_eq!(runtime.get_path_state(name_path.clone()).error, None);
		assert_eq!(runtime.form_state().error.get(), None);
		assert_eq!(event_count.get(), 0);

		// A later loader refresh persists current collection defaults as well as scalars.
		runtime.set_path_value(name_path.clone(), String::from("loaded row"));
		runtime.set_path_value(radio.answers_enabled_path(key), false);
		runtime.set_path_value(
			radio.answers_name_path(second_key),
			String::from("other loaded row"),
		);
		radio.load_initial_values().await.unwrap();
		assert_eq!(
			runtime.move_item(radio.answers_collection(), key, 1),
			Some((0, 1))
		);
		let name_watch = runtime.watch_path::<String>(name_path.clone());
		runtime.reset_default_values();
		runtime.set_value(radio.name_field(), String::from("changed again"));
		runtime.set_value(radio.enabled_field(), true);
		runtime.set_value(radio.answer_field(), String::new());
		runtime.set_path_value(name_path.clone(), String::from("changed row again"));
		runtime.set_path_value(
			radio.answers_name_path(second_key),
			String::from("other changed row"),
		);
		runtime.set_path_value(radio.answers_enabled_path(key), true);
		event_count.set(0);
		container.native_form().reset();
		gloo_timers::future::TimeoutFuture::new(0).await;
		assert_eq!(runtime.watch().get().name, "loaded name");
		assert_eq!(container.input("name").value(), "loaded name");
		assert_eq!(runtime.get_values().answer, "yes");
		assert!(container.input("answer").checked());
		assert!(!runtime.get_values().enabled);
		assert!(!container.input("enabled").checked());
		assert_eq!(name_watch.get(), "loaded row");
		assert_eq!(container.input("answers_1_name").value(), "loaded row");
		assert_eq!(
			container.input("answers_0_name").value(),
			"other loaded row"
		);
		assert!(!runtime.get_values().answers[1].enabled);
		assert!(!container.input("answers_1_enabled").checked());
		assert_eq!(radio.answers().get_untracked()[1].key(), key);
		assert_eq!(radio.answers().get_untracked()[0].key(), second_key);
		assert!(!runtime.form_state().is_dirty.get());
		assert!(!runtime.form_state().is_touched.get());
		assert_eq!(event_count.get(), 0);
		runtime.set_path_value(name_path, String::from("observed after reset"));
		assert_eq!(name_watch.get(), "observed after reset");
		assert_eq!(
			container.input("answers_1_name").value(),
			"observed after reset"
		);
	}

	#[wasm_bindgen_test]
	async fn radio_input_native_reset_uses_defaults_loaded_after_mount() {
		struct LoadedAnswer {
			answer: String,
		}
		async fn load_answer() -> Result<LoadedAnswer, reinhardt_pages::ServerFnError> {
			Ok(LoadedAnswer {
				answer: String::from("yes"),
			})
		}

		// Arrange: create the page before the asynchronous default arrives.
		let radio = form! {
			name: LoadedRadios,
			initial_loader: load_answer,
			fields: {
				answer: ChoiceField<String> {
					widget: RadioInput,
					choices: [("yes", "Yes")],
					initial_from: "answer",
				}
			}
		};
		let container = TestContainer::mount(radio.clone().into_page());
		assert!(!container.input("answer").checked());
		radio.load_initial_values().await.unwrap();
		assert!(container.input("answer").checked());
		radio.answer().set(String::new());
		assert!(!container.input("answer").checked());

		// Act and assert: reset reads the refreshed initial storage at event time.
		container.native_form().reset();
		gloo_timers::future::TimeoutFuture::new(0).await;
		assert_eq!(radio.answer().get(), "yes");
		assert!(container.input("answer").checked());
	}

	#[wasm_bindgen_test]
	fn radio_focus_replacement_is_scoped_to_new_roots_and_descendants() {
		// Arrange: a preceding form owns controls with matching IDs, names, and values.
		let unrelated = TestContainer::mount(
			PageElement::new("form")
				.child(
					PageElement::new("input")
						.attr("type", "radio")
						.attr("id", "answer")
						.attr("name", "answer")
						.attr("value", "yes"),
				)
				.child(
					PageElement::new("input")
						.attr("type", "radio")
						.attr("id", "answers_0_answer")
						.attr("name", "answers[0][answer]")
						.attr("value", "yes"),
				)
				.into_page(),
		);
		let radio = form! {
			name: FocusedRadios,
			fields: {
				answer: ChoiceField<String> {
					widget: RadioInput,
					choices: [("yes", "Yes")]
				}
				answers: FieldArray {
					fields: {
						answer: ChoiceField<String> {
							widget: RadioInput,
							choices: [("yes", "Yes")]
						}
					}
				}
			}
		};
		let runtime = use_form(&radio).build();
		let key = runtime.push_item(radio.answers_collection(), radio.new_answers_item());
		let container = TestContainer::mount(radio.clone().into_page());
		let document = web_sys::window().unwrap().document().unwrap();

		// Act and assert: both a root radio and a radio below a collection wrapper keep focus.
		container.input("answer").focus().unwrap();
		radio.answer().set(String::from("yes"));
		assert!(
			document
				.active_element()
				.unwrap()
				.is_same_node(Some(&container.input("answer")))
		);
		assert!(!unrelated.input("answer").checked());
		container.input("answers_0_answer").focus().unwrap();
		runtime.set_path_value(radio.answers_answer_path(key), String::from("yes"));
		assert!(
			document
				.active_element()
				.unwrap()
				.is_same_node(Some(&container.input("answers_0_answer")))
		);
		assert!(!unrelated.input("answers_0_answer").checked());
	}

	#[wasm_bindgen_test]
	fn radio_input_honors_unbound_snapshot_and_disabled_options() {
		// Arrange
		let radio = form! {
			name: UnboundRadios,
			action: "/answer",
			method: Get,
			fields: {
				snapshot: ChoiceField<String> {
					widget: RadioInput,
					initial: "on",
					bind: false,
				}
				unavailable: ChoiceField<String> {
					widget: RadioInput,
					choices: [("yes", "Unavailable") { disabled }],
				}
			}
		};
		let container = TestContainer::mount(radio.clone().into_page());

		// Act and assert: neither direction is bound, while native checked state works.
		assert_radio(
			&container.input("snapshot"),
			"snapshot",
			"snapshot",
			"on",
			true,
		);
		radio.snapshot().set("other".into());
		assert!(container.input("snapshot").checked());
		container.input("snapshot").set_checked(false);
		container.input("snapshot").click();
		assert!(container.input("snapshot").checked());
		assert_eq!(radio.snapshot().get(), "other");
		assert!(container.input("unavailable").disabled());
		container.input("unavailable").click();
		assert_radio(
			&container.input("unavailable"),
			"unavailable",
			"unavailable",
			"yes",
			false,
		);
		assert_eq!(radio.unavailable().get(), "");
	}

	#[wasm_bindgen_test]
	fn collection_radio_input_updates_only_its_item_and_resets() {
		// Arrange
		let radio = form! {
			name: BrowserCollectionRadios,
			action: "/answer",
			method: Get,
			fields: {
				answers: FieldArray {
					fields: {
						answer: ChoiceField<String> {
							widget: RadioInput,
							choices: [("yes", "Yes")],
						}
					}
				}
			}
		};
		let runtime = use_form(&radio).build();
		let first = runtime.push_item(radio.answers_collection(), radio.new_answers_item());
		let second = runtime.push_item(radio.answers_collection(), radio.new_answers_item());
		runtime.reset_default_values();
		let container = TestContainer::mount(radio.clone().into_page());
		assert_eq!(
			container
				.0
				.query_selector_all("input[type=radio]")
				.unwrap()
				.length(),
			2
		);
		assert_radio(
			&container.input("answers_0_answer"),
			"answers[0][answer]",
			"answers_0_answer",
			"yes",
			false,
		);
		assert_radio(
			&container.input("answers_1_answer"),
			"answers[1][answer]",
			"answers_1_answer",
			"yes",
			false,
		);
		assert_eq!(
			container
				.0
				.query_selector("label[for=answers_0_answer]")
				.unwrap()
				.unwrap()
				.text_content()
				.as_deref(),
			Some("Yes")
		);

		// Act and assert: independent item names prevent browser radio grouping.
		container.input("answers_0_answer").click();
		assert_eq!(runtime.get_values().answers[0].answer, "yes");
		assert_eq!(runtime.get_values().answers[1].answer, "");
		runtime.set_path_value(radio.answers_answer_path(second), String::from("yes"));
		assert!(container.input("answers_0_answer").checked());
		assert!(container.input("answers_1_answer").checked());
		runtime.set_path_value(radio.answers_answer_path(first), String::new());
		assert!(!container.input("answers_0_answer").checked());
		assert!(container.input("answers_1_answer").checked());
		runtime.reset();
		assert_eq!(runtime.get_values().answers.len(), 2);
		assert_eq!(runtime.get_values().answers[0].answer, "");
		assert_eq!(runtime.get_values().answers[1].answer, "");
		assert!(!container.input("answers_0_answer").checked());
		assert!(!container.input("answers_1_answer").checked());
	}
}
