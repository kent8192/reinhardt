use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidMultipleRadioInputChoices,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choices: [("yes", "Yes"), ("no", "No")],
			}
		}
	};
}
