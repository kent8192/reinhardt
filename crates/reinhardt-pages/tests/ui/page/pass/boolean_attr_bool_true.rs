//! page! macro with boolean attribute having boolean literal true

use reinhardt_pages::page;

fn main() {
	let _invalid = page!(|| {
		button {
			disabled: true,
			"Submit"
		}
	});
}
