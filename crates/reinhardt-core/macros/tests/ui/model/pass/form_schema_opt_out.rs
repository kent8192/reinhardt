use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "polls")]
struct Question {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 200)]
	title: String,
}

fn main() {
	let _ = Question::build().title("Question?").finish();
}
