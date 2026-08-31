//! `IntegerField` bind listener generates code compatible with `Signal<i64>`.

use reinhardt_pages::form;

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _ = form! {
			name: CounterForm,
			action: "/api/counter",
			fields: {
				count: IntegerField {
					bind,
					initial: 0i64,
				}
			}
		};
	});
}
