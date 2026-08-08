#![allow(unexpected_cfgs)]

use reinhardt::db::orm::{FieldRef, FileField, GeneratedModelField, ModelFileField};
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "accounts", table_name = "profiles")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Profile {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(upload_to = "avatars/%Y/%m/%d", max_length = 255)]
	avatar: FileField,
	#[field(
		upload_to = "documents/%H/%M/%S",
		file_storage = "private_uploads",
	)]
	document: Option<FileField>,
	#[field(upload_to = "external/%Y", file_storage = "default", storage = "external")]
	external: FileField,
}

fn generated_api() {
	let _: ModelFileField<Profile> = Profile::file_avatar();
	let _: ModelFileField<Profile> = Profile::file_document();
	let _: ModelFileField<Profile> = Profile::file_external();
	let _: FieldRef<Profile, FileField, GeneratedModelField> = Profile::field_avatar();
	let _: FieldRef<Profile, Option<FileField>, GeneratedModelField> = Profile::field_document();
}

fn main() {
	generated_api();
}
