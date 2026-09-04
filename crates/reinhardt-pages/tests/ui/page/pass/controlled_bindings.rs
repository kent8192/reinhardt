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
		let number = Signal::new(0_i64);
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
				a11y: off,
				type: "search",
				bind: text
			}
			input {
				a11y: off,
				type: "tel",
				bind: text
			}
			input {
				a11y: off,
				type: "url",
				bind: text
			}
			input {
				a11y: off,
				type: "email",
				bind: text
			}
			input {
				a11y: off,
				type: "password",
				bind: text
			}
			input {
				a11y: off,
				type: "color",
				bind: text
			}
			input {
				a11y: off,
				type: "date",
				bind: text
			}
			input {
				a11y: off,
				type: "datetime-local",
				bind: text
			}
			input {
				a11y: off,
				type: "month",
				bind: text
			}
			input {
				a11y: off,
				type: "week",
				bind: text
			}
			input {
				a11y: off,
				type: "time",
				bind: text
			}
			input {
				a11y: off,
				type: "range",
				bind: number
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
	});
}
