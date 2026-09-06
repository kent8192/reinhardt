//! `BooleanField` bind listener uses `.checked()` directly with `Signal<bool>`.

use reinhardt_pages::form;

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _ = form! {
			name: ToggleForm,
			action: "/api/toggle",
			fields: {
				enabled: BooleanField {
					bind: true,
					initial: false,
				}
			}
		};
	});
}
