use reinhardt_db::orm::{
	Field, FieldSelector, Manager, Model, QuerySet, Vector,
	query::{Filter, FilterOperator, FilterValue},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Document {
	id: Option<i64>,
	embedding: Vector<3>,
	title: String,
}

#[derive(Clone)]
struct DocumentFields {
	embedding: Field<Document, Vector<3>>,
	title: Field<Document, String>,
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

fn typed_query(target: Vector<3>) -> reinhardt_core::exception::Result<QuerySet<Document>> {
	let fields = Document::new_fields();
	Ok(QuerySet::<Document>::new()
		.filter(fields.embedding.clone().cosine_distance(target.clone()).lt(0.3))
		.order_by(fields.embedding.clone().l2_distance(target.clone()).asc())
		.annotate(
			fields
				.embedding
				.clone()
				.negative_inner_product(target.clone())
				.label("inner_distance")?,
		)?
		.select_expr("cosine_distance", fields.embedding.cosine_distance(target)))
}

fn typed_annotation_query() -> reinhardt_core::exception::Result<QuerySet<Document>> {
	Ok(QuerySet::<Document>::new()
		.filter(Filter::new(
			"title",
			FilterOperator::Eq,
			FilterValue::String("guide".to_owned()),
		))
		.order_by(&["title"])
		.annotate(Document::new_fields().title.into_expression().label("title_copy")?)?
		.values(&["title"]))
}

fn main() {
	let target = Vector::<3>::try_from_slice(&[1.0, 2.0, 3.0]).unwrap();
	let _typed = typed_query(target).unwrap();
	let _typed_annotation = typed_annotation_query().unwrap();
}
