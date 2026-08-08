use reinhardt::db::orm::{FileField, ModelFileField};
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "documents", table_name = "attachments")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Attachment {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = "attachments")]
	file: Option<FileField>,
}

fn main() {
	let _: ModelFileField<Attachment> = Attachment::file_file();
}
