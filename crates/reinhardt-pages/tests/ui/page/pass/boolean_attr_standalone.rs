//! page! macro with standalone boolean attributes

use reinhardt_pages::page;

fn main() {
	let _valid = page!(|| {
		div {
			input {
				r#type: "text",
				required: true,
			}
			button {
				disabled: true,
				"Submit"
			}
			select {
				multiple: true,
				option {
					"A"
				}
				option {
					"B"
				}
			}
		}
	});
}
