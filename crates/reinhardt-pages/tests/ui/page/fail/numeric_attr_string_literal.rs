//! page! macro with numeric attribute having string literal

use reinhardt_pages::page;

fn main() {
	let _invalid = page!(|| {
		input {
			r#type: "text",
			maxlength: "100",
		}
	});
}
