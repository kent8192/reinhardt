use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");

use db::orm::{DatabaseField, Model};

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
	#[field(index(
		name = "documents_embedding_cosine_hnsw",
		method = "hnsw",
		opclass = "vector_cosine_ops",
		m = 16,
		ef_construction = 64
	))]
	embedding: Vector<1536>,
	#[field(index(
		name = "select embedding-ann",
		method = "ivfflat",
		opclass = "vector_l2_ops",
		lists = 100
	))]
	summary: Vector<768>,
	#[field(max_length = 255, index = true)]
	title: String,
}

fn main() {
	let indexes = Document::index_metadata();
	assert_eq!(indexes.len(), 3);
	assert_eq!(indexes[0].name, "idx_documents_title");
	assert_eq!(indexes[1].name, "documents_embedding_cosine_hnsw");
	assert_eq!(indexes[2].name, "select embedding-ann");
}
