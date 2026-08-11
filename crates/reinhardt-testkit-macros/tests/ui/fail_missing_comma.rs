use reinhardt_testkit_macros::with_di_overrides;

struct First;
struct Second;

fn main() {
	let _future = async {
		let _ = with_di_overrides! {
			singleton First request Second
		};
	};
}
