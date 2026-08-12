use reinhardt::db::orm::FileField;
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "accounts")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	#[field(upload_to = "avatars", storage = "default")]
	avatar: FileField,
}

fn main() {}
