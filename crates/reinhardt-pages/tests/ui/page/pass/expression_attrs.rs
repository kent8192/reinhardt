//! page! macro with expression-based attribute values

use reinhardt_pages::page;

fn main() {
	// Dynamic class attribute with conditional
	let _dynamic_class = page!(|is_active: bool| {
		div {
			class: if is_active { "active" } else { "inactive" },
			"Content"
		}
	});

	// Static attributes
	let _static_attrs = page!(|| {
		div {
			class: "container",
			id: "main",
			"Styled content"
		}
	});
}
