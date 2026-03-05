//! page! macro with nested for loops

use reinhardt_pages::page;

fn main() {
	// Simple for loop
	let _simple = page!(|items: Vec<String>| {
		ul {
			for item in items {
				li {
					{ item }
				}
			}
		}
	});

	// For loop with nested if
	let _for_if = page!(|items: Vec<(i32, bool)>| {
		ul {
			for (num, active) in items {
				li {
					if active {
						strong {
							{ format!("{}", num) }
						}
					} else {
						span {
							{ format!("{}", num) }
						}
					}
				}
			}
		}
	});
}
