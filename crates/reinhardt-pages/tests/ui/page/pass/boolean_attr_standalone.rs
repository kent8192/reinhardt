//! page! macro with standalone boolean attributes

use reinhardt_pages::page;

fn main() {
	let _valid = page!(|| {
		div {
			input {
				r#type: "text",
				required
			}
			button {
				disabled
				"Submit"
			}
			select {
				multiple
				option { "A" }
				option { "B" }
			}
		}
	});
}
