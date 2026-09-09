use reinhardt_pages::form;

fn main() {
	let _form = form! {
		name: InvalidIntegerRadioInput,
		action: "/invalid",
		fields: {
			answer: IntegerField {
				widget: RadioInput,
			}
		}
	};
}
