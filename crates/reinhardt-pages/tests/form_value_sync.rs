//! Initial HTML and mounted value synchronization for stable `form!` controls.
//!
//! Native: `cargo test -p reinhardt-pages --test form_value_sync`
//! Browser: `wasm-pack test crates/reinhardt-pages --headless --chrome --test form_value_sync`

#[path = "fixtures/form_value_sync.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
#[path = "fixtures/form_scope.rs"]
mod form_scope;

use reinhardt_pages::{form, use_form};

use fixture::{INITIAL_VALUES, value_sync_form};

macro_rules! collection_form {
	() => {
		form! {
			name: CollectionValueSync,
			action: "/collection-value-sync",
			fields: {
				rows: FieldArray {
					fields: {
						title: CharField {}
						body: TextField {}
						active: BooleanField {}
						token: HiddenField<String> {}
						starts_at: DateTimeField {}
						state: ChoiceField<String> { choices: [("open", "Open"), ("review", "In review")] }
						tags: MultipleChoiceField<String> { choices: [("rust", "Rust"), ("web", "Web"), ("wasm", "WASM")] }
					}
				}
			}
		}
	};
}

macro_rules! seed_collection {
	($form:ident, $runtime:ident) => {{
		let mut row = $form.new_rows_item();
		row.title = String::from("Ada");
		row.body = String::from("Compiler engineer");
		row.active = true;
		row.token = String::from("row-token");
		row.starts_at =
			chrono::NaiveDate::from_ymd_opt(2026, 9, 7).and_then(|day| day.and_hms_opt(9, 30, 15));
		row.state = String::from("review");
		row.tags = vec![String::from("rust"), String::from("wasm")];
		let key = $runtime.push_item($form.rows_collection(), row);
		$runtime.reset_default_values();
		key
	}};
}

const COLLECTION_VALUES: &[(&str, &str, &str)] = &[
	("rows_0_title", "text", "Ada"),
	("rows_0_body", "textarea", "Compiler engineer"),
	("rows_0_active", "checkbox", "true"),
	("rows_0_token", "hidden", "row-token"),
	("rows_0_starts_at", "datetime-local", "2026-09-07T09:30:15"),
	("rows_0_state", "select-one", "review"),
	("rows_0_tags", "select-multiple", "rust,wasm"),
];

static STATIC_CHOICE_EVALUATIONS: std::sync::atomic::AtomicUsize =
	std::sync::atomic::AtomicUsize::new(0);

