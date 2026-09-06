use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(attachment))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = "documents", max_length = 100)]
	attachment: FileField,
}

fn main() {}
