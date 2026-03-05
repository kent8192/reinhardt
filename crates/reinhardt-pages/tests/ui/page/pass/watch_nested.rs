//! Test: Nested watch blocks
//!
//! Validates that watch blocks can be nested within other elements
//! and multiple watch blocks can exist within the same parent.

use reinhardt_pages::Signal;
use reinhardt_pages::page;

fn main() {
	// Watch nested inside multiple elements
	let _nested_in_element = page!(|active: Signal<bool>| {
		div {
			class: "container",
			section {
				article {
					watch {
						if active.get() {
							p {
								"Active content"
							}
						}
					}
				}
			}
		}
	});

	// Multiple watch blocks in same parent
	let _multiple_watches = page!(|loading: Signal<bool>, error: Signal<Option<String>>| {
		div {
			watch {
				if loading.get() {
					div {
						"Loading..."
					}
				}
			}
			watch {
				if error.get().is_some() {
					div {
						class: "error",
						{ error.get().unwrap_or_default() }
					}
				}
			}
		}
	});
}
