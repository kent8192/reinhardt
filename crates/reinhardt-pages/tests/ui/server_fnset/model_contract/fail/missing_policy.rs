use reinhardt_db::orm::{FieldSelector, Manager, Model, TransactionExecutor, UniqueFieldRef};
use reinhardt_pages::server_fn::{
	ModelServerFnResource, PageRequest, ServerFnListQuery, ServerFnResource, ServerFnSetError,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Article {
	id: Option<i64>,
}

#[derive(Clone)]
struct ArticleFields;

impl FieldSelector for ArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for Article {
	type PrimaryKey = i64;
	type Fields = ArticleFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"articles"
	}

	fn new_fields() -> Self::Fields {
		ArticleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

struct ListQuery;

impl ServerFnListQuery for ListQuery {
	fn page_request(&self) -> PageRequest {
		PageRequest::default()
	}
}

struct ArticleResource;

impl ServerFnResource for ArticleResource {
	type Lookup = i64;
	type Read = i64;
	type Create = i64;
	type Update = i64;
	type Patch = i64;
	type ListQuery = ListQuery;
}

#[async_trait::async_trait]
impl ModelServerFnResource for ArticleResource {
	type Model = Article;

	fn lookup_field() -> UniqueFieldRef<Self::Model, Self::Lookup> {
		// SAFETY: The handwritten test model declares `id` as its unique primary key.
		unsafe { UniqueFieldRef::from_model_field_with_names("id", "id") }
	}

	async fn to_read(
		model: &Self::Model,
		_executor: Option<&mut dyn TransactionExecutor>,
	) -> Result<Self::Read, ServerFnSetError> {
		Ok(model.id.unwrap_or_default())
	}
}

fn main() {}
