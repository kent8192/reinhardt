use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");

use db::orm::DatabaseField;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Vector;

impl DatabaseField for Vector {
	type Storage = String;

	fn encode_database(&self) -> Result<Self::Storage, db::orm::FieldCodecError> {
		Ok(String::new())
	}

	fn decode_database(
		_value: Self::Storage,
		_context: &db::orm::FieldCodecContext,
	) -> Result<Self, db::orm::FieldCodecError> {
		Ok(Self)
	}
}

#[model(app_label = "search", table_name = "documents")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	embedding: Vector,
}

fn main() {}
