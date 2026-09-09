use reinhardt_pages::{event::EventFile, page, reactive::Signal};

fn main() {
	let files = Signal::new(Vec::<EventFile>::new());
	let _ = page!({
		input {
			a11y: off,
			type: "file",
			value: "initial",
			bind: files,
		}
	});
}
