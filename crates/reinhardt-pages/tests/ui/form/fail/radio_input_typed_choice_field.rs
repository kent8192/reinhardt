use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidTypedChoiceRadioInput,
		action: "/invalid",
		fields: {
			answer: ChoiceField<i64> {
				widget: RadioInput,
			}
		}
	};
}
