//! Typed custom event handlers must accept the declared payload wrapper.

// reinhardt-fmt: ignore-all

use reinhardt_pages::event::CustomEvent;
use reinhardt_pages::page;
use serde::Deserialize;

#[derive(Deserialize)]
struct ItemSelected;

#[derive(Deserialize)]
struct OtherPayload;

fn handle_other(event: CustomEvent<OtherPayload>) {
	let _ = event;
}

fn main() {
	let _invalid = page!(|| {
		div { @custom::<ItemSelected>("item-selected"): crate::handle_other, }
	});
}
