//! Typed custom event payloads must be deserializable.

use reinhardt_pages::page;

struct NotDeserializable;

fn main() {
	let _invalid = page!(|| {
		div {
			@custom::<NotDeserializable>("item-selected"): |event| {
				let _ = event;
			},
		}
	});
}
