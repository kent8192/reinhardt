use reinhardt_testkit_macros::with_di_overrides;

fn main() {
	let _future = async {
		let _ = with_di_overrides! { singleton };
	};
}
