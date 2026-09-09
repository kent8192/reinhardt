use reinhardt_pages::{event::EventFile, page, reactive::Signal};

fn main() {
	let files = Signal::new(String::new());
	let _ = page!({
		input {
			a11y: off,
			type: "file",
			bind: files,
		}
	});

	let optional_file = Signal::new(None::<EventFile>);
	let _ = page!({
		input {
			a11y: off,
			type: "file",
			bind: optional_file,
		}
	});
}
