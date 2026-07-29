use reinhardt_db::orm::{Field, FieldSelector, Manager, Model, Vector};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Document {
	pub id: Option<i64>,
	pub embedding: Vector<3>,
	pub title: String,
}

#[derive(Clone)]
pub struct DocumentFields {
	pub embedding: Field<Document, Vector<3>>,
	pub title: Field<Document, String>,
}

impl FieldSelector for DocumentFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.embedding = self.embedding.with_alias(alias);
		self.title = self.title.with_alias(alias);
		self
	}
}

impl Model for Document {
	type PrimaryKey = i64;
	type Fields = DocumentFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"documents"
	}

	fn new_fields() -> Self::Fields {
		DocumentFields {
			embedding: Field::new(vec!["embedding"]),
			title: Field::new(vec!["title"]),
		}
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OtherDocument {
	pub id: Option<i64>,
	pub embedding: Vector<3>,
}

#[derive(Clone)]
pub struct OtherDocumentFields {
	pub embedding: Field<OtherDocument, Vector<3>>,
}

impl FieldSelector for OtherDocumentFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.embedding = self.embedding.with_alias(alias);
		self
	}
}

impl Model for OtherDocument {
	type PrimaryKey = i64;
	type Fields = OtherDocumentFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"other_documents"
	}

	fn new_fields() -> Self::Fields {
		OtherDocumentFields {
			embedding: Field::new(vec!["embedding"]),
		}
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

pub fn vector3() -> Vector<3> {
	Vector::try_from_slice(&[1.0, 2.0, 3.0]).unwrap()
}

pub fn vector4() -> Vector<4> {
	Vector::try_from_slice(&[1.0, 2.0, 3.0, 4.0]).unwrap()
}