fn next_static_choice_value() -> usize {
	STATIC_CHOICE_EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

fn static_choice_expression_page(
	scope: &reinhardt_pages::reactive::ReactiveScope,
) -> reinhardt_pages::Page {
	scope.enter(|| {
		STATIC_CHOICE_EVALUATIONS.store(0, std::sync::atomic::Ordering::Relaxed);
		let form = form! {
			name: StaticChoiceExpressions,
			action: "/static-choice-expressions",
			fields: {
				plain: ChoiceField<usize> {
					initial: 1usize,
					choices: [(0, "Other"), (next_static_choice_value(), "Selected")],
				}
				group: FieldGroup {
					fields: {
						grouped: MultipleChoiceField<usize> {
							initial: vec![2, 3],
							choices: [OptGroup("Grouped") {
								(next_static_choice_value(), "Second"),
								(next_static_choice_value(), "Third"),
							}],
						}
					}
				}
				rows: FieldArray {
					fields: {
						single: ChoiceField<usize> {
							choices: [(0, "Other"), (next_static_choice_value(), "Fourth")],
						}
						multi: MultipleChoiceField<usize> {
							choices: [OptGroup("Row choices") {
								(next_static_choice_value(), "Fifth"),
								(next_static_choice_value(), "Sixth"),
							}],
						}
					}
				}
			}
		};
		let runtime = use_form(&form).build();
		let mut row = form.new_rows_item();
		row.single = 4;
		row.multi = vec![5, 6];
		runtime.push_item(form.rows_collection(), row);
		form.clone().into_page()
	})
}

const STATIC_CHOICE_VALUES: &[(&str, &str, &str)] = &[
	("plain", "select-one", "1"),
	("grouped", "select-multiple", "2,3"),
	("rows_0_single", "select-one", "4"),
	("rows_0_multi", "select-multiple", "5,6"),
];

#[cfg(not(target_arch = "wasm32"))]
mod native {
	use super::*;
	use rstest::rstest;

	fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
		let pattern = format!(" {name}=\"");
		tag.split_once(&pattern)?
			.1
			.split_once('"')
			.map(|(value, _)| value)
	}

	fn tag_by_id<'a>(html: &'a str, id: &str) -> &'a str {
		let id_offset = html.find(&format!(" id=\"{id}\"")).expect("field ID");
		let start = html[..id_offset].rfind('<').expect("tag start");
		let tail = &html[start..];
		&tail[..=tail.find('>').expect("tag end")]
	}

	fn contents_by_id<'a>(html: &'a str, id: &str, tag_name: &str) -> &'a str {
		let tag = tag_by_id(html, id);
		let start = html.find(tag).expect("opening tag") + tag.len();
		let rest = &html[start..];
		&rest[..rest.find(&format!("</{tag_name}>")).expect("closing tag")]
	}

	fn tags<'a>(html: &'a str, name: &'a str) -> impl Iterator<Item = &'a str> {
		html.split('<').filter_map(move |tail| {
			tail.strip_prefix(name)
				.filter(|rest| rest.starts_with(' '))
				.and_then(|_| tail.split_once('>').map(|(tag, _)| tag))
		})
	}

	fn assert_html_value(html: &str, id: &str, kind: &str, expected: &str) {
		match kind {
			"radio" => {
				let selected = tags(html, "input")
					.filter(|tag| attr(tag, "name") == Some(id))
					.filter(|tag| attr(tag, "checked") == Some("checked"))
					.map(|tag| attr(tag, "value").expect("radio value"))
					.collect::<Vec<_>>();
				assert_eq!(selected, [expected], "{id}: selected radio HTML");
			}
			"select-one" | "select-multiple" => {
				let html = contents_by_id(html, id, "select");
				let selected = tags(html, "option")
					.filter(|tag| attr(tag, "selected") == Some("selected"))
					.map(|tag| attr(tag, "value").expect("option value"))
					.collect::<Vec<_>>();
				assert_eq!(selected.join(","), expected, "{id}: selected option HTML");
			}
			"textarea" => {
				assert_eq!(contents_by_id(html, id, "textarea"), expected, "{id}");
				assert_eq!(
					attr(tag_by_id(html, id), "value"),
					None,
					"{id}: uses content"
				);
			}
			"checkbox" => {
				assert_eq!(attr(tag_by_id(html, id), "type"), Some("checkbox"));
				let checked = (expected == "true").then_some("checked");
				assert_eq!(attr(tag_by_id(html, id), "checked"), checked, "{id}");
			}
			"password" => {
				let tag = tag_by_id(html, id);
				assert_eq!(attr(tag, "type"), Some("password"));
				assert_eq!(attr(tag, "value"), None, "SSR must not disclose passwords");
				assert_eq!(attr(tag, "data-rh-password-omitted"), Some("true"));
			}
			"file" => {
				assert_eq!(attr(tag_by_id(html, id), "type"), Some("file"));
				assert_eq!(
					attr(tag_by_id(html, id), "value"),
					None,
					"{id}: native file restriction"
				);
			}
			_ => {
				let tag = tag_by_id(html, id);
				assert_eq!(attr(tag, "type"), Some(kind), "{id}: input type");
				assert_eq!(
					attr(tag, "value"),
					Some(expected),
					"{id}: initial value HTML"
				);
			}
		}
	}

	#[rstest]
	#[serial_test::serial(form_static_choice_expressions)]
	fn static_choice_expressions_are_evaluated_once_for_value_and_selection() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: every expression returns a different value on each evaluation.
			let scope = reinhardt_pages::reactive::ReactiveScope::new();
			let page = static_choice_expression_page(&scope);

			// Act.
			let html = page.render_to_string();

			// Assert: flat, grouped, and collection options reuse their emitted value.
			assert_eq!(
				STATIC_CHOICE_EVALUATIONS.load(std::sync::atomic::Ordering::Relaxed),
				6
			);
			for &(id, kind, expected) in STATIC_CHOICE_VALUES {
				assert_html_value(&html, id, kind, expected);
			}
		})
	}

	#[rstest]
	fn initial_html_covers_all_stable_field_types_and_widgets(
		#[values(
			0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
			24, 25, 26, 27, 28, 29
		)]
		index: usize,
	) {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: expected strings are literal references, independent of codegen.
			let form = value_sync_form!();
			let (id, kind, value) = INITIAL_VALUES[index];
			let expected = if id == "preferences" {
				"{&quot;theme&quot;:&quot;dark&quot;}"
			} else {
				value
			};

			// Act.
			let html = form.clone().into_page().render_to_string();

			// Assert.
			assert_html_value(&html, id, kind, expected);
		})
	}

	#[rstest]
	fn unbound_initial_render_is_a_snapshot_of_the_current_signal() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			form.unbound().set(String::from("Before render"));

			// Act.
			let page = form.clone().into_page();
			form.unbound().set(String::from("After render"));

			// Assert.
			assert_html_value(&page.render_to_string(), "unbound", "text", "Before render");
		})
	}

	#[rstest]
	fn initial_html_escapes_structured_values_and_preserves_optional_empty_values() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = form! {
				name: EscapedInitialValues,
				action: "/escaping",
				fields: {
					name: CharField {
						initial: "Ada & \"Co\""
					}
					body: TextField {
						initial: "<script>unsafe</script> & text"
					}
					json: JsonField<String> {
						initial: "<Ada>"
					}
					day: DateField {}
					time: TimeField {}
					starts_at: DateTimeField {}
					identifier: UuidField {}
					address: IpAddressField {}
					checked: BooleanField {}
				}
			};

			// Act.
			let html = form.clone().into_page().render_to_string();

			// Assert.
			assert_html_value(&html, "name", "text", "Ada &amp; &quot;Co&quot;");
			assert_html_value(
				&html,
				"body",
				"textarea",
				"&lt;script&gt;unsafe&lt;/script&gt; &amp; text",
			);
			assert_html_value(&html, "json", "textarea", "&quot;&lt;Ada&gt;&quot;");
			for (id, kind) in [
				("day", "date"),
				("time", "time"),
				("starts_at", "datetime-local"),
				("identifier", "text"),
				("address", "text"),
			] {
				assert_html_value(&html, id, kind, "");
			}
			assert_html_value(&html, "checked", "checkbox", "false");
		})
	}

	#[rstest]
	fn collection_html_preserves_initial_selected_checked_and_formatted_values() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = collection_form!();
			let runtime = use_form(&form).build();
			let key = seed_collection!(form, runtime);
			let title = runtime.watch_path::<String>(form.rows_title_path(key));
			assert_eq!(title.get(), "Ada");

			// Act.
			let html = form.clone().into_page().render_to_string();

			// Assert.
			for &(id, kind, expected) in COLLECTION_VALUES {
				assert_html_value(&html, id, kind, expected);
			}
			runtime.set_path_value(form.rows_title_path(key), String::from("Grace"));
			assert_eq!(title.get(), "Grace");
			assert_html_value(
				&form.clone().into_page().render_to_string(),
				"rows_0_title",
				"text",
				"Grace",
			);
			runtime.reset();
			assert_eq!(title.get(), "Ada", "existing path watchers follow reset");
			assert_eq!(
				form.rows().get()[0].key(),
				key,
				"reset preserves the existing item key"
			);
			assert_html_value(
				&form.clone().into_page().render_to_string(),
				"rows_0_title",
				"text",
				"Ada",
			);
		})
	}
}

#[cfg(target_arch = "wasm32")]
mod browser {
	use super::*;
	use fixture::EDITED_VALUES;
	use reinhardt_pages::component::{PageExt, cleanup_reactive_nodes};
	use reinhardt_pages::reactive::with_runtime;
	use serial_test::serial;
	use wasm_bindgen::{JsCast, JsValue};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	struct MountedForm(web_sys::Element);

	impl MountedForm {
		fn empty() -> Self {
			let document = web_sys::window().unwrap().document().unwrap();
			let root = Self(document.create_element("div").unwrap());
			document.body().unwrap().append_child(&root.0).unwrap();
			root
		}

