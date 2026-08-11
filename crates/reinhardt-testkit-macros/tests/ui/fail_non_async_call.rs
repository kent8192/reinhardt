use reinhardt_testkit_macros::with_di_overrides;

struct Service;

fn main() {
	let _ = with_di_overrides! { singleton Service };
}
