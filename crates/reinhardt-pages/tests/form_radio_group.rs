//! Regression coverage for generated radio group names and option labels.

use reinhardt_pages::{Page, form, reactive::Signal};

#[cfg(wasm)]
use wasm_bindgen_test::*;

#[cfg(wasm)]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg(wasm)]
struct ChoiceItem {
	value: String,
	label: String,
}

#[cfg(wasm)]
struct Choices {
	plans: Vec<ChoiceItem>,
}

#[cfg(wasm)]
async fn load_choices() -> Result<Choices, reinhardt_pages::ServerFnError> {
	Ok(Choices { plans: Vec::new() })
}

struct RadioFixture {
	page: Page,
	choices: Signal<Vec<(String, String)>>,
	selected: Signal<String>,
}

// form! compares arbitrary choices through Display/ToString; this String fixture
// intentionally exercises the public configuration from the regression report.
#[allow(clippy::cmp_owned)]
fn radio_fixture() -> RadioFixture {
	let form = form! {
		name: RadioGroupProbe,
		action: "/test",
		choices_loader: load_choices,
		fields: {
			plan: ChoiceField<String> {
				label: "Plan",
				widget: RadioSelect,
				initial: "pro",
				choices_from: "plans",
			}
		}
	};
	// Keep the loader contract checked without polling it or replacing seeded choices.
	drop(form.load_choices());
	let choices = form.plan_choices().clone();
	let selected = form.plan().clone();
	choices.set(vec![
		("basic".to_owned(), "Basic".to_owned()),
		("pro".to_owned(), "Pro".to_owned()),
	]);
	RadioFixture {
		page: form.into_page(),
		choices,
		selected,
	}
}

// form! compares arbitrary choices through Display/ToString; keep the same String
// configuration while exercising custom wrappers and surrounding field labels.
#[allow(clippy::cmp_owned)]
fn configured_form_page() -> Page {
	let form = form! {
		name: ConfiguredRadioGroups,
		action: "/test",
		choices_loader: load_choices,
		fields: {
			custom: ChoiceField<String> {
				label: "Custom plan",
				widget: RadioSelect,
				choices_from: "plans",
				wrapper: section {
					id: "custom-wrapper",
					class: "custom-field",
					title: "Plans",
					role: "radiogroup"
				},
				label_class: "custom-label",
				class: "custom-input",
			}
			direct: ChoiceField<String> {
				label: "Direct plan",
				widget: RadioSelect,
				choices_from: "plans",
				wrapper: p {
					id: "direct"
				},
				wrapper_class: "direct-field",
			}
			subscription: FieldGroup {
				label: "Subscription",
				fields: {
					tier: ChoiceField<String> {
						widget: RadioSelect,
						choices_from: "plans",
					}
				}
			}
			username: CharField {
				label: "Username"
			}
			standalone: ChoiceField<String> {
				label: "Standalone",
				widget: RadioSelect
			}
		}
	};
	for choices in [
		form.custom_choices(),
		form.direct_choices(),
		form.tier_choices(),
	] {
		choices.set(vec![("basic".to_owned(), "Basic".to_owned())]);
	}
	form.into_page()
}

fn fields(page: &Page) -> &[Page] {
	let Page::Element(form) = page else {
		panic!("generated form must be an element");
	};
	// Default POST forms start with an automatically injected CSRF input.
	&form.child_views()[1..]
}

#[cfg_attr(native, test)]
#[cfg_attr(wasm, wasm_bindgen_test)]
fn radio_group_ssr_names_the_group_and_keeps_option_labels() {
	// Arrange: use a valid loader contract and seed choices before rendering.
	let fixture = radio_fixture();

	// Act: render the generated field, including its reactive choices.
	let html = fields(&fixture.page)[0].render_to_string();

	// Assert: the legend names the fieldset and every label targets its own input.
	assert_eq!(
		html,
		concat!(
			"<fieldset class=\"reinhardt-field\"><legend class=\"reinhardt-label\">Plan</legend>",
			"<label for=\"plan_0\"><input type=\"radio\" name=\"plan\" id=\"plan_0\" value=\"basic\" class=\"reinhardt-input\" />Basic</label>",
			"<label for=\"plan_1\"><input type=\"radio\" name=\"plan\" id=\"plan_1\" value=\"pro\" class=\"reinhardt-input\" checked=\"checked\" />Pro</label>",
			"</fieldset>",
		)
	);
	assert_eq!(fixture.selected.get(), "pro");

	fixture.choices.set(Vec::new());
	assert_eq!(
		fields(&fixture.page)[0].render_to_string(),
		"<fieldset class=\"reinhardt-field\"><legend class=\"reinhardt-label\">Plan</legend></fieldset>",
	);
	assert_eq!(fixture.selected.get(), "pro");
}