		fn new(page: reinhardt_pages::Page) -> Self {
			let root = Self::empty();
			page.mount(&reinhardt_pages::dom::Element::new(root.0.clone()))
				.unwrap();
			flush();
			root
		}

		fn control(&self, id: &str) -> web_sys::Element {
			self.0
				.query_selector(&format!("#{id}"))
				.unwrap()
				.unwrap_or_else(|| panic!("missing mounted field: {id}"))
		}
	}

	impl Drop for MountedForm {
		fn drop(&mut self) {
			cleanup_reactive_nodes();
			self.0.remove();
		}
	}

	fn flush() {
		with_runtime(|runtime| runtime.flush_updates());
	}

	fn dom_value(root: &MountedForm, id: &str, kind: &str) -> String {
		if kind == "radio" {
			let checked = root
				.0
				.query_selector(&format!("input[name='{id}']:checked"))
				.unwrap()
				.expect("checked radio");
			return checked
				.unchecked_into::<web_sys::HtmlInputElement>()
				.value();
		}
		let control = root.control(id);
		match kind {
			"textarea" => control
				.unchecked_into::<web_sys::HtmlTextAreaElement>()
				.value(),
			"select-one" | "select-multiple" => {
				let select = control.unchecked_into::<web_sys::HtmlSelectElement>();
				assert_eq!(select.type_(), kind, "{id}: select type");
				let selected = select.selected_options();
				(0..selected.length())
					.map(|index| {
						selected
							.item(index)
							.unwrap()
							.unchecked_into::<web_sys::HtmlOptionElement>()
							.value()
					})
					.collect::<Vec<_>>()
					.join(",")
			}
			_ => {
				let input = control.unchecked_into::<web_sys::HtmlInputElement>();
				assert_eq!(input.type_(), kind, "{id}: native input type");
				if kind == "checkbox" {
					input.checked().to_string()
				} else {
					input.value()
				}
			}
		}
	}

	fn edit_control(root: &MountedForm, id: &str, kind: &str, value: &str) {
		let control = if kind == "radio" {
			root.0
				.query_selector(&format!("input[name='{id}'][value='{value}']"))
				.unwrap()
				.expect("radio option")
		} else {
			root.control(id)
		};
		let event = match kind {
			"textarea" => {
				control
					.unchecked_ref::<web_sys::HtmlTextAreaElement>()
					.set_value(value);
				"input"
			}
			"select-one" | "select-multiple" => {
				let options = control.query_selector_all("option").unwrap();
				for index in 0..options.length() {
					let option = options
						.item(index)
						.unwrap()
						.unchecked_into::<web_sys::HtmlOptionElement>();
					option
						.set_selected(value.split(',').any(|selected| selected == option.value()));
				}
				"change"
			}
			"checkbox" | "radio" => {
				control
					.unchecked_ref::<web_sys::HtmlInputElement>()
					.set_checked(kind == "radio" || value == "true");
				"change"
			}
			_ => {
				control
					.unchecked_ref::<web_sys::HtmlInputElement>()
					.set_value(value);
				"input"
			}
		};
		control
			.dispatch_event(&web_sys::Event::new(event).unwrap())
			.unwrap();
		flush();
		assert_eq!(
			dom_value(root, id, kind),
			value,
			"{id}: establish edited DOM"
		);
	}

	fn assert_values(root: &MountedForm, values: &[(&str, &str, &str)]) {
		for &(id, kind, expected) in values {
			assert_eq!(dom_value(root, id, kind), expected, "{id}: DOM value");
		}
	}

	fn remember_nodes(root: &MountedForm) -> Vec<web_sys::Node> {
		let controls = root
			.0
			.query_selector_all("input, textarea, select, option")
			.unwrap();
		(0..controls.length())
			.map(|index| controls.item(index).unwrap())
			.collect()
	}

