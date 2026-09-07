use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidEmptyRadioInputChoices,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choices: [],
			}
		}
	};
}
