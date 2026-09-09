//! Single radio inputs support string choices, groups, and collections.

use reinhardt_pages::{form, use_form};

fn main() {
	let form = form! {
		name: RadioInputForm,
		action: "/radio-input",
		fields: {
			answer: ChoiceField<String> {
				label: "Answer",
				widget: RadioInput,
			}
			native: ChoiceField {
				widget: RadioInput,
				initial: "on",
			}
			qualified: ChoiceField<::std::string::String> {
				widget: RadioInput,
				choices: [("yes", "Yes")],
				initial: "yes",
			}
			disabled: ChoiceField {
				widget: RadioInput,
				choices: [("yes", "Yes") { disabled }],
			}
			snapshot: ChoiceField {
				widget: RadioInput,
				choices: [("yes", "Yes")],
				bind: false,
			}
			preferences: FieldGroup {
				label: "Preferences",
				fields: {
					contact: ChoiceField {
						widget: RadioInput,
						choices: [("email", "Email")],
					}
				}
			}
			members: FieldArray {
				fields: {
					confirmed: ChoiceField {
						widget: RadioInput,
						choices: [("yes", "Confirmed")],
					}
				}
			}
		}
	};

	assert_eq!(form.answer().get(), "");
	assert_eq!(form.native().get(), "on");
	assert_eq!(form.qualified().get(), "yes");
	assert_eq!(form.contact().get(), "");
	assert_eq!(form.new_members_item().confirmed, "");
	assert_eq!(use_form(&form).build().get_values().qualified, "yes");
}
