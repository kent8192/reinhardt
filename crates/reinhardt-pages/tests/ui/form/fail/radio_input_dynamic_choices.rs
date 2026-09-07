use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidDynamicRadioInput,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choices_from: "answers",
			}
		}
	};
}
