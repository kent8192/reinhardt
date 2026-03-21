//! page! macro with mixed standalone and key-value boolean attributes

use reinhardt_pages::page;

fn main() {
	let is_readonly = true;

	let _valid = page!(|| {
		div {
			input {
				r#type: "text",
				required: true,
				class: "form-input",
				readonly: is_readonly,
			}
		}
	});
}
