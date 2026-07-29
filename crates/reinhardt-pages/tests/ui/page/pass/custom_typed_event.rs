//! Typed custom intrinsic events accept typed sync, async, external, and Callback handlers.

use reinhardt_pages::event::CustomEvent;
use reinhardt_pages::{Callback, page};
use serde::Deserialize;

#[derive(Deserialize)]
struct ItemSelected {
	id: u64,
}

fn external(event: CustomEvent<ItemSelected>) {
	let _ = event.detail();
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _inferred = page!(|| {
			div {
				@custom::<ItemSelected>("item-selected"): |event| {
					let _: CustomEvent<ItemSelected> = event;
				},
			}
		});
		let _explicit = page!(|| {
			div {
				@custom::<ItemSelected>("item-selected-explicit"): |event: CustomEvent<ItemSelected>| {
					let _ = event.into_detail();
				},
			}
		});
		let _async = page!(|| {
			div {
				@custom::<ItemSelected>("item-loaded"): async |event| {
					let _ = event.detail();
				},
			}
		});
		let _external = page!(|| {
			div {
				@custom::<ItemSelected>("item-removed"): crate::external,
			}
		});
		let callback = Callback::<CustomEvent<ItemSelected>, ()>::new(|event| {
			let _ = event.detail();
		});
		let _callback = page!(|callback: Callback<CustomEvent<ItemSelected>, ()>| {
			div {
				@custom::<ItemSelected>("item-callback"): callback,
			}
		})(callback);
		let _zero_argument = page!(|| {
			div {
				@custom::<ItemSelected>("item-focused"): || {},
			}
		});
	});
}
