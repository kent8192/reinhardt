use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidRadioInputChoiceGroupDisabled,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choice_group_disabled: "disabled",
			}
		}
	};
}
