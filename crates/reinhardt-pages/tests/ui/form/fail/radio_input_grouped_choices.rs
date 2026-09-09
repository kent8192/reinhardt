use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidGroupedRadioInputChoices,
		action: "/invalid",
		fields: {
			answer: ChoiceField {
				widget: RadioInput,
				choices: [OptGroup("Answers") {
					("yes", "Yes"),
				}, ],
			}
		}
	};
}
