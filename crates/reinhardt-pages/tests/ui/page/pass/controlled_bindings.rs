use reinhardt_core::types::page::NumberParseError;
use reinhardt_pages::page;
use reinhardt_pages::reactive::{ReactiveScope, Signal};

#[derive(Clone)]
struct Fields {
	name: Signal<String>,
}

impl Fields {
	fn name(&self) -> &Signal<String> {
		&self.name
	}
}

fn main() {
	ReactiveScope::run(|| {
		let text = Signal::new(String::new());
		let checked = Signal::new(false);
		let radio = Signal::new("draft".to_owned());
		let mut number = Signal::new(0_i64);
		let number_error = Signal::new(None::<NumberParseError>);
		let selected = Signal::new(String::new());
		let selected_many = Signal::new(Vec::<String>::new());
		let fields = Fields {
			name: Signal::new(String::new()),
		};

		let _ = page!({
			input {
				a11y: off,
				bind: text
			}
			textarea {
				a11y: off,
				bind: fields.name()
			}
			input {
				a11y: off,
				type: "checkbox",
				bind: checked
			}
			input {
				a11y: off,
				type: "radio",
				value: "draft",
				bind: radio
			}
			input {
				a11y: off,
				type: "number",
				bind: number(number, number_error)
			}
			input {
				aria_label: "Borrowed number with parse error",
				type: "number",
				bind: number(&number, &number_error)
			}
			select {
				a11y: off,
				bind: selected,
				option {
					value: "a",
					"A"
				}
			}
			select {
				a11y: off,
				multiple: true,
				bind: selected_many,
				option {
					value: "a",
					"A"
				}
			}
		});
		let mutable_number = page!(|number: &mut Signal<i64>| {
			input {
				aria_label: "Borrowed number",
				type: "number",
				bind: number
			}
		});
		let _ = mutable_number(&mut number);
	});
}
