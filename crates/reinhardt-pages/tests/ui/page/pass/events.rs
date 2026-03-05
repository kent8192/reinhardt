//! page! macro with event handlers

use reinhardt_pages::page;

fn main() {
	// Click event
	let _with_click = page!(|| {
		button {
			@click: |_| { },
			"Click me"
		}
	});

	// Multiple events
	let _with_events = page!(|| {
		input {
			@input: |_| { },
			@change: |_| { },
			@focus: |_| { },
			@blur: |_| { },
		}
	});

	// Form events
	let _form = page!(|| {
		form {
			@submit: |_| { },
			button {
				@click: |_| { },
				"Submit"
			}
		}
	});
}
