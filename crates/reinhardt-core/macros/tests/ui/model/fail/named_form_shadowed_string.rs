#![allow(unexpected_cfgs)]

use reinhardt_macros::model;

include!("../support.rs");

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct ShadowString(std::string::String);

type String = ShadowString;

impl reinhardt::db::orm::DatabaseField for ShadowString {
	type Storage = std::string::String;

	fn encode_database(&self) -> Result<Self::Storage, reinhardt::db::orm::FieldCodecError> {
		Ok(self.0.clone())
	}

	fn decode_database(
		value: Self::Storage,
		_context: &reinhardt::db::orm::FieldCodecContext,
	) -> Result<Self, reinhardt::db::orm::FieldCodecError> {
		Ok(Self(value))
	}
}

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(name))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 100)]
	name: String,
}

fn main() {}
