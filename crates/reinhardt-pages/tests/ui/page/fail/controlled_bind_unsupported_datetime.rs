use reinhardt_pages::page;

fn main() {
	page!(|| {
		input {
			a11y: off,
			type: "datetime",
			bind: (),
		}
	});
}
