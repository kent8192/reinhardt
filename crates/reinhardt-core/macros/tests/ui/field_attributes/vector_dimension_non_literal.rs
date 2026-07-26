use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");

use db::orm::DatabaseField;

const DIMENSIONS: usize = 1536;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Vector<const N: usize>(String);

impl<const N: usize> DatabaseField for Vector<N> {
	type Storage = String;

	fn encode_database(&self) -> Result<Self::Storage, db::orm::FieldCodecError> {
		Ok(self.0.clone())
	}

	fn decode_database(
		value: Self::Storage,
		_context: &db::orm::FieldCodecContext,
	) -> Result<Self, db::orm::FieldCodecError> {
		Ok(Self(value))
	}
}

#[model(app_label = "search", table_name = "documents")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	embedding: Vector<DIMENSIONS>,
}

fn main() {}
