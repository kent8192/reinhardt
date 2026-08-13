use reinhardt::db::orm::FileField;
use reinhardt::model;
use serde::{Deserialize, Serialize};

const UPLOAD_DIRECTORY: &str = "avatars";

#[model(app_label = "accounts")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = UPLOAD_DIRECTORY)]
	avatar: FileField,
}

fn main() {}