#[cfg_attr(native, test)]
#[cfg_attr(wasm, wasm_bindgen_test)]
fn configured_radio_wrappers_preserve_semantics_and_other_control_labels() {
	// Arrange: combine custom wrappers, a parent FieldGroup, and ordinary fields.
	let page = configured_form_page();

	// Act: render each generated field independently of the form's attributes.
	let html: Vec<_> = fields(&page).iter().map(Page::render_to_string).collect();

	// Assert: native groups and custom wrappers have valid group names.
	assert_eq!(
		html,
		[
			concat!(
				"<section id=\"custom-wrapper\" class=\"custom-field\" title=\"Plans\" role=\"radiogroup\" aria-labelledby=\"custom--label\">",
				"<span id=\"custom--label\" class=\"custom-label\">Custom plan</span>",
				"<label for=\"custom_0\"><input type=\"radio\" name=\"custom\" id=\"custom_0\" value=\"basic\" class=\"custom-input\" />Basic</label>",
				"</section>",
			),
			concat!(
				"<p id=\"direct\" class=\"direct-field\" role=\"group\" aria-labelledby=\"direct--label\"><span id=\"direct--label\" class=\"reinhardt-label\">Direct plan</span>",
				"<label for=\"direct_0\"><input type=\"radio\" name=\"direct\" id=\"direct_0\" value=\"basic\" class=\"reinhardt-input\" />Basic</label>",
				"</p>",
			),
			concat!(
				"<fieldset class=\"reinhardt-field-group\"><legend class=\"reinhardt-field-group-label\">Subscription</legend>",
				"<fieldset class=\"reinhardt-field\"><legend class=\"reinhardt-label\">tier</legend>",
				"<label for=\"tier_0\"><input type=\"radio\" name=\"tier\" id=\"tier_0\" value=\"basic\" class=\"reinhardt-input\" />Basic</label>",
				"</fieldset></fieldset>",
			),
			concat!(
				"<div class=\"reinhardt-field\"><label for=\"username\" class=\"reinhardt-label\">Username</label>",
				"<input type=\"text\" name=\"username\" id=\"username\" class=\"reinhardt-input\" placeholder=\"\" value=\"\" /></div>",
			),
			concat!(
				"<div class=\"reinhardt-field\"><label for=\"standalone\" class=\"reinhardt-label\">Standalone</label>",
				"<input type=\"radio\" name=\"standalone\" id=\"standalone\" class=\"reinhardt-input\" /></div>",
			),
		]
	);
}

#[cfg(wasm)]
mod browser {
	use super::*;
	use reinhardt_pages::portal::{PortalTarget, mount_portal};
	use wasm_bindgen::JsCast;

	fn element(parent: &web_sys::Element, selector: &str) -> web_sys::Element {
		parent.query_selector(selector).unwrap().expect(selector)
	}

	fn assert_options(group: &web_sys::Element, expected: &[(&str, &str, bool)]) {
		let inputs = group.query_selector_all("input[type=radio]").unwrap();
		assert_eq!(inputs.length() as usize, expected.len());
		assert_eq!(
			group.query_selector_all("label").unwrap().length() as usize,
			expected.len()
		);
		for (index, (value, name, checked)) in expected.iter().enumerate() {
			let input = inputs
				.item(index as u32)
				.unwrap()
				.dyn_into::<web_sys::HtmlInputElement>()
				.unwrap();
			let label = input.parent_element().unwrap();
			assert_eq!(label.tag_name(), "LABEL");
			assert_eq!(label.get_attribute("for"), Some(input.id()));
			assert_eq!(input.labels().unwrap().length(), 1);
			assert_eq!(label.text_content().as_deref(), Some(*name));
			assert_eq!(input.id(), format!("plan_{index}"));
			assert_eq!(input.name(), "plan");
			assert_eq!(input.value(), *value);
			assert_eq!(input.checked(), *checked);
		}
	}

