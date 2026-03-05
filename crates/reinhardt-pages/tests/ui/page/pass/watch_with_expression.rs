//! Test: Watch block with expression node
//!
//! Validates that watch blocks can contain expression nodes
//! that evaluate to dynamic content.

use reinhardt_pages::Signal;
use reinhardt_pages::page;

fn main() {
	// Watch with expression node
	let _with_expr = page!(|count: Signal<i32>| {
		div {
			watch {
				{ format!("Count: {}", count.get()) }
			}
		}
	});

	// Watch with expression in element
	let _expr_in_element = page!(|name: Signal<String>| {
		div {
			watch {
				span {
					class: "greeting",
					{ format!("Hello, {}!", name.get()) }
				}
			}
		}
	});

	// Watch with conditional expression
	let _conditional_expr = page!(|value: Signal<Option<String>>| {
		div {
			watch {
				{ value.get().unwrap_or_else(| | "Default".to_string()) }
			}
		}
	});
}
