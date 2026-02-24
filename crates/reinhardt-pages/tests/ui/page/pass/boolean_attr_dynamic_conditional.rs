//! page! macro with boolean attributes using conditional expressions

use reinhardt_pages::page;

fn main() {
	let condition = true;
	let count = 5;

	let _valid = page!(|| {
		div {
			button {
				disabled: condition,
				"Submit"
			}
			input {
				r#type: "checkbox",
				checked: count> 3,
			}
			input {
				r#type: "text",
				required: count<10 &&condition,
			}
		}
	});
}