	#[wasm_bindgen_test]
	#[serial_test::serial(form_radio_group)]
	fn mounted_custom_wrappers_keep_named_groups_and_valid_labels() {
		// Arrange: mount the configured form with automatic subtree cleanup.
		let _mount = mount_portal(PortalTarget::body(), configured_form_page()).unwrap();
		let document = web_sys::window().unwrap().document().unwrap();
		let form = document
			.get_element_by_id("configured-radio-groups")
			.unwrap();

		// Act: inspect the mounted wrappers and each native group's direct legend.
		let custom = element(&form, "#custom-wrapper");
		let direct = element(&form, "#direct");
		let legends = form.query_selector_all("fieldset > legend").unwrap();
		let names: Vec<_> = (0..legends.length())
			.map(|index| legends.item(index).unwrap().text_content().unwrap())
			.collect();

		// Assert: wrapper customization does not remove or duplicate the group name.
		assert_eq!(custom.tag_name(), "SECTION");
		assert_eq!(custom.class_name(), "custom-field");
		assert_eq!(custom.get_attribute("title").as_deref(), Some("Plans"));
		assert_eq!(
			element(&custom, ":scope > span").class_name(),
			"custom-label"
		);
		assert_eq!(element(&custom, "input").class_name(), "custom-input");
		assert_eq!(direct.tag_name(), "P");
		assert_eq!(direct.class_name(), "direct-field");
		assert_eq!(direct.query_selector_all("fieldset").unwrap().length(), 0);
		assert_eq!(names, ["Subscription", "tier"]);
		for (wrapper, id, name, role) in [
			(&custom, "custom--label", "Custom plan", "radiogroup"),
			(&direct, "direct--label", "Direct plan", "group"),
		] {
			assert_eq!(wrapper.get_attribute("role").as_deref(), Some(role));
			assert_eq!(
				wrapper.get_attribute("aria-labelledby").as_deref(),
				Some(id)
			);
			let label = element(wrapper, ":scope > span");
			assert_eq!(label.id(), id);
			assert_eq!(label.text_content().as_deref(), Some(name));
			assert_eq!(
				wrapper
					.query_selector_all(":scope > label > input")
					.unwrap()
					.length(),
				1
			);
		}
		assert_eq!(
			element(&form, ".reinhardt-field-group > fieldset > legend")
				.text_content()
				.as_deref(),
			Some("tier")
		);

		let labels = form.query_selector_all("label").unwrap();
		assert_eq!(labels.length(), 5);
		for index in 0..labels.length() {
			let label = labels
				.item(index)
				.unwrap()
				.dyn_into::<web_sys::Element>()
				.unwrap();
			let target = label.get_attribute("for").unwrap();
			let input = element(&form, &format!("#{target}"))
				.dyn_into::<web_sys::HtmlInputElement>()
				.unwrap();
			assert_eq!(input.labels().unwrap().length(), 1);
			assert_eq!(input.labels().unwrap().item(0).unwrap(), label.into());
		}
	}

	#[wasm_bindgen_test]
	#[serial_test::serial(form_radio_group)]
	fn mounted_radio_group_survives_choice_and_selection_changes() {
		// Arrange: PortalHandle owns both mounted nodes and their reactive cleanup.
		let fixture = radio_fixture();
		let _mount = mount_portal(PortalTarget::body(), fixture.page).unwrap();
		let document = web_sys::window().unwrap().document().unwrap();
		let form = document.get_element_by_id("radio-group-probe").unwrap();
		let group = element(&form, "fieldset");
		let legend = element(&group, ":scope > legend");

		// Act and assert: individual names and initial selection survive native mounting.
		assert_eq!(form.query_selector_all("fieldset").unwrap().length(), 1);
		assert_eq!(
			form.query_selector_all("label[for=plan]").unwrap().length(),
			0
		);
		assert_eq!(legend.text_content().as_deref(), Some("Plan"));
		assert_eq!(legend.class_name(), "reinhardt-label");
		assert_options(&group, &[("basic", "Basic", false), ("pro", "Pro", true)]);

		element(&group, "#plan_0")
			.dyn_into::<web_sys::HtmlInputElement>()
			.unwrap()
			.click();
		assert_eq!(fixture.selected.get(), "basic");
		assert_options(&group, &[("basic", "Basic", true), ("pro", "Pro", false)]);

		fixture.choices.set(vec![
			("enterprise".to_owned(), "Enterprise".to_owned()),
			("basic".to_owned(), "Basic renamed".to_owned()),
		]);
		assert_options(
			&group,
			&[
				("enterprise", "Enterprise", false),
				("basic", "Basic renamed", true),
			],
		);
		assert_eq!(element(&form, "fieldset"), group);
		assert_eq!(element(&group, ":scope > legend"), legend);

		fixture.choices.set(Vec::new());
		assert_options(&group, &[]);
		assert_eq!(element(&form, "fieldset"), group);
		assert_eq!(legend.text_content().as_deref(), Some("Plan"));
		assert_eq!(fixture.selected.get(), "basic");

		fixture
			.choices
			.set(vec![("basic".to_owned(), "Basic restored".to_owned())]);
		assert_options(&group, &[("basic", "Basic restored", true)]);
	}
}
