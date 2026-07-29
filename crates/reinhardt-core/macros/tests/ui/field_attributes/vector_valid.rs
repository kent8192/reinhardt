use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");

use db::orm::{DatabaseField, DatabaseStorageKind, Model};

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
	embedding: Vector<1536>,
}

fn assert_embedding_selector(_field: db::orm::query_fields::Field<Document, Vector<1536>>) {}

fn main() {
	let fields = Document::new_fields();
	assert_embedding_selector(fields.embedding);

	let embedding = Document::field_metadata()
		.into_iter()
		.find(|field| field.name == "embedding")
		.expect("generated embedding metadata");
	assert_eq!(
		embedding.storage_kind,
		Some(DatabaseStorageKind::Vector(1536))
	);
	assert_eq!(embedding.field_type, "reinhardt.orm.models.VectorField");
}
