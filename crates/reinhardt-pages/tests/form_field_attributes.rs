//! Regression coverage for native form attributes and accessible help text.

use reinhardt_pages::{Page, form, use_form};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
const HELP: &str = "<strong>Use \"full\" name & details.</strong>";
const ESCAPED_HELP: &str = "&lt;strong&gt;Use &quot;full&quot; name &amp; details.&lt;/strong&gt;";

type AttributeExpectation<'a> = (&'a str, Option<&'a str>);
type HelpExpectation<'a> = (&'a str, &'a str, Option<&'a str>);

fn element_html<'a>(html: &'a str, id: &str) -> &'a str {
	let position = html
		.find(&format!(" id=\"{id}\""))
		.unwrap_or_else(|| panic!("missing rendered element #{id}: {html}"));
	&html[html[..position].rfind('<').expect("element opening tag")..]
}

fn attribute_values<'a>(tag: &'a str, name: &str) -> Vec<&'a str> {
	tag.split(&format!(" {name}=\""))
		.skip(1)
		.map(|value| value.split_once('"').expect("attribute closing quote").0)
		.collect()
}

fn assert_rendered_fields(
	page: Page,
	expected: &[(&str, &[AttributeExpectation<'_>])],
	help_elements: &[HelpExpectation<'_>],
) {
	// Arrange: inspect the actual SSR serialization, including duplicate attributes.
	let html = page.render_to_string();
	for (id, attributes) in expected {
		assert_eq!(html.matches(&format!(" id=\"{id}\"")).count(), 1);
		let tag = element_html(&html, id).split_once('>').unwrap().0;
		for (name, value) in *attributes {
			assert_eq!(
				attribute_values(tag, name),
				value.iter().copied().collect::<Vec<_>>(),
				"SSR #{id} {name}"
			);
		}
	}
	for (id, expected_tag, parent_id) in help_elements {
		let (tag, contents) = element_html(&html, id).split_once('>').unwrap();
		assert_eq!(tag[1..].split_whitespace().next(), Some(*expected_tag));
		assert_eq!(attribute_values(tag, "class"), ["reinhardt-help"]);
		assert_eq!(contents.split_once('<').unwrap().0, ESCAPED_HELP);
		assert_eq!(html.matches(&format!(" id=\"{id}\"")).count(), 1);
		if let Some(parent_id) = parent_id {
			let parent = element_html(&html, parent_id);
			let parent_tag = parent[1..].split_whitespace().next().unwrap();
			let closing_tag = format!("</{parent_tag}>");
			let parent_contents = parent.split_once(closing_tag.as_str()).unwrap().0;
			assert_eq!(parent_contents.matches(&format!(" id=\"{id}\"")).count(), 1);
		}
	}

	#[cfg(target_arch = "wasm32")]
	{
		use reinhardt_pages::portal::{PortalTarget, mount_portal};
		use wasm_bindgen::JsValue;

		// Act: the portal owns the mounted DOM and reactive subscriptions.
		let _portal = mount_portal(PortalTarget::body(), page).expect("mount form");
		let document = web_sys::window().unwrap().document().unwrap();
		for (id, attributes) in expected {
			let element = document.get_element_by_id(id).expect("mounted control");
			for (name, value) in *attributes {
				assert_eq!(
					element.get_attribute(name).as_deref(),
					*value,
					"WASM #{id} {name}"
				);
				if matches!(*name, "disabled" | "readonly" | "required" | "autofocus") {
					let property = if *name == "readonly" {
						"readOnly"
					} else {
						name
					};
					let actual = js_sys::Reflect::get(&element, &JsValue::from_str(property))
						.expect("native control property");
					if let Some(actual) = actual.as_bool() {
						assert_eq!(actual, value.is_some(), "WASM #{id} {property}");
					}
				}
			}
		}
		for (id, expected_tag, parent_id) in help_elements {
			let help = document.get_element_by_id(id).expect("mounted help text");
			assert_eq!(help.tag_name(), expected_tag.to_ascii_uppercase());
			assert_eq!(help.text_content().as_deref(), Some(HELP));
			assert_eq!(help.child_element_count(), 0, "help must remain plain text");
			assert_eq!(
				help.get_attribute("class").as_deref(),
				Some("reinhardt-help")
			);
			assert_eq!(
				document
					.query_selector_all(&format!("[id=\"{id}\"]"))
					.unwrap()
					.length(),
				1
			);
			if let Some(parent_id) = parent_id {
				assert_eq!(
					help.parent_element(),
					document.get_element_by_id(parent_id),
					"help must stay inside its custom wrapper"
				);
			}
		}
	}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn built_in_and_grouped_fields_preserve_applicable_metadata() {
	// Arrange
	let form = form! {
		name: AttributeForm,
		action: "/attributes",
		fields: {
			name: CharField {
				label: "Name",
				required,
				min_length: 2,
				max_length: 40,
				pattern: "[A-Za-z ]+",
				autofocus,
				disabled: false,
				readonly: false,
				placeholder: "Full name",
				autocomplete: "name",
				help_text: "<strong>Use \"full\" name & details.</strong>",
				attrs: {
					aria_describedby: "external-description",
					data_testid: "name-control"
				},
			}
			name_help: CharField {}
			paragraph: CharField {
				wrapper: p {
					id: "paragraph-wrapper"
				},
				help_text: "<strong>Use \"full\" name & details.</strong>",
			}
			bio: TextField {
				required: true,
				disabled: true,
				readonly: true,
				autofocus: false,
				min_length: 3,
				max_length: 120,
				pattern: "[A-Z]+",
				placeholder: "Biography",
				autocomplete: "off",
				help_text: "<strong>Use \"full\" name & details.</strong>",
			}
			locked: CharField { disabled }
			reference: CharField { readonly }
			enabled: CharField {
				required: false,
				disabled: false,
				readonly: false,
				autofocus: false,
				attrs: {
					aria_describedby: "existing-description"
				},
			}
			count: IntegerField {
				min_value: 1,
				max_value: 100,
				readonly: true,
				min_length: 2,
				max_length: 3,
				pattern: "[0-9]+",
			}
			min_override: IntegerField {
				min_value: 1,
				max_value: 10,
				min: 3,
				step: 2
			}
			max_override: FloatField {
				min_value: 1,
				max_value: 10,
				max: 8,
				step: "0.5"
			}
			range: IntegerField {
				widget: RangeInput,
				min_value: 1,
				max_value: 10,
				readonly
			}
			check: BooleanField {
				required,
				disabled,
				readonly,
				min_length: 2,
				pattern: "true",
				help_text: "<strong>Use \"full\" name & details.</strong>",
			}
			radio: ChoiceField<i64> {
				widget: RadioSelect,
				required: true,
				disabled: true,
				readonly: true,
				choices_from: "choices",
				choice_value: "value",
				choice_label: "label",
				help_text: "<strong>Use \"full\" name & details.</strong>",
				attrs: {
					aria_describedby: "radio-instructions"
				},
			}
			select: ChoiceField<String> {
				widget: Select,
				required,
				disabled,
				readonly,
				choices: [("a", "First"), ("b", "Second")],
				autocomplete: "off",
				help_text: "<strong>Use \"full\" name & details.</strong>",
			}
			multi: MultipleChoiceField<String> {
				widget: SelectMultiple,
				bind: false,
				disabled: false,
				readonly: true,
				choices: [("a", "First"), ("b", "Second")],
			}
			avatar: FileField {
				accept: "image/png",
				capture: "environment",
				disabled: true,
				readonly: true
			}
			address: FieldGroup {
				label: "Address",
				fields: {
					street: CharField {
						min_length: 4,
						max_length: 80,
						readonly,
						disabled: true,
						help_text: "<strong>Use \"full\" name & details.</strong>",
					}
				}
			}
		}
	};
	form.radio_choices()
		.set(vec![(1, "First".into()), (2, "Second".into())]);

	// Act / Assert
	assert_rendered_fields(
		form.into_page(),
		&[
			(
				"name",
				&[
					("minlength", Some("2")),
					("maxlength", Some("40")),
					("pattern", Some("[A-Za-z ]+")),
					("autofocus", Some("autofocus")),
					("required", Some("required")),
					("disabled", None),
					("readonly", None),
					("placeholder", Some("Full name")),
					("autocomplete", Some("name")),
					("data-testid", Some("name-control")),
					("aria-describedby", Some("external-description name--help")),
				],
			),
			(
				"name_help",
				&[
					("name", Some("name_help")),
					("type", Some("text")),
					("aria-describedby", None),
				],
			),
			(
				"paragraph",
				&[("aria-describedby", Some("paragraph--help"))],
			),
			(
				"bio",
				&[
					("minlength", Some("3")),
					("maxlength", Some("120")),
					("pattern", None),
					("required", Some("required")),
					("disabled", Some("disabled")),
					("readonly", Some("readonly")),
					("autofocus", None),
					("placeholder", Some("Biography")),
					("autocomplete", Some("off")),
					("aria-describedby", Some("bio--help")),
				],
			),
			(
				"locked",
				&[("disabled", Some("disabled")), ("aria-describedby", None)],
			),
			("reference", &[("readonly", Some("readonly"))]),
			(
				"enabled",
				&[
					("required", None),
					("disabled", None),
					("readonly", None),
					("autofocus", None),
					("aria-describedby", Some("existing-description")),
				],
			),
			(
				"count",
				&[
					("min", Some("1")),
					("max", Some("100")),
					("readonly", Some("readonly")),
					("minlength", None),
					("maxlength", None),
					("pattern", None),
				],
			),
			(
				"min_override",
				&[("min", Some("3")), ("max", Some("10")), ("step", Some("2"))],
			),
			(
				"max_override",
				&[
					("min", Some("1")),
					("max", Some("8")),
					("step", Some("0.5")),
				],
			),
			(
				"range",
				&[("min", Some("1")), ("max", Some("10")), ("readonly", None)],
			),
			(
				"check",
				&[
					("required", Some("required")),
					("disabled", Some("disabled")),
					("readonly", None),
					("minlength", None),
					("pattern", None),
					("aria-describedby", Some("check--help")),
				],
			),
			(
				"radio_0",
				&[
					("disabled", Some("disabled")),
					("readonly", None),
					("required", Some("required")),
					("aria-describedby", Some("radio-instructions radio--help")),
				],
			),
			(
				"radio_1",
				&[
					("disabled", Some("disabled")),
					("readonly", None),
					("aria-describedby", Some("radio-instructions radio--help")),
				],
			),
			(
				"select",
				&[
					("required", Some("required")),
					("disabled", Some("disabled")),
					("readonly", None),
					("autocomplete", Some("off")),
					("aria-describedby", Some("select--help")),
				],
			),
			(
				"multi",
				&[
					("multiple", Some("multiple")),
					("disabled", None),
					("readonly", None),
				],
			),
			(
				"avatar",
				&[
					("accept", Some("image/png")),
					("capture", Some("environment")),
					("disabled", Some("disabled")),
					("readonly", None),
				],
			),
			(
				"street",
				&[
					("minlength", Some("4")),
					("maxlength", Some("80")),
					("readonly", Some("readonly")),
					("disabled", Some("disabled")),
					("aria-describedby", Some("street--help")),
				],
			),
		],
		&[
			("name--help", "p", None),
			("paragraph--help", "span", Some("paragraph-wrapper")),
			("bio--help", "p", None),
			("check--help", "p", None),
			("radio--help", "p", None),
			("select--help", "p", None),
			("street--help", "p", None),
		],
	);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
async fn dynamic_radio_autofocus_is_single_and_choice_disabled_is_preserved() {
	#[cfg(target_arch = "wasm32")]
	struct Choices {
		items: Vec<Choice>,
	}
	#[cfg(target_arch = "wasm32")]
	struct Choice {
		value: i64,
		label: String,
		disabled: bool,
	}
	#[cfg(target_arch = "wasm32")]
	async fn load_choices() -> Result<Choices, reinhardt_pages::ServerFnError> {
		Ok(Choices {
			items: vec![
				Choice {
					value: 1,
					label: "First".into(),
					disabled: false,
				},
				Choice {
					value: 2,
					label: "Second".into(),
					disabled: true,
				},
			],
		})
	}

	// Arrange
	let form = form! {
		name: RadioAutofocusForm,
		action: "/radio-autofocus",
		choices_loader: load_choices,
		fields: {
			focused: ChoiceField<i64> {
				widget: RadioSelect,
				autofocus: true,
				disabled: false,
				choices_from: "items",
				choice_value: "value",
				choice_label: "label",
				choice_disabled: "disabled",
			}
		}
	};
	form.focused_choices()
		.set(vec![(1, "First".into()), (2, "Second".into())]);

	// Act: native SSR uses its supplied choices; the WASM loader adds choice metadata.
	#[cfg(target_arch = "wasm32")]
	form.load_choices().await.expect("load choice metadata");

	// Assert
	assert_rendered_fields(
		form.into_page(),
		&[
			(
				"focused_0",
				&[("autofocus", Some("autofocus")), ("disabled", None)],
			),
			(
				"focused_1",
				&[
					("autofocus", None),
					(
						"disabled",
						cfg!(target_arch = "wasm32").then_some("disabled"),
					),
				],
			),
		],
		&[],
	);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn collection_fields_preserve_metadata_and_indexed_help_links_after_reordering() {
	// Arrange
	let form = form! {
		name: CollectionAttributeForm,
		action: "/collection-attributes",
		fields: {
			items: FieldArray {
				fields: {
					name: CharField {
						required: true,
						min_length: 2,
						max_length: 40,
						pattern: "[A-Za-z ]+",
						disabled: true,
						readonly: true,
						autofocus: false,
						placeholder: "Item name",
						autocomplete: "off",
						help_text: "<strong>Use \"full\" name & details.</strong>",
						attrs: {
							aria_describedby: "collection-instructions",
							data_testid: "item-name"
						},
					}
					notes: TextField {
						min_length: 3,
						max_length: 120,
						pattern: "[A-Z]+",
						readonly,
						disabled: false,
						required: false,
						help_text: "<strong>Use \"full\" name & details.</strong>",
					}
					quantity: IntegerField {
						min_value: 1,
						max_value: 10,
						min: 3,
						step: 2,
						readonly
					}
					ratio: FloatField {
						min_value: 1,
						max_value: 10,
						max: 8,
						step: "0.5"
					}
					check: BooleanField {
						disabled,
						readonly
					}
					radio: ChoiceField<String> {
						widget: RadioSelect,
						disabled,
						readonly
					}
					select: ChoiceField<String> {
						widget: Select,
						disabled,
						readonly
					}
				}
			}
		}
	};
	let runtime = use_form(&form).build();
	let collection = form.items_collection();
	let mut first = form.new_items_item();
	first.name = "First".into();
	let mut second = form.new_items_item();
	second.name = "Second".into();
	runtime.push_item(collection, first);
	let second_key = runtime.push_item(collection, second);

	// Act: reordering must keep each description associated with the rendered index.
	assert_eq!(runtime.move_item(collection, second_key, 0), Some((1, 0)));

	// Assert
	assert_rendered_fields(
		form.into_page(),
		&[
			(
				"items_0_name",
				&[
					("name", Some("items[0][name]")),
					("value", Some("Second")),
					("minlength", Some("2")),
					("maxlength", Some("40")),
					("pattern", Some("[A-Za-z ]+")),
					("required", Some("required")),
					("disabled", Some("disabled")),
					("readonly", Some("readonly")),
					("autofocus", None),
					("placeholder", Some("Item name")),
					("autocomplete", Some("off")),
					("data-testid", Some("item-name")),
					(
						"aria-describedby",
						Some("collection-instructions items_0_name--help"),
					),
				],
			),
			(
				"items_1_name",
				&[
					("name", Some("items[1][name]")),
					("value", Some("First")),
					("minlength", Some("2")),
					("disabled", Some("disabled")),
					(
						"aria-describedby",
						Some("collection-instructions items_1_name--help"),
					),
				],
			),
			(
				"items_0_notes",
				&[
					("minlength", Some("3")),
					("maxlength", Some("120")),
					("pattern", None),
					("readonly", Some("readonly")),
					("disabled", None),
					("required", None),
					("aria-describedby", Some("items_0_notes--help")),
				],
			),
			(
				"items_0_quantity",
				&[
					("min", Some("3")),
					("max", Some("10")),
					("step", Some("2")),
					("readonly", Some("readonly")),
				],
			),
			(
				"items_0_ratio",
				&[
					("min", Some("1")),
					("max", Some("8")),
					("step", Some("0.5")),
				],
			),
			(
				"items_0_check",
				&[
					("type", Some("checkbox")),
					("disabled", Some("disabled")),
					("readonly", None),
				],
			),
			(
				"items_0_radio",
				&[
					("type", Some("radio")),
					("disabled", Some("disabled")),
					("readonly", None),
				],
			),
			(
				"items_0_select",
				&[("disabled", Some("disabled")), ("readonly", None)],
			),
		],
		&[
			("items_0_name--help", "p", None),
			("items_1_name--help", "p", None),
			("items_0_notes--help", "p", None),
			("items_1_notes--help", "p", None),
		],
	);
}
