use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidBooleanRadioInput,
		action: "/invalid",
		fields: {
			answer: BooleanField {
				widget: RadioInput,
			}
		}
	};
}
