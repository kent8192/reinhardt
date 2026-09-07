use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidMultipleChoiceRadioInput,
		action: "/invalid",
		fields: {
			answer: MultipleChoiceField<String> {
				widget: RadioInput,
			}
		}
	};
}
