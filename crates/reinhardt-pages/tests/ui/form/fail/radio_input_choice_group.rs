use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidRadioInputChoiceGroup,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choice_group: "group",
			}
		}
	};
}
