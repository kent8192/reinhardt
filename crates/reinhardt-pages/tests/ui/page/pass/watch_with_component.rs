//! Test: Watch block with component call
//!
//! Validates that watch blocks can contain component invocations.

use reinhardt_pages::Signal;
use reinhardt_pages::component::Page;
use reinhardt_pages::page;

// Simulated component function for testing
fn my_component(_props: &str) -> Page {
	Page::Empty
}

fn main() {
	// Watch with component call inside conditional
	let _with_component = page!(|show: Signal<bool>| {
		div {
			watch {
				if show.get() {
					div {
						class: "wrapper",
						"Component would go here"
					}
				}
			}
		}
	});
}