	fn assert_same_nodes(root: &MountedForm, previous: &[web_sys::Node]) {
		let current = remember_nodes(root);
		assert_eq!(current.len(), previous.len());
		for (before, after) in previous.iter().zip(current) {
			assert!(
				before.is_same_node(Some(&after)),
				"a mounted {} control was replaced: {:?}",
				before.node_name(),
				before
					.dyn_ref::<web_sys::Element>()
					.map(web_sys::Element::id),
			);
		}
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn static_choice_expressions_are_evaluated_once_for_value_and_selection() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange and act: mount flat, grouped, and collection select expressions.
			let scope = reinhardt_pages::reactive::ReactiveScope::new();
			let mounted = MountedForm::new(static_choice_expression_page(&scope));

			// Assert: selected attributes and live selection use the same serialized values.
			assert_eq!(
				STATIC_CHOICE_EVALUATIONS.load(std::sync::atomic::Ordering::Relaxed),
				6
			);
			assert_values(&mounted, STATIC_CHOICE_VALUES);
			for &(id, _, expected) in STATIC_CHOICE_VALUES {
				let options = mounted.control(id).query_selector_all("option").unwrap();
				let defaults = (0..options.length())
					.map(|index| {
						options
							.item(index)
							.unwrap()
							.unchecked_into::<web_sys::HtmlOptionElement>()
					})
					.filter(web_sys::HtmlOptionElement::default_selected)
					.map(|option| option.value())
					.collect::<Vec<_>>();
				assert_eq!(defaults.join(","), expected, "{id}: default selection");
			}
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn initial_dom_covers_all_stable_field_types_and_widgets() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			let form = value_sync_form!();
			let mounted = MountedForm::new(form.clone().into_page());
			assert_values(&mounted, INITIAL_VALUES);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn edits_signal_updates_and_reset_preserve_nodes_and_focus() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let mounted = MountedForm::new(form.clone().into_page());
			let nodes = remember_nodes(&mounted);
			let name = mounted
				.control("name")
				.unchecked_into::<web_sys::HtmlInputElement>();
			name.focus().unwrap();

			// Act: every bound writable control first receives a different DOM value.
			for &(id, kind, value) in EDITED_VALUES {
				edit_control(&mounted, id, kind, value);
			}

			// Assert the DOM-to-Rust conversions independently of their string formatter.
			assert_eq!(form.name().get(), "Grace");
			assert_eq!(form.bio().get(), "Rear admiral\nCOBOL");
			assert_eq!(form.email().get(), "grace@example.com");
			assert_eq!(form.password().get(), "another-password");
			assert_eq!(form.site().get(), "https://example.com/grace");
			assert_eq!(form.slug().get(), "grace-hopper");
			assert_eq!(form.count().get(), 7);
			assert_eq!(form.ratio().get(), 2.75);
			assert_eq!(form.price().get(), 18.25);
			assert!(!form.subscribed().get());
			assert_eq!(
				form.day().get(),
				chrono::NaiveDate::from_ymd_opt(2026, 10, 8)
			);
			assert_eq!(
				form.time().get(),
				chrono::NaiveTime::from_hms_opt(14, 45, 20)
			);
			assert_eq!(
				form.starts_at().get(),
				chrono::NaiveDate::from_ymd_opt(2026, 10, 8)
					.and_then(|day| day.and_hms_opt(14, 45, 20))
			);
			assert_eq!(
				form.identifier().get(),
				Some(uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap())
			);
			assert_eq!(
				form.address().get(),
				Some("2001:db8::8".parse::<std::net::IpAddr>().unwrap())
			);
			assert_eq!(form.token().get(), "changed-hidden");
			assert_eq!(form.status().get(), "open");
			assert_eq!(form.tags().get(), [String::from("web")]);
			assert_eq!(
				form.preferences().get(),
				serde_json::json!({"theme": "light"})
			);
			assert_eq!(form.radio().get(), "open");
			assert_eq!(form.color().get(), "#996633");
			assert_eq!(form.level().get(), 75);
			assert_eq!(form.phone().get(), "+1-202-555-0108");
			assert_eq!(form.search().get(), "typed state");
			assert_eq!(form.month().get(), "2026-10");
			assert_eq!(form.week().get(), "2026-W41");
			assert_eq!(form.typed_choice().get(), 1);
			assert_same_nodes(&mounted, &nodes);

			// Act: direct signal writes must update every mounted control category.
			let defaults = runtime.default_values();
			form.name().set(defaults.name);
			form.bio().set(defaults.bio);
			form.email().set(defaults.email);
			form.password().set(defaults.password);
			form.site().set(defaults.site);
			form.slug().set(defaults.slug);
			form.count().set(defaults.count);
			form.ratio().set(defaults.ratio);
			form.price().set(defaults.price);
			form.subscribed().set(defaults.subscribed);
			form.day().set(defaults.day);
			form.time().set(defaults.time);
			form.starts_at().set(defaults.starts_at);
			form.identifier().set(defaults.identifier);
			form.address().set(defaults.address);
			form.token().set(defaults.token);
			form.status().set(defaults.status);
			form.tags().set(defaults.tags);
			form.preferences().set(defaults.preferences);
			form.radio().set(defaults.radio);
			form.color().set(defaults.color);
			form.level().set(defaults.level);
			form.phone().set(defaults.phone);
			form.search().set(defaults.search);
			form.month().set(defaults.month);
			form.week().set(defaults.week);
			form.typed_choice().set(defaults.typed_choice);
			flush();

			// Assert.
			assert_values(&mounted, INITIAL_VALUES);
			assert_same_nodes(&mounted, &nodes);
			assert_eq!(
				web_sys::window()
					.unwrap()
					.document()
					.unwrap()
					.active_element()
					.unwrap(),
				name.clone().unchecked_into::<web_sys::Element>()
			);

			// Act: reset must also overwrite already-dirty browser value properties.
			for &(id, kind, value) in EDITED_VALUES {
				edit_control(&mounted, id, kind, value);
			}
			runtime.reset();
			flush();

			// Assert.
			assert_values(&mounted, INITIAL_VALUES);
			assert_same_nodes(&mounted, &nodes);
			assert_eq!(
				web_sys::window()
					.unwrap()
					.document()
					.unwrap()
					.active_element()
					.unwrap(),
				name.unchecked_into::<web_sys::Element>()
			);
			assert_eq!(runtime.get_values().name, "Ada");
			assert!(!runtime.form_state().is_dirty.get());
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn bind_false_renders_a_snapshot_without_listeners_or_signal_binding() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			form.unbound().set(String::from("Before mount"));
			let mounted = MountedForm::new(form.clone().into_page());
			assert_eq!(dom_value(&mounted, "unbound", "text"), "Before mount");

			// Act and assert each direction separately.
			form.unbound().set(String::from("Programmatic"));
			flush();
			assert_eq!(dom_value(&mounted, "unbound", "text"), "Before mount");
			edit_control(&mounted, "unbound", "text", "DOM only");
			assert_eq!(form.unbound().get(), "Programmatic");
			runtime.reset();
			flush();
			assert_eq!(form.unbound().get(), "Snapshot");
			assert_eq!(dom_value(&mounted, "unbound", "text"), "DOM only");
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn hydration_adopts_preexisting_edits_and_reset_restores_declared_defaults() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: the DOM is SSR HTML, edited before any listener is attached.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			for &(id, kind, value) in EDITED_VALUES {
				edit_control(&mounted, id, kind, value);
			}
			assert_eq!(form.name().get(), "Ada");
			let name = mounted
				.control("name")
				.unchecked_into::<web_sys::HtmlInputElement>();
			name.focus().unwrap();
			name.set_selection_range(1, 3).unwrap();

			// Act.
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert: adopting edits does not change the DOM or the reset baseline.
			assert_values(&mounted, EDITED_VALUES);
			assert_eq!(form.name().get(), "Grace");
			assert!(!form.subscribed().get());
			assert_eq!(form.status().get(), "open");
			assert_eq!(form.tags().get(), [String::from("web")]);
			assert_eq!(form.count().get(), 7);
			assert_eq!(runtime.default_values().name, "Ada");
			assert_eq!(name.selection_start().unwrap(), Some(1));
			assert_eq!(name.selection_end().unwrap(), Some(3));
			assert_same_nodes(&mounted, &nodes);
			runtime.reset();
			flush();
			assert_values(&mounted, INITIAL_VALUES);
			assert_same_nodes(&mounted, &nodes);
			assert_eq!(
				web_sys::window()
					.unwrap()
					.document()
					.unwrap()
					.active_element()
					.unwrap(),
				name.unchecked_into::<web_sys::Element>()
			);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn pre_hydration_runtime_reset_takes_precedence_over_stale_ssr_values() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: server defaults differ from the fresh client render across control types.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			form.name().set(String::from("Stale server name"));
			form.bio().set(String::from("Stale server biography"));
			form.subscribed().set(false);
			form.status().set(String::from("open"));
			form.tags().set(vec![String::from("web")]);
			form.radio().set(String::from("open"));
			let mounted = MountedForm::empty();
			mounted
				.0
				.set_inner_html(&form.clone().into_page().render_to_string());
			let nodes = remember_nodes(&mounted);
			let name = mounted
				.control("name")
				.unchecked_into::<web_sys::HtmlInputElement>();
			name.focus().unwrap();
			assert_eq!(dom_value(&mounted, "name", "text"), "Stale server name");
			assert_eq!(dom_value(&mounted, "tags", "select-multiple"), "web");

			// Act: an explicit reset before hydration must win over the stale browser values.
			runtime.reset();
			let page = form.clone().into_page();
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::reconcile(&root, &page)
				.expect("controlled defaults may differ during hydration");
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert.
			assert_values(&mounted, INITIAL_VALUES);
			assert_eq!(form.name().get(), "Ada");
			assert_eq!(form.bio().get(), "Compiler engineer\nRust");
			assert!(form.subscribed().get());
			assert_eq!(form.status().get(), "review");
			assert_eq!(
				form.tags().get(),
				[String::from("rust"), String::from("wasm")]
			);
			assert_eq!(form.radio().get(), "review");
			assert_same_nodes(&mounted, &nodes);
			assert_eq!(
				web_sys::window()
					.unwrap()
					.document()
					.unwrap()
					.active_element()
					.unwrap(),
				name.unchecked_into::<web_sys::Element>()
			);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn native_form_reset_adopts_browser_defaults_after_the_default_action() {
		crate::form_scope::run(async {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let mounted = MountedForm::new(form.clone().into_page());
			let nodes = remember_nodes(&mounted);
			for &(id, kind, value) in EDITED_VALUES {
				edit_control(&mounted, id, kind, value);
			}
			assert_eq!(form.name().get(), "Grace");
			assert_eq!(form.tags().get(), [String::from("web")]);
			// Hidden input values also change their native default-value attribute.
			assert_eq!(
				mounted
					.control("token")
					.unchecked_into::<web_sys::HtmlInputElement>()
					.default_value(),
				"changed-hidden"
			);

			// Act: the browser reset default action precedes the retained reset listener.
			mounted
				.0
				.first_element_child()
				.unwrap()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			flush();

			// Assert.
			for &(id, kind, initial) in INITIAL_VALUES {
				let expected = if id == "password" {
					""
				} else if id == "token" {
					"changed-hidden"
				} else {
					initial
				};
				assert_eq!(
					dom_value(&mounted, id, kind),
					expected,
					"{id}: native reset default"
				);
			}
			assert_eq!(form.name().get(), "Ada");
			assert_eq!(form.token().get(), "changed-hidden");
			assert!(form.subscribed().get());
			assert_eq!(form.status().get(), "review");
			assert_eq!(
				form.tags().get(),
				[String::from("rust"), String::from("wasm")]
			);
			assert_eq!(runtime.get_values().count, 42);
			assert_same_nodes(&mounted, &nodes);
		})
		.await
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn native_reset_snapshots_shared_checkbox_values_before_writing_signals() {
		crate::form_scope::run(async {
			use reinhardt_pages::component::{
				ControlBinding, ControlKind, ControlValue, IntoPage, PageElement,
			};
			use reinhardt_pages::reactive::Signal;

			// Arrange: both native defaults are checked, but the shared live value is empty.
			let selected = Signal::new(Vec::<String>::new());
			let inputs = ["rust", "wasm"].map(|value| {
				let read = selected.clone();
				let write = selected.clone();
				PageElement::new("input")
					.attr("type", "checkbox")
					.attr("id", value)
					.bool_attr("checked", true)
					.control_binding(
						ControlBinding::from_parts(
							ControlKind::Checkbox,
							None,
							selected.id(),
							move || {
								ControlValue::Checked(read.get().iter().any(|item| item == value))
							},
							move |raw| {
								let ControlValue::Checked(checked) = raw else {
									panic!("checkbox value")
								};
								write.update(|selected| {
									selected.retain(|item| item != value);
									if checked {
										selected.push(String::from(value));
									}
								});
								Ok(reinhardt_pages::component::ControlWriteOutcome::Committed)
							},
							move || {
								let value = selected.get_untracked();
								Box::new(move || selected.set(value))
							},
						)
						.on_native_reset(|| {}),
					)
			});
			let mounted = MountedForm::new(PageElement::new("form").children(inputs).into_page());
			// Projected controls can choose browser defaults independently of their source.
			for id in ["rust", "wasm"] {
				let input = mounted
					.control(id)
					.unchecked_into::<web_sys::HtmlInputElement>();
				input.set_default_checked(true);
				input.set_checked(false);
			}
			assert_eq!(dom_value(&mounted, "rust", "checkbox"), "false");
			assert_eq!(dom_value(&mounted, "wasm", "checkbox"), "false");

			// Act.
			mounted
				.0
				.first_element_child()
				.unwrap()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			flush();

			// Assert: writing the first value must not erase the second snapshot.
			assert_eq!(selected.get(), [String::from("rust"), String::from("wasm")]);
			assert_eq!(dom_value(&mounted, "rust", "checkbox"), "true");
			assert_eq!(dom_value(&mounted, "wasm", "checkbox"), "true");
		})
		.await
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn cancelled_native_reset_keeps_the_current_source_and_dom() {
		crate::form_scope::run(async {
			let form = value_sync_form!();
			let mounted = MountedForm::new(form.clone().into_page());
			let element =
				reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			let _cancel_reset =
				element.add_event_listener_with_event("reset", |event| event.prevent_default());
			edit_control(&mounted, "name", "text", "Keep this edit");
			mounted
				.0
				.first_element_child()
				.unwrap()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			flush();
			assert_eq!(form.name().get(), "Keep this edit");
			assert_eq!(dom_value(&mounted, "name", "text"), "Keep this edit");
		})
		.await
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn hydrated_select_keeps_its_source_when_options_arrive_or_are_replaced() {
		crate::form_scope::run(async {
			use reinhardt_pages::component::{ControlBinding, IntoPage, PageElement};
			use reinhardt_pages::reactive::Signal;

			// Arrange: an empty SSR select cannot represent the current source yet.
			let value = Signal::new(String::from("review"));
			let page = PageElement::new("form")
				.child(
					PageElement::new("select")
						.attr("id", "pending_choices")
						.control_binding(ControlBinding::select_one(value)),
				)
				.into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			let select = mounted
				.control("pending_choices")
				.unchecked_into::<web_sys::HtmlSelectElement>();
			select.focus().unwrap();

			// Act and assert: hydration must not replace a pending source with an empty value.
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();
			assert_eq!(value.get(), "review");
			assert_eq!(select.value(), "");
			for options in [
				r#"<option value="open">Open</option><option value="review">In review</option>"#,
				r#"<option value="open">Reopened</option><option value="review">Review pending</option>"#,
			] {
				select.set_inner_html(options);
				assert_eq!(
					select.value(),
					"open",
					"the new options first use their browser default"
				);
				gloo_timers::future::TimeoutFuture::new(0).await;
				flush();
				assert_eq!(select.value(), "review");
				assert_eq!(value.get(), "review");
				assert!(select.is_same_node(Some(&mounted.control("pending_choices"))));
				assert_eq!(
					web_sys::window()
						.unwrap()
						.document()
						.unwrap()
						.active_element()
						.unwrap(),
					select.clone().unchecked_into::<web_sys::Element>()
				);
			}
		})
		.await
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn collection_updates_and_reset_preserve_controls_and_focus() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = collection_form!();
			let runtime = use_form(&form).build();
			let key = seed_collection!(form, runtime);
			let mounted = MountedForm::new(form.clone().into_page());
			assert_values(&mounted, COLLECTION_VALUES);
			let nodes = remember_nodes(&mounted);
			let title = mounted
				.control("rows_0_title")
				.unchecked_into::<web_sys::HtmlInputElement>();
			title.focus().unwrap();

			// Act: use the existing path API, including selected and checked properties.
			edit_control(&mounted, "rows_0_title", "text", "Browser edit");
			assert_eq!(runtime.get_values().rows[0].title, "Browser edit");
			runtime.set_path_value(form.rows_title_path(key), String::from("Grace"));
			runtime.set_path_value(form.rows_active_path(key), false);
			runtime.set_path_value(form.rows_state_path(key), String::from("open"));
			runtime.set_path_value(form.rows_tags_path(key), vec![String::from("web")]);
			flush();

			// Assert.
			assert_eq!(dom_value(&mounted, "rows_0_title", "text"), "Grace");
			assert_eq!(dom_value(&mounted, "rows_0_active", "checkbox"), "false");
			assert_eq!(dom_value(&mounted, "rows_0_state", "select-one"), "open");
			assert_eq!(dom_value(&mounted, "rows_0_tags", "select-multiple"), "web");
			assert_eq!(
				form.rows().get()[0].key(),
				key,
				"value updates preserve the item key"
			);
			assert_same_nodes(&mounted, &nodes);
			runtime.reset();
			assert_eq!(
				form.rows().get()[0].key(),
				key,
				"reset preserves the existing item key"
			);
			flush();
			assert_values(&mounted, COLLECTION_VALUES);
			assert_same_nodes(&mounted, &nodes);
			assert_eq!(
				web_sys::window()
					.unwrap()
					.document()
					.unwrap()
					.active_element()
					.unwrap(),
				title.unchecked_into::<web_sys::Element>()
			);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn pre_hydration_reconciled_collection_defaults_preserve_other_dirty_controls() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			use reinhardt_pages::RevalidateOn;
			use std::{cell::Cell, rc::Rc};

			for dirty_note in [false, true] {
				// Arrange: old SSR values remain in the browser when defaults refresh.
				let form = form! {
					name: ReconciledCollectionHydration,
					fields: {
						note: CharField {
							initial: "SSR note"
						}
						rows: FieldArray {
							fields: {
								title: CharField {}
								active: BooleanField {}
							}
						}
					}
				};
				let runtime = use_form(&form)
					.deps(0_u8)
					.revalidate_on(RevalidateOn::Change)
					.build();
				let mut row = form.new_rows_item();
				row.title = String::from("SSR row");
				let key = runtime.push_item(form.rows_collection(), row);
				runtime.reset_default_values();
				let watched = runtime.watch_path::<String>(form.rows_title_path(key));
				let page = form.clone().into_page();
				let mounted = MountedForm::empty();
				mounted.0.set_inner_html(&page.render_to_string());
				let nodes = remember_nodes(&mounted);
				if dirty_note {
					form.note().set(String::from("Known edit"));
					edit_control(&mounted, "note", "text", "Latest browser edit");
				}
				let mut defaults = runtime.default_values();
				defaults.note = String::from("Refreshed note");
				defaults.rows[0].title = String::from("Refreshed row");
				defaults.rows[0].active = true;

				// Act: KeepDirtyValues updates only pristine sources before attachment.
				runtime.reconcile_defaults(defaults, 1_u8);
				assert_eq!(watched.get(), "Refreshed row");
				let events = Rc::new(Cell::new(0));
				let observed_events = Rc::clone(&events);
				let _subscription =
					runtime.subscribe(move |_| observed_events.set(observed_events.get() + 1));
				let root =
					reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
				reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
				flush();

				// Assert: refreshed paths win, and unrelated browser edits stay adoptable.
				assert_eq!(runtime.get_values().rows[0].title, "Refreshed row");
				assert_eq!(watched.get(), "Refreshed row");
				assert_eq!(dom_value(&mounted, "rows_0_title", "text"), "Refreshed row");
				assert_eq!(dom_value(&mounted, "rows_0_active", "checkbox"), "true");
				assert_eq!(form.rows().get()[0].key(), key);
				assert!(
					!runtime
						.get_collection_state(form.rows_collection())
						.is_dirty
				);
				let expected_note = if dirty_note {
					"Latest browser edit"
				} else {
					"Refreshed note"
				};
				assert_eq!(form.note().get(), expected_note);
				assert_eq!(dom_value(&mounted, "note", "text"), expected_note);
				assert_eq!(runtime.form_state().is_dirty.get(), dirty_note);
				assert_eq!(events.get(), if dirty_note { 2 } else { 0 });
				assert_same_nodes(&mounted, &nodes);
			}
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn pre_hydration_field_reset_preserves_other_browser_edits() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			edit_control(&mounted, "name", "text", "Browser name");
			edit_control(&mounted, "bio", "textarea", "Browser biography");

			// Act: only the reset field takes precedence over its browser snapshot.
			runtime.reset_field(form.name_field());
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert.
			assert_eq!(form.name().get(), "Ada");
			assert_eq!(form.bio().get(), "Browser biography");
			assert_eq!(dom_value(&mounted, "name", "text"), "Ada");
			assert_eq!(dom_value(&mounted, "bio", "textarea"), "Browser biography");
			assert_same_nodes(&mounted, &nodes);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn pre_hydration_scalar_setter_preserves_explicit_value_and_other_browser_edits() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			edit_control(&mounted, "name", "text", "Browser name");
			edit_control(&mounted, "bio", "textarea", "Browser biography");

			// Act: the explicit setter wins even when the Page was constructed earlier.
			runtime.set_value(form.name_field(), String::from("Explicit name"));
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert.
			assert_eq!(form.name().get(), "Explicit name");
			assert_eq!(form.bio().get(), "Browser biography");
			assert_eq!(dom_value(&mounted, "name", "text"), "Explicit name");
			assert_same_nodes(&mounted, &nodes);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn pre_hydration_path_setter_preserves_only_the_selected_path() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = collection_form!();
			let runtime = use_form(&form).build();
			let key = seed_collection!(form, runtime);
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			edit_control(&mounted, "rows_0_title", "text", "Browser title");
			edit_control(&mounted, "rows_0_body", "textarea", "Browser body");

			// Act.
			runtime.set_path_value(form.rows_title_path(key), String::from("Explicit title"));
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert.
			assert_eq!(runtime.get_values().rows[0].title, "Explicit title");
			assert_eq!(runtime.get_values().rows[0].body, "Browser body");
			assert_eq!(
				dom_value(&mounted, "rows_0_title", "text"),
				"Explicit title"
			);
			assert_eq!(form.rows().get()[0].key(), key);
			assert_same_nodes(&mounted, &nodes);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn hydration_preserves_source_order_for_an_unchanged_multiple_selection() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: source order deliberately differs from option order.
			let form = value_sync_form!();
			form.tags()
				.set(vec![String::from("wasm"), String::from("rust")]);
			let runtime = use_form(&form).build();
			runtime.reset_default_values();
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			assert_eq!(dom_value(&mounted, "tags", "select-multiple"), "rust,wasm");

			// Act.
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert.
			assert_eq!(
				form.tags().get(),
				[String::from("wasm"), String::from("rust")]
			);
			assert!(!runtime.form_state().is_dirty.get());
			assert!(!runtime.form_state().is_touched.get());
			assert_same_nodes(&mounted, &nodes);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn hydration_adopts_browser_owned_files_without_replacing_inputs() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange: selections occur before any event listeners have been attached.
			let form = value_sync_form!();
			let page = form.clone().into_page();
			let mounted = MountedForm::empty();
			mounted.0.set_inner_html(&page.render_to_string());
			let nodes = remember_nodes(&mounted);
			let parts = js_sys::Array::of1(&JsValue::from_str("pre-hydration contents"));
			let file = web_sys::File::new_with_str_sequence(&parts, "selected.txt").unwrap();
			for id in ["attachment", "image"] {
				select_file(&mounted.control(id).unchecked_into(), &file);
			}
			assert!(form.attachment().get().is_none());
			assert!(form.image().get().is_none());

			// Act.
			let root = reinhardt_pages::dom::Element::new(mounted.0.first_element_child().unwrap());
			reinhardt_pages::hydration::attach_events_to_mounted_view(&root, &page).unwrap();
			flush();

			// Assert: the exact File objects remain available to submission code.
			assert_eq!(form.attachment().get(), Some(file.clone()));
			assert_eq!(form.image().get(), Some(file.clone()));
			for id in ["attachment", "image"] {
				let input = mounted
					.control(id)
					.unchecked_into::<web_sys::HtmlInputElement>();
				assert_eq!(input.files().unwrap().get(0), Some(file.clone()));
			}
			assert_same_nodes(&mounted, &nodes);
		})
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn native_reset_clears_runtime_metadata_even_when_values_are_already_default() {
		crate::form_scope::run(async {
			use reinhardt_pages::{FieldError, RevalidateOn};

			// Arrange: setting the same value still marks the field touched.
			let form = value_sync_form!();
			let runtime = use_form(&form).revalidate_on(RevalidateOn::Change).build();
			// Passwords deliberately have an empty browser reset default.
			form.password().set(String::new());
			runtime.reset_default_values();
			let mounted = MountedForm::new(form.clone().into_page());
			let events = std::rc::Rc::new(std::cell::Cell::new(0));
			let _subscription = runtime.subscribe({
				let events = events.clone();
				move |_| events.set(events.get() + 1)
			});
			for value in ["Grace", "Ada"] {
				runtime.set_value(form.name_field(), String::from(value));
				runtime.set_error(form.name_field(), FieldError::new("stale field error"));
				assert!(runtime.get_field_state(form.name_field()).is_touched);
				events.set(0);

				// Act.
				mounted
					.0
					.first_element_child()
					.unwrap()
					.unchecked_into::<web_sys::HtmlFormElement>()
					.reset();
				gloo_timers::future::TimeoutFuture::new(0).await;
				flush();

				// Assert: reset emits neither validation nor value-change notifications.
				assert_eq!(form.name().get(), "Ada");
				assert!(!runtime.get_field_state(form.name_field()).is_touched);
				assert_eq!(runtime.get_field_state(form.name_field()).error, None);
				assert!(!runtime.form_state().is_touched.get());
				assert!(!runtime.form_state().is_dirty.get());
				assert_eq!(events.get(), 0);
			}
		})
		.await
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn native_reset_clears_collection_and_path_metadata() {
		crate::form_scope::run(async {
			use reinhardt_pages::FieldError;

			// Arrange.
			let form = collection_form!();
			let runtime = use_form(&form).build();
			let key = seed_collection!(form, runtime);
			let mounted = MountedForm::new(form.clone().into_page());
			let path = form.rows_title_path(key);
			let watched = runtime.watch_path::<String>(path.clone());
			runtime.set_path_value(path.clone(), String::from("Edited title"));
			runtime.set_path_error(path.clone(), FieldError::new("stale path error"));
			assert!(runtime.get_path_state(path.clone()).is_touched);
			assert!(
				runtime
					.get_collection_state(form.rows_collection())
					.is_touched
			);

			// Act.
			mounted
				.0
				.first_element_child()
				.unwrap()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			flush();

			// Assert.
			assert_eq!(watched.get(), "Ada");
			assert!(!runtime.get_path_state(path.clone()).is_touched);
			assert_eq!(runtime.get_path_state(path).error, None);
			assert!(
				!runtime
					.get_collection_state(form.rows_collection())
					.is_touched
			);
			assert!(!runtime.form_state().is_touched.get());
			assert!(!runtime.form_state().is_dirty.get());
		})
		.await
	}

	fn select_file(input: &web_sys::HtmlInputElement, file: &web_sys::File) {
		let window = web_sys::window().unwrap();
		let constructor = js_sys::Reflect::get(&window, &JsValue::from_str("DataTransfer"))
			.unwrap()
			.unchecked_into::<js_sys::Function>();
		let transfer = js_sys::Reflect::construct(&constructor, &js_sys::Array::new()).unwrap();
		let items = js_sys::Reflect::get(&transfer, &JsValue::from_str("items")).unwrap();
		let add = js_sys::Reflect::get(&items, &JsValue::from_str("add"))
			.unwrap()
			.unchecked_into::<js_sys::Function>();
		add.call1(&items, file).unwrap();
		let files = js_sys::Reflect::get(&transfer, &JsValue::from_str("files"))
			.unwrap()
			.unchecked_into::<web_sys::FileList>();
		input.set_files(Some(&files));
		input
			.dispatch_event(&web_sys::Event::new("change").unwrap())
			.unwrap();
		flush();
		assert_eq!(input.files().unwrap().length(), 1);
	}

	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	fn file_signals_never_inject_nonempty_values_and_none_or_reset_clears_selection() {
		reinhardt_pages::reactive::ReactiveScope::run(|| {
			// Arrange.
			let form = value_sync_form!();
			let runtime = use_form(&form).build();
			let parts = js_sys::Array::of1(&JsValue::from_str("fixture contents"));
			let file = web_sys::File::new_with_str_sequence(&parts, "fixture.txt").unwrap();
			form.attachment().set(Some(file.clone()));
			form.image().set(Some(file.clone()));
			let mounted = MountedForm::new(form.clone().into_page());
			let input = mounted
				.control("attachment")
				.unchecked_into::<web_sys::HtmlInputElement>();
			let image = mounted
				.control("image")
				.unchecked_into::<web_sys::HtmlInputElement>();
			assert_eq!(input.value(), "");
			assert_eq!(image.value(), "");

			// Act: files created in Rust must not synthesize a browser selection.
			let replacement =
				web_sys::File::new_with_str_sequence(&parts, "replacement.txt").unwrap();
			form.attachment().set(Some(replacement.clone()));
			form.image().set(Some(replacement));
			flush();
			for control in [&input, &image] {
				assert_eq!(control.value(), "");
				assert_eq!(control.files().unwrap().length(), 0);
				assert_eq!(control.get_attribute("value"), None);
			}

			// Act: an existing browser selection may only be cleared programmatically.
			select_file(&input, &file);
			select_file(&image, &file);
			assert_eq!(form.attachment().get().unwrap().name(), "fixture.txt");
			assert_eq!(form.image().get().unwrap().name(), "fixture.txt");
			form.attachment().set(None);
			form.image().set(None);
			flush();
			assert_eq!(input.files().unwrap().length(), 0);
			assert_eq!(image.files().unwrap().length(), 0);
			select_file(&input, &file);
			select_file(&image, &file);
			runtime.reset();
			flush();

			// Assert.
			assert_eq!(input.value(), "");
			assert_eq!(image.value(), "");
			assert_eq!(form.attachment().get(), None);
			assert_eq!(form.image().get(), None);
			assert!(input.is_same_node(Some(&mounted.control("attachment"))));
			assert!(image.is_same_node(Some(&mounted.control("image"))));
		})
	}
	#[rstest::rstest]
	#[test_attr(wasm_bindgen_test)]
	#[serial(form_value_sync_dom)]
	async fn native_reset_clears_rejected_numeric_editor_even_when_source_matches_default() {
		crate::form_scope::run(async {
			// Arrange: rejected input leaves the typed source equal to its default.
			let form = form! {
				name: NumericNativeReset,
				action: "/numeric-reset",
				fields: {
					amount: IntegerField { initial: 7 }
				}
			};
			let runtime = use_form(&form).build();
			let mounted = MountedForm::new(form.clone().into_page());
			let input = mounted
				.control("amount")
				.unchecked_into::<web_sys::HtmlInputElement>();
			input.set_value("");
			input
				.dispatch_event(&web_sys::Event::new("input").unwrap())
				.unwrap();
			assert_eq!(form.amount().get(), 7);
			assert_eq!(
				runtime
					.trigger()
					.expect_err("empty editor must fail")
					.field_errors()
					.len(),
				1
			);

			// Act.
			mounted
				.0
				.first_element_child()
				.unwrap()
				.unchecked_into::<web_sys::HtmlFormElement>()
				.reset();
			gloo_timers::future::TimeoutFuture::new(0).await;
			flush();

			// Assert: native defaults repair both the value and the rejected editor state.
			assert_eq!(input.value(), "7");
			assert_eq!(runtime.trigger(), Ok(()));
			assert_eq!(runtime.get_field_state(form.amount_field()).error, None);
			assert!(!runtime.form_state().is_touched.get());
		})
		.await;
	}
}
